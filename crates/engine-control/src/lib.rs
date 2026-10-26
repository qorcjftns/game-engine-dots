//! GUI가 게시한 불변 snapshot을 로컬 CLI에 제공하는 읽기 전용 제어 통로입니다.
//! 연결 파일에는 비밀 token이 있으므로 사용자 전용 외부 경로를 사용하고 출력하지 않습니다.

mod connection;
mod mutation;
mod server;
mod snapshot;

pub use connection::{inspect_connection_file, lookup_connection_file, mutate_connection_file};
pub use mutation::{
    MUTATION_QUEUE_CAPACITY, MutationCommand, MutationEffect, MutationInbox, PendingMutation,
    RECEIPT_CAPACITY, apply_command,
};
pub use server::QueryServer;
pub use snapshot::{SessionTracker, SnapshotPublisher};

pub type ControlError = engine_protocol::ApiError;
pub type Result<T> = std::result::Result<T, ControlError>;

pub const MAX_REQUEST_BYTES: usize = 8 * 1024;
pub const MAX_RESPONSE_BYTES: usize = 4 * 1024 * 1024;
pub const SOCKET_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);

fn failure(code: engine_protocol::ErrorCode, message: impl Into<String>) -> ControlError {
    ControlError::new(code, message)
}

fn random_id() -> Result<String> {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes).map_err(|_| {
        failure(
            engine_protocol::ErrorCode::ControlIo,
            "세션 식별자를 만들 수 없습니다.",
        )
    })?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}
