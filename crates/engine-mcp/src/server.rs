//! 자체 MCP 프로토콜이나 문서 사본 없이 공식 SDK와 공통 control client만 연결합니다.
use crate::input::{EmptyInput, MutationInput, ResultInput};
use engine_protocol::{ApiError, ApiResponse, DataSource, ErrorCode};
use rmcp::{
    ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, Implementation, ServerCapabilities, ServerConfig},
    tool, tool_handler, tool_router,
};
use serde::Serialize;
use std::{path::PathBuf, sync::Arc};

#[derive(Clone)]
pub struct EditorTools {
    connection_file: Arc<PathBuf>,
    tool_router: ToolRouter<Self>,
}
impl EditorTools {
    pub fn new(connection_file: PathBuf) -> Self {
        Self {
            connection_file: Arc::new(connection_file),
            tool_router: Self::tool_router(),
        }
    }
}
#[tool_router]
impl EditorTools {
    #[tool(
        description = "GUI의 확정 문서/session_id/revision/busy를 조회합니다. 입력 중 draft는 포함하지 않습니다.",
        annotations(read_only_hint = true)
    )]
    async fn session_inspect(&self, Parameters(_input): Parameters<EmptyInput>) -> CallToolResult {
        let file = self.connection_file.clone();
        execute(move || engine_control::inspect_connection_file(file.as_path())).await
    }
    #[tool(
        description = "같은 GUI 세션의 편집·실행 제어 요청을 접수합니다. queued/running은 적용 성공이 아니며 session_result로 확인합니다. 응답 유실 시 새 request_id로 재시도하지 마세요.",
        annotations(read_only_hint = false)
    )]
    async fn session_mutate(&self, Parameters(input): Parameters<MutationInput>) -> CallToolResult {
        let file = self.connection_file.clone();
        execute(move || engine_control::mutate_connection_file(file.as_path(), input.into())).await
    }
    #[tool(
        description = "session_id/request_id의 보관 결과를 조회합니다. unknown_request/세션교체는 미적용 증거가 아니며 새 ID로 blind retry하지 않습니다.",
        annotations(read_only_hint = true)
    )]
    async fn session_result(&self, Parameters(input): Parameters<ResultInput>) -> CallToolResult {
        let file = self.connection_file.clone();
        execute(move || {
            engine_control::lookup_connection_file(
                file.as_path(),
                &input.session_id,
                &input.request_id,
            )
        })
        .await
    }
}
#[tool_handler(router = self.tool_router)]
impl ServerHandler for EditorTools {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("engine-mcp", env!("CARGO_PKG_VERSION")))
            .with_instructions("로컬 GUI 한 개의 편집·실행 통로입니다. 먼저 session_inspect로 session/revision을 확인하세요. 사람의 draft/drag/modal 중 변경은 거부됩니다. runtime run_id로 pause/step/stop을 제어하며 실행 중 문서 변경은 거부됩니다. mutation receipt의 state를 확인하고 응답 유실은 동일 ID 결과 조회로 확인하세요. 프로젝트 파일 직접 쓰기·임의 코드 실행·provider 호출 도구는 제공하지 않습니다.".to_owned())
    }
}
async fn execute<T: Serialize + Send + 'static>(
    operation: impl FnOnce() -> Result<T, ApiError> + Send + 'static,
) -> CallToolResult {
    let result = tokio::task::spawn_blocking(operation)
        .await
        .unwrap_or_else(|_| {
            Err(ApiError::new(
                ErrorCode::ControlIo,
                "공통 제어 작업이 중단됐습니다. 적용 여부는 같은 요청 ID로 확인하세요.",
            ))
        });
    let failed = result.is_err();
    let response = match result {
        Ok(data) => ApiResponse::success(DataSource::Live, data),
        Err(error) => ApiResponse::failure(DataSource::Live, error),
    };
    match serde_json::to_value(response) {
        Ok(value) if failed => CallToolResult::structured_error(value),
        Ok(value) => CallToolResult::structured(value),
        Err(_) => CallToolResult::structured_error(
            serde_json::json!({"protocol_version":1,"source":"live","status":"error","error":{"code":"control_io","message":"응답을 만들지 못했습니다. 같은 요청 ID로 결과를 조회하세요."}}),
        ),
    }
}
