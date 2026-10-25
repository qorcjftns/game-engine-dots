use std::sync::{Arc, RwLock};

use engine_document::EditorSession;
use engine_protocol::{ErrorCode, LiveInspection, ProjectInspection};

use crate::{Result, failure, random_id};

pub struct SessionTracker {
    session_id: String,
    generation: Option<u64>,
    revision: u64,
    project: Option<Arc<ProjectInspection>>,
}

impl SessionTracker {
    pub fn new() -> Result<Self> {
        Ok(Self {
            session_id: random_id()?,
            generation: None,
            revision: 0,
            project: None,
        })
    }

    /// generation은 GUI의 프로젝트 교체마다 증가해야 합니다. 같은 파일 재열기도 포함합니다.
    pub fn observe(
        &mut self,
        generation: u64,
        session: Option<&EditorSession>,
        busy: bool,
    ) -> Result<LiveInspection> {
        let replaced = self
            .generation
            .is_some_and(|previous| previous != generation)
            || self.project.is_some() != session.is_some();
        let changed = match (&self.project, session) {
            (Some(previous), Some(current)) => previous.document != *current.document(),
            (None, None) => false,
            _ => true,
        };
        if replaced {
            self.session_id = random_id()?;
            self.revision = 0;
        } else if changed && self.generation.is_some() {
            if let Some(next) = self.revision.checked_add(1) {
                self.revision = next;
            } else {
                self.session_id = random_id()?;
                self.revision = 0;
            }
        }
        if replaced || changed {
            self.project = session.map(|current| {
                Arc::new(ProjectInspection {
                    project_file: current.project_file().display().to_string(),
                    document: current.document().clone(),
                })
            });
        }
        self.generation = Some(generation);
        Ok(LiveInspection {
            session_id: self.session_id.clone(),
            revision: self.revision,
            busy,
            is_dirty: session.is_some_and(EditorSession::is_dirty),
            can_undo: session.is_some_and(EditorSession::can_undo),
            can_redo: session.is_some_and(EditorSession::can_redo),
            project: self.project.clone(),
        })
    }
}

/// TCP 스레드는 GUI나 파일을 읽지 않고 마지막으로 게시한 확정 상태만 가져갑니다.
#[derive(Clone)]
pub struct SnapshotPublisher {
    state: Arc<RwLock<Arc<LiveInspection>>>,
}

impl SnapshotPublisher {
    pub(crate) fn new(initial: LiveInspection) -> Self {
        Self {
            state: Arc::new(RwLock::new(Arc::new(initial))),
        }
    }

    pub fn publish(&self, snapshot: LiveInspection) -> Result<()> {
        let mut state = self
            .state
            .write()
            .map_err(|_| failure(ErrorCode::ControlIo, "세션 상태를 게시할 수 없습니다."))?;
        *state = Arc::new(snapshot);
        Ok(())
    }

    pub fn snapshot(&self) -> Result<Arc<LiveInspection>> {
        self.state
            .read()
            .map(|state| state.clone())
            .map_err(|_| failure(ErrorCode::ControlIo, "세션 상태를 조회할 수 없습니다."))
    }
}
