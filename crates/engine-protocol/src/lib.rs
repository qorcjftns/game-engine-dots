//! CLI와 향후 live/MCP 어댑터가 공유할 JSON 자료형입니다. 파일이나 GUI를 직접 다루지 않습니다.

use engine_document::{DocumentError, PngAsset, ProjectDocument, Scene};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// 프로젝트 파일의 schema_version과 별개인 도구 응답 계약 버전입니다.
pub const PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataSource {
    Disk,
    Live,
}

/// tagged enum으로 성공 응답에 오류가, 오류 응답에 data가 섞이지 않게 합니다.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ApiResponse<T> {
    Ok {
        protocol_version: u32,
        source: DataSource,
        data: T,
    },
    Error {
        protocol_version: u32,
        source: DataSource,
        error: ApiError,
    },
}

impl<T> ApiResponse<T> {
    pub fn success(source: DataSource, data: T) -> Self {
        Self::Ok {
            protocol_version: PROTOCOL_VERSION,
            source,
            data,
        }
    }

    pub fn failure(source: DataSource, error: ApiError) -> Self {
        Self::Error {
            protocol_version: PROTOCOL_VERSION,
            source,
            error,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectInspection {
    /// 해당 프로젝트 파일의 canonical 경로입니다. live 문서는 저장 전 변경도 포함할 수 있습니다.
    pub project_file: String,
    pub document: ProjectDocument,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SceneInspection {
    pub project_file: String,
    pub project_name: String,
    pub logical_width: u32,
    pub logical_height: u32,
    /// entities 순서는 실제 장면의 그리기 순서이며 ID를 재발급하지 않습니다.
    pub scene: Scene,
    pub assets: Vec<PngAsset>,
}

/// 확정된 문서만 공유하며 입력 중인 문자열과 드래그 미리보기는 포함하지 않습니다.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiveInspection {
    pub session_id: String,
    pub revision: u64,
    pub busy: bool,
    pub is_dirty: bool,
    pub can_undo: bool,
    pub can_redo: bool,
    /// 상태만 바뀔 때 문서 복사를 피하는 불변 공유입니다. JSON에서는 일반 객체입니다.
    pub project: Option<Arc<ProjectInspection>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    InvalidArguments,
    ProjectNotFound,
    ProjectAccessDenied,
    ProjectIo,
    InvalidProject,
    ControlIo,
    InvalidConnectionFile,
    InvalidRequest,
    Unauthorized,
    UnsupportedProtocol,
    RequestTooLarge,
    ResponseTooLarge,
    ServerBusy,
    EditorBusy,
    SessionMismatch,
    RevisionMismatch,
    RequestConflict,
    LedgerFull,
    QueueFull,
    DeadlineExceeded,
    UnknownRequest,
    MutationsDisabled,
    NoProject,
    CommandFailed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiError {
    pub code: ErrorCode,
    pub message: String,
}

impl ApiError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ApiError {}

impl From<DocumentError> for ApiError {
    fn from(error: DocumentError) -> Self {
        let code = match &error {
            DocumentError::Io(error) => match error.kind() {
                std::io::ErrorKind::NotFound => ErrorCode::ProjectNotFound,
                std::io::ErrorKind::PermissionDenied => ErrorCode::ProjectAccessDenied,
                _ => ErrorCode::ProjectIo,
            },
            _ => ErrorCode::InvalidProject,
        };
        Self::new(code, error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn success_and_error_have_distinct_json_shapes() {
        let success = ApiResponse::success(DataSource::Disk, "조회 결과".to_owned());
        let encoded = serde_json::to_value(&success).unwrap();
        assert_eq!(encoded["protocol_version"], 1);
        assert_eq!(encoded["source"], "disk");
        assert_eq!(encoded["status"], "ok");
        assert!(encoded.get("error").is_none());
        assert_eq!(
            serde_json::from_value::<ApiResponse<String>>(encoded).unwrap(),
            success
        );

        let failure = ApiResponse::<()>::failure(
            DataSource::Disk,
            ApiError::new(ErrorCode::InvalidProject, "문서 오류"),
        );
        let encoded = serde_json::to_value(&failure).unwrap();
        assert_eq!(encoded["status"], "error");
        assert_eq!(encoded["error"]["code"], "invalid_project");
        assert!(encoded.get("data").is_none());
    }

    #[test]
    fn filesystem_errors_keep_actionable_codes() {
        for (kind, expected) in [
            (std::io::ErrorKind::NotFound, ErrorCode::ProjectNotFound),
            (
                std::io::ErrorKind::PermissionDenied,
                ErrorCode::ProjectAccessDenied,
            ),
            (std::io::ErrorKind::InvalidData, ErrorCode::ProjectIo),
        ] {
            let error = ApiError::from(DocumentError::Io(std::io::Error::from(kind)));
            assert_eq!(error.code, expected);
        }
    }
}

/// GUI와 CLI/MCP가 공유하는 기존 스프라이트 편집 명령입니다.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum MutationCommand {
    AddSprite {
        asset_id: engine_document::AssetId,
        name: String,
        position: engine_document::Position,
    },
    RemoveEntity {
        id: engine_document::EntityId,
    },
    RenameEntity {
        id: engine_document::EntityId,
        name: String,
    },
    MoveEntity {
        id: engine_document::EntityId,
        position: engine_document::Position,
    },
    Undo,
    Redo,
    Save,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MutationRequest {
    pub expected_session_id: String,
    pub expected_revision: u64,
    pub request_id: String,
    /// 서버 수신 시 고정되는 대기 기한입니다. 이미 실행 중인 저장을 취소하지 않습니다.
    pub timeout_ms: u32,
    pub command: MutationCommand,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MutationEffect {
    pub entity_id: Option<engine_document::EntityId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MutationReceipt {
    pub session_id: String,
    pub request_id: String,
    #[serde(flatten)]
    pub state: MutationState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum MutationState {
    Queued,
    Running,
    Applied {
        revision: u64,
        is_dirty: bool,
        effect: MutationEffect,
    },
    Rejected {
        revision: u64,
        error: ApiError,
    },
    ExpiredBeforeApply,
}

// serde의 unit variant는 추가 필드를 무시하므로, 입력에만 빈 struct variant를 사용합니다.
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum StrictMutationCommand {
    AddSprite {
        asset_id: engine_document::AssetId,
        name: String,
        position: engine_document::Position,
    },
    RemoveEntity {
        id: engine_document::EntityId,
    },
    RenameEntity {
        id: engine_document::EntityId,
        name: String,
    },
    MoveEntity {
        id: engine_document::EntityId,
        position: engine_document::Position,
    },
    Undo {},
    Redo {},
    Save {},
}
impl From<StrictMutationCommand> for MutationCommand {
    fn from(value: StrictMutationCommand) -> Self {
        match value {
            StrictMutationCommand::AddSprite {
                asset_id,
                name,
                position,
            } => Self::AddSprite {
                asset_id,
                name,
                position,
            },
            StrictMutationCommand::RemoveEntity { id } => Self::RemoveEntity { id },
            StrictMutationCommand::RenameEntity { id, name } => Self::RenameEntity { id, name },
            StrictMutationCommand::MoveEntity { id, position } => Self::MoveEntity { id, position },
            StrictMutationCommand::Undo {} => Self::Undo,
            StrictMutationCommand::Redo {} => Self::Redo,
            StrictMutationCommand::Save {} => Self::Save,
        }
    }
}
impl<'de> Deserialize<'de> for MutationCommand {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        StrictMutationCommand::deserialize(deserializer).map(Into::into)
    }
}
#[cfg(test)]
mod strict_mutation_tests {
    use super::*;
    #[test]
    fn unit_commands_reject_extra_fields_without_changing_public_variants() {
        for (name, expected) in [
            ("undo", MutationCommand::Undo),
            ("redo", MutationCommand::Redo),
            ("save", MutationCommand::Save),
        ] {
            let valid = serde_json::json!({"type":name});
            assert_eq!(
                serde_json::from_value::<MutationCommand>(valid.clone()).unwrap(),
                expected
            );
            assert_eq!(serde_json::to_value(expected).unwrap(), valid);
            assert!(
                serde_json::from_value::<MutationCommand>(
                    serde_json::json!({"type":name,"extra":1})
                )
                .is_err()
            );
        }
    }
}
