//! 한정 Rhai 호스트. 문서 IO와 GUI를 제공하지 않으며 tick 실패는 world에 반영하지 않는다.
use crate::ActionInput;
use engine_document::{BehaviorResource, EntityId, Position, ProjectDocument};
use rhai::{
    AST, Engine, EvalAltResult, ImmutableString, NativeCallContext, OptimizationLevel, Scope,
};
use serde::{Deserialize, Serialize};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
};

const TICK_OPERATIONS: u64 = 8000;
const MAX_LOGS: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BehaviorDiagnostic {
    pub file: String,
    pub line: usize,
    pub column: usize,
    pub entity_id: Option<EntityId>,
    pub tick: u64,
    pub message: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BehaviorLog {
    pub file: String,
    pub line: usize,
    pub entity_id: EntityId,
    pub tick: u64,
    pub message: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BehaviorState {
    pub entity_id: EntityId,
    pub position: Position,
    pub state: i64,
}
fn bounded_text(text: &str) -> String {
    text.chars()
        .scan(0, |bytes, ch| {
            *bytes += ch.len_utf8();
            (*bytes <= 1024).then_some(ch)
        })
        .collect()
}
fn diagnostic(
    resource: &BehaviorResource,
    entity_id: Option<EntityId>,
    tick: u64,
    pos: rhai::Position,
    message: String,
) -> BehaviorDiagnostic {
    BehaviorDiagnostic {
        file: resource.file.clone(),
        line: pos.line().unwrap_or(1),
        column: pos.position().unwrap_or(1),
        entity_id,
        tick,
        message: bounded_text(&message),
    }
}
fn engine(counter: Arc<AtomicU64>, logs: Arc<Mutex<Vec<(usize, String)>>>) -> Engine {
    let mut engine = Engine::new();
    engine
        .set_max_operations(2000)
        .set_max_call_levels(8)
        .set_max_expr_depths(32, 16)
        .set_max_variables(32)
        .set_max_functions(16)
        .set_max_string_size(256);
    engine.set_optimization_level(OptimizationLevel::None);
    // 동적 소스 컴파일과 stdout는 프로젝트 API가 아니다.
    engine.disable_symbol("eval");
    engine.on_print(|_| {});
    engine.on_debug(|_, _, _| {});
    engine.on_progress(move |_| {
        (counter.fetch_add(1, Ordering::Relaxed) >= TICK_OPERATIONS)
            .then_some("tick operation budget".into())
    });
    engine.register_fn(
        "log",
        move |ctx: NativeCallContext, message: ImmutableString| -> Result<(), Box<EvalAltResult>> {
            let mut logs = logs.lock().expect("log mutex");
            if logs.len() >= MAX_LOGS || message.len() > 256 {
                return Err(EvalAltResult::ErrorRuntime(
                    "log budget exceeded".into(),
                    ctx.call_position(),
                )
                .into());
            }
            logs.push((ctx.call_position().line().unwrap_or(1), message.to_string()));
            Ok(())
        },
    );
    engine
}
/// 문법 검사는 소스를 실행하지 않는다. runtime 이름/자료형 오류는 Play에서 진단한다.
pub fn validate_behaviors(document: &ProjectDocument) -> Vec<BehaviorDiagnostic> {
    let engine = engine(Default::default(), Default::default());
    document
        .behaviors
        .iter()
        .filter_map(|resource| {
            engine
                .compile(&resource.source)
                .err()
                .map(|error| diagnostic(resource, None, 0, error.position(), error.to_string()))
        })
        .collect()
}
struct Binding {
    resource: BehaviorResource,
    entity: usize,
    ast: AST,
    state: i64,
}
pub(crate) struct Behaviors {
    engine: Engine,
    bindings: Vec<Binding>,
    counter: Arc<AtomicU64>,
    logs: Arc<Mutex<Vec<(usize, String)>>>,
}
impl Behaviors {
    pub fn compile(document: &ProjectDocument) -> Result<Self, BehaviorDiagnostic> {
        let counter = Arc::default();
        let logs = Arc::default();
        let engine = engine(Arc::clone(&counter), Arc::clone(&logs));
        let mut bindings = Vec::new();
        for (entity, item) in document.scene.entities.iter().enumerate() {
            let Some(id) = item.behavior_id else {
                continue;
            };
            let resource = document
                .behaviors
                .iter()
                .find(|b| b.id == id)
                .expect("validated binding");
            let ast = engine.compile(&resource.source).map_err(|error| {
                diagnostic(
                    resource,
                    Some(item.id),
                    0,
                    error.position(),
                    error.to_string(),
                )
            })?;
            bindings.push(Binding {
                resource: resource.clone(),
                entity,
                ast,
                state: 0,
            });
        }
        Ok(Self {
            engine,
            bindings,
            counter,
            logs,
        })
    }
    pub fn has_bindings(&self) -> bool {
        !self.bindings.is_empty()
    }
    pub fn states(&self, document: &ProjectDocument) -> Vec<BehaviorState> {
        self.bindings
            .iter()
            .map(|b| BehaviorState {
                entity_id: document.scene.entities[b.entity].id,
                position: document.scene.entities[b.entity].position,
                state: b.state,
            })
            .collect()
    }
    pub fn tick(
        &mut self,
        document: &mut ProjectDocument,
        tick: u64,
        input: ActionInput,
        pressed: ActionInput,
        released: ActionInput,
    ) -> Result<(bool, Vec<BehaviorLog>), BehaviorDiagnostic> {
        self.counter.store(0, Ordering::Relaxed);
        let mut updates = Vec::new();
        let mut all_logs = Vec::new();
        for binding in &self.bindings {
            let entity = &document.scene.entities[binding.entity];
            let mut scope = Scope::new();
            scope
                .push("x", i64::from(entity.position.x))
                .push("y", i64::from(entity.position.y))
                .push("state", binding.state);
            scope.push_constant("tick", i64::try_from(tick).unwrap_or(i64::MAX));
            for (name, value) in [
                ("left", input.left),
                ("right", input.right),
                ("up", input.up),
                ("down", input.down),
                ("primary", input.primary),
                ("primary_pressed", pressed.primary),
                ("primary_released", released.primary),
            ] {
                scope.push_constant(name, value);
            }
            scope
                .push_constant("pointer_x", i64::from(input.pointer.map_or(-1, |p| p[0])))
                .push_constant("pointer_y", i64::from(input.pointer.map_or(-1, |p| p[1])));
            self.logs.lock().expect("log mutex").clear();
            self.engine
                .run_ast_with_scope(&mut scope, &binding.ast)
                .map_err(|error| {
                    diagnostic(
                        &binding.resource,
                        Some(entity.id),
                        tick,
                        error.unwrap_inner().position(),
                        error.to_string(),
                    )
                })?;
            let result = scope
                .get_value::<i64>("x")
                .and_then(|x| i32::try_from(x).ok())
                .zip(
                    scope
                        .get_value::<i64>("y")
                        .and_then(|y| i32::try_from(y).ok()),
                )
                .zip(scope.get_value::<i64>("state"));
            let Some(((x, y), state)) = result else {
                return Err(diagnostic(&binding.resource, Some(entity.id), tick, rhai::Position::new(1,1), "x/y는 i32 범위의 정수, state는 i64 정수여야 합니다. 최종 출력 검증 실패입니다.".into()));
            };
            updates.push((Position { x, y }, state));
            for (line, message) in self.logs.lock().expect("log mutex").drain(..) {
                if all_logs.len() >= MAX_LOGS {
                    return Err(diagnostic(
                        &binding.resource,
                        Some(entity.id),
                        tick,
                        rhai::Position::new(line as u16, 1),
                        "tick log budget exceeded".into(),
                    ));
                }
                all_logs.push(BehaviorLog {
                    file: binding.resource.file.clone(),
                    line,
                    entity_id: entity.id,
                    tick,
                    message,
                });
            }
        }
        let mut changed = false;
        for (binding, (position, state)) in self.bindings.iter_mut().zip(updates) {
            let target = &mut document.scene.entities[binding.entity].position;
            changed |= *target != position;
            *target = position;
            binding.state = state;
        }
        Ok((changed, all_logs))
    }
}
