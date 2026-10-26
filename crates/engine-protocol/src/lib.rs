//! CLI와 향후 live/MCP 어댑터가 공유할 JSON 자료형입니다. 파일이나 GUI를 직접 다루지 않습니다.

use engine_document::{DocumentError, PngAsset, ProjectDocument, Scene};
use serde::{Deserialize, Serialize};

/// 프로젝트 파일의 schema_version과 별개인 도구 응답 계약 버전입니다.
pub const PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataSource {
    Disk,
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
    /// 조회한 파일의 canonical 경로입니다. 저장 전 GUI 문서의 경로를 뜻하지 않습니다.
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    InvalidArguments,
    ProjectNotFound,
    ProjectAccessDenied,
    ProjectIo,
    InvalidProject,
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
