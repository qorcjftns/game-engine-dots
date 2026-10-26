//! MCP schema 전용 입력입니다. 모든 정수와 문자열을 공통 계약으로 그대로 옮깁니다.
use engine_document::{AssetId, EntityId, Position};
use engine_protocol::{MutationCommand, MutationRequest};
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EmptyInput {}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ResultInput {
    pub session_id: String,
    pub request_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MutationInput {
    pub expected_session_id: String,
    pub expected_revision: u64,
    pub request_id: String,
    pub timeout_ms: u32,
    pub command: CommandInput,
}
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PositionInput {
    pub x: i32,
    pub y: i32,
}
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum CommandInput {
    AddSprite {
        asset_id: u64,
        name: String,
        position: PositionInput,
    },
    RemoveEntity {
        id: u64,
    },
    RenameEntity {
        id: u64,
        name: String,
    },
    MoveEntity {
        id: u64,
        position: PositionInput,
    },
    Undo {},
    Redo {},
    Save {},
    RunStart {},
    RunPause {
        run_id: u64,
        paused: bool,
    },
    RunStep {
        run_id: u64,
        ticks: u32,
        input: ActionInput,
    },
    RunStop {
        run_id: u64,
    },
}
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ActionInput {
    pub left: bool,
    pub right: bool,
    pub up: bool,
    pub down: bool,
    pub primary: bool,
    pub pointer: Option<[i32; 2]>,
}
impl From<ActionInput> for engine_runtime::ActionInput {
    fn from(v: ActionInput) -> Self {
        Self {
            left: v.left,
            right: v.right,
            up: v.up,
            down: v.down,
            primary: v.primary,
            pointer: v.pointer,
        }
    }
}
impl From<PositionInput> for Position {
    fn from(value: PositionInput) -> Self {
        Self {
            x: value.x,
            y: value.y,
        }
    }
}
impl From<MutationInput> for MutationRequest {
    fn from(value: MutationInput) -> Self {
        let command = match value.command {
            CommandInput::AddSprite {
                asset_id,
                name,
                position,
            } => MutationCommand::AddSprite {
                asset_id: AssetId(asset_id),
                name,
                position: position.into(),
            },
            CommandInput::RemoveEntity { id } => MutationCommand::RemoveEntity { id: EntityId(id) },
            CommandInput::RenameEntity { id, name } => MutationCommand::RenameEntity {
                id: EntityId(id),
                name,
            },
            CommandInput::MoveEntity { id, position } => MutationCommand::MoveEntity {
                id: EntityId(id),
                position: position.into(),
            },
            CommandInput::Undo {} => MutationCommand::Undo,
            CommandInput::Redo {} => MutationCommand::Redo,
            CommandInput::Save {} => MutationCommand::Save,
            CommandInput::RunStart {} => MutationCommand::RunStart,
            CommandInput::RunPause { run_id, paused } => {
                MutationCommand::RunPause { run_id, paused }
            }
            CommandInput::RunStep {
                run_id,
                ticks,
                input,
            } => MutationCommand::RunStep {
                run_id,
                ticks,
                input: input.into(),
            },
            CommandInput::RunStop { run_id } => MutationCommand::RunStop { run_id },
        };
        Self {
            expected_session_id: value.expected_session_id,
            expected_revision: value.expected_revision,
            request_id: value.request_id,
            timeout_ms: value.timeout_ms,
            command,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn all_commands_preserve_ids_unicode_and_integer_boundaries() {
        for command in [
            json!({
                "type": "add_sprite",
                "asset_id": u64::MAX,
                "name": "한글 \\\" 이름",
                "position": {
                    "x": i32::MIN,
                    "y": i32::MAX
                }
            }),
            json!({"type":"remove_entity","id":u64::MAX}),
            json!({"type":"rename_entity","id":73,"name":"이름"}),
            json!({"type":"move_entity","id":19,"position":{"x":-12,"y":38}}),
            json!({"type":"undo"}),
            json!({"type":"redo"}),
            json!({"type":"save"}),
            json!({"type":"run_start"}),
            json!({"type":"run_pause","run_id":7,"paused":true}),
            json!({"type":"run_stop","run_id":7}),
            json!({"type":"run_step","run_id":7,"ticks":73,"input":{"left":true,"right":false,"up":false,"down":false,"primary":false,"pointer":null}}),
        ] {
            let value = json!({
                "expected_session_id": "session",
                "expected_revision": u64::MAX,
                "request_id": "same-id",
                "timeout_ms": 5000,
                "command": command
            });
            let input: MutationInput = serde_json::from_value(value.clone()).unwrap();
            let common: MutationRequest = input.into();
            assert_eq!(serde_json::to_value(common).unwrap(), value);
        }
    }
    #[test]
    fn malformed_or_extended_commands_are_not_coerced() {
        for command in [
            json!({"type":"shell","code":"anything"}),
            json!({"type":"save","extra":1}),
            json!({"type":"move_entity","id":1,"position":{"x":1.5,"y":2}}),
            json!({"type":"remove_entity","id":-1}),
        ] {
            assert!(serde_json::from_value::<CommandInput>(command).is_err());
        }
    }
}
