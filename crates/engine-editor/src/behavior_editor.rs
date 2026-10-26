//! 소스 draft와 확정 문서를 분리한다. 저장/전환은 기존 미저장 보호 경로를 사용한다.
use crate::app::EditorApp;
use eframe::egui;
use engine_document::{BehaviorId, EditCommand};

const TEMPLATE: &str = "if right { move(2,0); }\nif left { move(-2,0); }\nif down { move(0,2); }\nif up { move(0,-2); }\nset_state(state+1);\nif primary_pressed { log(\"primary pressed\"); }\n";
#[derive(Clone)]
pub(crate) struct BehaviorDraft {
    pub id: BehaviorId,
    pub file: String,
    pub source: String,
}
impl EditorApp {
    pub(crate) fn behavior_pending(&self) -> bool {
        self.behavior_draft.as_ref().is_some_and(|draft| {
            self.workspace
                .session
                .as_ref()
                .and_then(|s| s.document().behaviors.iter().find(|b| b.id == draft.id))
                .is_some_and(|b| b.file != draft.file || b.source != draft.source)
        })
    }
    pub(crate) fn load_behavior(&mut self, id: Option<BehaviorId>) {
        self.behavior_draft = self
            .workspace
            .session
            .as_ref()
            .and_then(|s| s.document().behaviors.iter().find(|b| Some(b.id) == id))
            .map(|b| BehaviorDraft {
                id: b.id,
                file: b.file.clone(),
                source: b.source.clone(),
            });
        self.behavior_validation.clear();
    }
    pub(crate) fn commit_behavior(&mut self) -> bool {
        if !self.behavior_pending() {
            return true;
        }
        let draft = self.behavior_draft.clone().unwrap();
        self.execute(EditCommand::UpdateBehavior {
            id: draft.id,
            file: draft.file,
            source: draft.source,
        })
    }
    pub(crate) fn behavior_window(&mut self, context: &egui::Context) {
        if !self.show_behavior {
            return;
        }
        let mut open = true;
        let mut select = None;
        let mut create = false;
        let mut apply = false;
        let mut reset = false;
        let mut validate = false;
        let mut bind = false;
        let mut unbind = false;
        let mut remove = false;
        let pending = self.behavior_pending();
        let resources = self
            .workspace
            .session
            .as_ref()
            .map(|s| s.document().behaviors.clone())
            .unwrap_or_default();
        egui::Window::new("행동 코드").open(&mut open).default_size([660.0,480.0]).show(context,|ui| {
            if self.play.is_some() || self.pending_transition.is_some() { ui.disable(); }
            ui.label("프로젝트에 저장하는 Rhai 소스 · 60 tick/초 · Stop 후 편집");
            ui.horizontal(|ui| {
                let current=self.behavior_draft.as_ref().map(|b|b.file.as_str()).unwrap_or("행동 선택");
                egui::ComboBox::from_id_salt("behavior-select").selected_text(current).show_ui(ui,|ui| {
                    for behavior in &resources { if ui.selectable_label(self.behavior_draft.as_ref().is_some_and(|d|d.id==behavior.id),&behavior.file).clicked() {select=Some(behavior.id);} }
                });
                create=ui.add_enabled(self.workspace.session.is_some(),egui::Button::new("새 행동")).clicked();
            });
            if let Some(draft)=&mut self.behavior_draft {
                ui.horizontal(|ui| {ui.label("파일 이름");ui.text_edit_singleline(&mut draft.file); if pending { ui.label("● 미확정 입력"); }});
                egui::ScrollArea::vertical().max_height(260.0).show(ui,|ui| {
                    ui.add(egui::TextEdit::multiline(&mut draft.source).code_editor().desired_rows(12).desired_width(f32::INFINITY));
                });
                ui.horizontal(|ui| {
                    validate=ui.button("문법 검증").clicked(); apply=ui.button("적용").clicked(); reset=ui.button("입력 되돌리기").clicked(); remove=ui.button("행동 삭제").clicked();
                });
                ui.horizontal(|ui| {
                    bind=ui.add_enabled(self.editing.selected.is_some(),egui::Button::new("선택 개체에 연결")).clicked();
                    unbind=ui.add_enabled(self.editing.selected.is_some(),egui::Button::new("선택 개체 연결 해제")).clicked();
                });
                if let Some(entity)=self.workspace.session.as_ref().and_then(|s|s.document().scene.entities.iter().find(|e|Some(e.id)==self.editing.selected)) {ui.label(format!("선택: {} · 행동 {:?}",entity.name,entity.behavior_id.map(|id|id.0)));}
                ui.small("x/y/state: tick 시작값 · move(dx,dy)/set_position(x,y)/set_state(n) · log(문자열)");
                ui.small("적용 후 Play. 문법 검증은 실행하지 않으며 실행 오류는 Play에서 확인합니다.");
            } else { ui.label("새 행동을 만들어 소스를 작성하세요."); }
            for message in &self.behavior_validation { ui.colored_label(egui::Color32::LIGHT_YELLOW,message); }
        });
        if !open {
            if pending {
                self.workspace.status = "행동 입력을 적용하거나 되돌린 뒤 창을 닫으세요.".into();
            } else {
                self.show_behavior = false;
            }
        }
        if reset {
            self.load_behavior(self.behavior_draft.as_ref().map(|d| d.id));
        }
        if apply {
            self.commit_behavior();
        }
        if validate
            && let Some(draft) = &self.behavior_draft
            && let Some(session) = &self.workspace.session
        {
            let mut document = session.document().clone();
            document.behaviors.retain(|b| b.id == draft.id);
            let behavior = &mut document.behaviors[0];
            behavior.file = draft.file.clone();
            behavior.source = draft.source.clone();
            self.behavior_validation =
                if draft.source.len() > engine_document::MAX_BEHAVIOR_SOURCE_BYTES {
                    vec!["행동 소스는 8192바이트 이하여야 합니다.".into()]
                } else {
                    engine_runtime::validate_behaviors(&document)
                        .iter()
                        .map(|d| format!("{}:{}:{} {}", d.file, d.line, d.column, d.message))
                        .collect()
                };
            if self.behavior_validation.is_empty() {
                self.behavior_validation
                    .push("문법 검증 성공 (실행 검증은 Play)".into());
            }
        }
        if let Some(id) = select
            && self.commit_behavior()
        {
            self.load_behavior(Some(id));
        }
        if create
            && self.commit_edits()
            && let Some(session) = &mut self.workspace.session
        {
            let mut number = 1;
            while session
                .document()
                .behaviors
                .iter()
                .any(|b| b.file == format!("behavior-{number}.rhai"))
            {
                number += 1;
            }
            match session.execute(EditCommand::AddBehavior {
                file: format!("behavior-{number}.rhai"),
                source: TEMPLATE.into(),
            }) {
                Ok(result) => self.load_behavior(result.behavior_id),
                Err(error) => self.workspace.error = Some(error.to_string()),
            }
            self.publish_live(false);
        }
        if (bind || unbind)
            && self.commit_edits()
            && let Some(id) = self.editing.selected
        {
            self.execute(EditCommand::BindBehavior {
                id,
                behavior_id: if unbind {
                    None
                } else {
                    self.behavior_draft.as_ref().map(|d| d.id)
                },
            });
        }
        if remove
            && self.commit_behavior()
            && let Some(id) = self.behavior_draft.as_ref().map(|d| d.id)
            && self.execute(EditCommand::RemoveBehavior { id })
        {
            self.load_behavior(None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_protocol::{ErrorCode, MutationCommand as C};
    #[test]
    fn human_source_draft_blocks_agents_and_save_commits_same_history() {
        let (_dir, mut app, id) = crate::editing_tests::fixture();
        let ctx = egui::Context::default();
        let effect = app
            .apply_agent_command(
                &ctx,
                &C::AddBehavior {
                    file: "move.rhai".into(),
                    source: TEMPLATE.into(),
                },
            )
            .unwrap();
        let behavior_id = effect.behavior_id.unwrap();
        app.apply_agent_command(
            &ctx,
            &C::BindBehavior {
                id,
                behavior_id: Some(behavior_id),
            },
        )
        .unwrap();
        app.load_behavior(Some(behavior_id));
        app.behavior_draft.as_mut().unwrap().source = "move(3,0);".into();
        assert!(app.has_unsaved_changes());
        assert!(app.editor_busy(&ctx));
        assert_eq!(
            app.apply_agent_command(&ctx, &C::Undo).unwrap_err().code,
            ErrorCode::EditorBusy
        );
        assert_eq!(
            app.apply_agent_command(
                &ctx,
                &C::UpdateBehavior {
                    id: behavior_id,
                    file: "move.rhai".into(),
                    source: "move(9,0);".into()
                }
            )
            .unwrap_err()
            .code,
            ErrorCode::EditorBusy
        );
        app.process_action(&ctx, crate::editor_actions::EditorAction::Save);
        assert!(!app.behavior_pending());
        let s = app.workspace.session.as_ref().unwrap();
        assert_eq!(s.document().behaviors[0].source, "move(3,0);");
        assert!(!s.is_dirty());
        assert_eq!(
            engine_document::EditorSession::open(s.project_file())
                .unwrap()
                .document(),
            s.document()
        );
        app.apply_agent_command(&ctx, &C::Undo).unwrap();
        assert_eq!(app.behavior_draft.as_ref().unwrap().source, TEMPLATE);
        app.apply_agent_command(&ctx, &C::Redo).unwrap();
        assert_eq!(app.behavior_draft.as_ref().unwrap().source, "move(3,0);");
    }
    #[test]
    fn source_validation_and_runtime_failure_preserve_editable_document() {
        let (_dir, mut app, id) = crate::editing_tests::fixture();
        let ctx = egui::Context::default();
        let behavior_id = app
            .apply_agent_command(
                &ctx,
                &C::AddBehavior {
                    file: "bad.rhai".into(),
                    source: "move(;".into(),
                },
            )
            .unwrap()
            .behavior_id
            .unwrap();
        app.apply_agent_command(
            &ctx,
            &C::BindBehavior {
                id,
                behavior_id: Some(behavior_id),
            },
        )
        .unwrap();
        let validation = app
            .apply_agent_command(&ctx, &C::ValidateBehaviors)
            .unwrap();
        assert_eq!(validation.diagnostics.len(), 1);
        let before = app.workspace.session.as_ref().unwrap().document().clone();
        assert!(app.start_play(&ctx).is_err());
        assert!(app.play.is_none());
        assert_eq!(app.workspace.session.as_ref().unwrap().document(), &before);
        app.apply_agent_command(
            &ctx,
            &C::UpdateBehavior {
                id: behavior_id,
                file: "bad.rhai".into(),
                source: "move(1,0);\nloop {}".into(),
            },
        )
        .unwrap();
        app.start_play(&ctx).unwrap();
        let run = app.play.as_mut().unwrap();
        run.pause(true);
        assert!(run.step(1).is_err());
        let run_id = run.status().run_id;
        assert_eq!(
            app.apply_agent_command(
                &ctx,
                &C::RunPause {
                    run_id,
                    paused: false
                }
            )
            .unwrap_err()
            .code,
            ErrorCode::InvalidRunState
        );
        app.apply_agent_command(&ctx, &C::RunStop { run_id })
            .unwrap();
        app.apply_agent_command(
            &ctx,
            &C::UpdateBehavior {
                id: behavior_id,
                file: "bad.rhai".into(),
                source: "move(2,0);".into(),
            },
        )
        .unwrap();
        app.start_play(&ctx).unwrap();
    }
}
