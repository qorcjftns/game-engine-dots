//! GUI·CLI·MCP가 함께 사용하는 편집 문서와 명령입니다. GUI 타입에는 의존하지 않습니다.
//!
//! ```no_run
//! use engine_document::{EditCommand, EditorSession, Position};
//!
//! let mut session = EditorSession::create("my-game/project.json", "내 게임", 320, 180)?;
//! let asset_id = session.import_png("hero.png")?;
//! session.execute(EditCommand::AddSprite {
//!     asset_id,
//!     name: "주인공".into(),
//!     position: Position { x: 32, y: 48 },
//! })?;
//! session.save()?;
//! # Ok::<(), engine_document::DocumentError>(())
//! ```
//!
//! PNG 가져오기와 스프라이트 추가는 각각 실행 취소 한 건입니다.
//! 실행 취소는 문서만 되돌리며 가져온 파일은 삭제하지 않습니다.
//! 프로젝트를 연 GUI 세션이 문서를 소유하며, 다른 프로세스와 동시 저장은 지원하지 않습니다.

mod assets;
mod model;
mod session;
mod storage;

pub use assets::PngPixels;
pub use model::{
    AssetId, BehaviorId, BehaviorResource, EntityId, MAX_BEHAVIOR_SOURCE_BYTES, PngAsset, Position,
    ProjectDocument, Scene, SpriteEntity,
};
pub use session::{CommandResult, EditCommand, EditorSession};

pub type Result<T> = std::result::Result<T, DocumentError>;

#[derive(Debug, thiserror::Error)]
pub enum DocumentError {
    #[error("문서 검증 실패: {0}")]
    InvalidDocument(String),
    #[error("파일 처리 실패: {0}")]
    Io(#[from] std::io::Error),
    #[error("프로젝트 JSON을 읽을 수 없습니다: {0}")]
    Json(#[from] serde_json::Error),
    #[error("PNG 이미지를 읽을 수 없습니다: {0}")]
    Image(#[from] image::ImageError),
    #[error("프로젝트 파일이 외부에서 바뀌었습니다. 다시 열기 전에 변경 내용을 확인하세요.")]
    ExternallyModified,
}

fn invalid(message: impl Into<String>) -> DocumentError {
    DocumentError::InvalidDocument(message.into())
}
