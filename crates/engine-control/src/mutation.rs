//! 원장은 세션 동안 항목을 퇴출하지 않습니다. 응답 유실 후에도 같은 ID를 다시 적용하지 않습니다.
use crate::{Result, failure};
use engine_document::{EditCommand, EditorSession};
use engine_protocol::{
    ApiError, ErrorCode, LiveInspection, MutationReceipt, MutationRequest, MutationState,
};
pub use engine_protocol::{MutationCommand, MutationEffect};
use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

pub const MUTATION_QUEUE_CAPACITY: usize = 32;
pub const RECEIPT_CAPACITY: usize = 1024;

struct Record {
    request: MutationRequest,
    deadline: Instant,
    receipt: MutationReceipt,
}
struct Ledger {
    session_id: String,
    records: HashMap<String, Record>,
    queue: VecDeque<String>,
    wakeup: Option<Arc<dyn Fn() + Send + Sync>>,
}
pub(crate) struct MutationHub {
    ledger: Mutex<Ledger>,
}
impl MutationHub {
    pub(crate) fn new(session_id: String) -> Self {
        Self {
            ledger: Mutex::new(Ledger {
                session_id,
                records: HashMap::new(),
                queue: VecDeque::new(),
                wakeup: None,
            }),
        }
    }
    pub(crate) fn reset_session(&self, session_id: &str) -> Result<()> {
        let mut ledger = self.ledger.lock().map_err(|_| poisoned())?;
        if ledger.session_id != session_id {
            ledger.session_id = session_id.into();
            ledger.records.clear();
            ledger.queue.clear();
        }
        Ok(())
    }
    pub(crate) fn enable(
        self: &Arc<Self>,
        wakeup: impl Fn() + Send + Sync + 'static,
    ) -> Result<MutationInbox> {
        let mut ledger = self.ledger.lock().map_err(|_| poisoned())?;
        if ledger.wakeup.is_some() {
            return Err(failure(
                ErrorCode::InvalidRequest,
                "변경 수신자는 하나만 연결할 수 있습니다.",
            ));
        }
        ledger.wakeup = Some(Arc::new(wakeup));
        Ok(MutationInbox { hub: self.clone() })
    }
    pub(crate) fn submit(
        &self,
        request: MutationRequest,
        current: &LiveInspection,
    ) -> Result<MutationReceipt> {
        if request.request_id.is_empty()
            || request.request_id.len() > 128
            || !request
                .request_id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
            || !(1..=10_000).contains(&request.timeout_ms)
        {
            return Err(failure(
                ErrorCode::InvalidRequest,
                "request_id는 ASCII 영숫자/-/_ 1~128자, timeout_ms는 1~10000이어야 합니다.",
            ));
        }
        let mut ledger = self.ledger.lock().map_err(|_| poisoned())?;
        if ledger.session_id != request.expected_session_id
            || current.session_id != request.expected_session_id
        {
            return Err(session_mismatch());
        }
        if let Some(record) = ledger.records.get_mut(&request.request_id) {
            if record.request != request {
                return Err(failure(
                    ErrorCode::RequestConflict,
                    "같은 request_id에 다른 요청을 사용할 수 없습니다.",
                ));
            }
            expire(record);
            return Ok(record.receipt.clone());
        }
        let wakeup = ledger.wakeup.clone().ok_or_else(|| {
            failure(
                ErrorCode::MutationsDisabled,
                "이 서버는 변경 요청을 받지 않습니다.",
            )
        })?;
        if ledger.records.len() >= RECEIPT_CAPACITY {
            return Err(failure(
                ErrorCode::LedgerFull,
                "세션 원장이 가득 찼습니다. 기존 결과 조회는 가능합니다.",
            ));
        }
        let rejection = guard(&request, current);
        if rejection.is_none() && ledger.queue.len() >= MUTATION_QUEUE_CAPACITY {
            return Err(failure(
                ErrorCode::QueueFull,
                "변경 대기열이 가득 찼습니다.",
            ));
        }
        let receipt = MutationReceipt {
            session_id: request.expected_session_id.clone(),
            request_id: request.request_id.clone(),
            state: rejection.map_or(MutationState::Queued, |error| MutationState::Rejected {
                revision: current.revision,
                error,
            }),
        };
        let record = Record {
            deadline: Instant::now() + Duration::from_millis(request.timeout_ms.into()),
            request,
            receipt: receipt.clone(),
        };
        if matches!(receipt.state, MutationState::Queued) {
            ledger.queue.push_back(receipt.request_id.clone());
        }
        ledger.records.insert(receipt.request_id.clone(), record);
        drop(ledger);
        wakeup();
        Ok(receipt)
    }
    pub(crate) fn lookup(&self, session_id: &str, request_id: &str) -> Result<MutationReceipt> {
        let mut ledger = self.ledger.lock().map_err(|_| poisoned())?;
        if ledger.session_id != session_id {
            return Err(session_mismatch());
        }
        let record = ledger.records.get_mut(request_id).ok_or_else(|| {
            failure(
                ErrorCode::UnknownRequest,
                "보관된 요청이 없습니다. 미적용으로 단정하거나 새 ID로 재시도하지 마세요.",
            )
        })?;
        expire(record);
        Ok(record.receipt.clone())
    }
}

/// GUI 메인 스레드만 소유합니다. 네트워크 스레드는 EditorSession에 접근하지 않습니다.
pub struct MutationInbox {
    hub: Arc<MutationHub>,
}
impl MutationInbox {
    pub fn try_recv(&mut self) -> Option<PendingMutation> {
        let mut ledger = self.hub.ledger.lock().ok()?;
        let request_id = ledger.queue.pop_front()?;
        Some(PendingMutation {
            hub: self.hub.clone(),
            session_id: ledger.session_id.clone(),
            request_id,
        })
    }
}
pub struct PendingMutation {
    hub: Arc<MutationHub>,
    session_id: String,
    request_id: String,
}
impl PendingMutation {
    /// 실제 실행 직전 GUI의 최신 상태를 다시 검사합니다. 실패도 원장에 남깁니다.
    pub fn begin(&mut self, current: &LiveInspection) -> Result<MutationCommand> {
        let mut ledger = self.hub.ledger.lock().map_err(|_| poisoned())?;
        if ledger.session_id != self.session_id || current.session_id != self.session_id {
            return Err(session_mismatch());
        }
        let record = ledger
            .records
            .get_mut(&self.request_id)
            .ok_or_else(session_mismatch)?;
        expire(record);
        if matches!(record.receipt.state, MutationState::ExpiredBeforeApply) {
            return Err(failure(
                ErrorCode::DeadlineExceeded,
                "대기 중 기한이 만료되어 적용하지 않았습니다.",
            ));
        }
        if !matches!(record.receipt.state, MutationState::Queued) {
            return Err(failure(
                ErrorCode::RequestConflict,
                "이미 처리된 요청입니다.",
            ));
        }
        if let Some(error) = guard(&record.request, current) {
            record.receipt.state = MutationState::Rejected {
                revision: current.revision,
                error: error.clone(),
            };
            return Err(error);
        }
        record.receipt.state = MutationState::Running;
        Ok(record.request.command.clone())
    }
    /// GUI가 변경 문서를 게시한 다음 호출합니다. 전체 문서는 receipt에 복사하지 않습니다.
    pub fn finish(
        self,
        outcome: Result<MutationEffect>,
        current: &LiveInspection,
    ) -> Result<MutationReceipt> {
        let mut ledger = self.hub.ledger.lock().map_err(|_| poisoned())?;
        if ledger.session_id != self.session_id || current.session_id != self.session_id {
            return Err(session_mismatch());
        }
        let record = ledger
            .records
            .get_mut(&self.request_id)
            .ok_or_else(session_mismatch)?;
        if !matches!(record.receipt.state, MutationState::Running) {
            return Err(failure(
                ErrorCode::RequestConflict,
                "실행 중인 요청만 완료할 수 있습니다.",
            ));
        }
        record.receipt.state = match outcome {
            Ok(effect) => MutationState::Applied {
                revision: current.revision,
                is_dirty: current.is_dirty,
                effect,
            },
            Err(error) => MutationState::Rejected {
                revision: current.revision,
                error,
            },
        };
        Ok(record.receipt.clone())
    }
}
fn expire(record: &mut Record) {
    if matches!(record.receipt.state, MutationState::Queued) && Instant::now() >= record.deadline {
        record.receipt.state = MutationState::ExpiredBeforeApply;
    }
}
fn guard(request: &MutationRequest, current: &LiveInspection) -> Option<ApiError> {
    if request.expected_session_id != current.session_id {
        Some(session_mismatch())
    } else if request.expected_revision != current.revision {
        Some(failure(
            ErrorCode::RevisionMismatch,
            "예상 revision이 현재 문서와 다릅니다.",
        ))
    } else if current.busy {
        Some(failure(
            ErrorCode::EditorBusy,
            "사람의 입력/드래그/대화상자가 끝난 뒤 새 요청을 만드세요.",
        ))
    } else if current.project.is_none() {
        Some(failure(ErrorCode::NoProject, "열린 프로젝트가 없습니다."))
    } else {
        None
    }
}
fn session_mismatch() -> ApiError {
    failure(
        ErrorCode::SessionMismatch,
        "편집 세션이 바뀌었습니다. 현재 상태를 다시 조회하세요.",
    )
}
fn poisoned() -> ApiError {
    failure(ErrorCode::ControlIo, "변경 원장을 사용할 수 없습니다.")
}

/// 기존 문서 코어만 실행합니다. GUI draft를 확정하거나 외부 파일을 직접 재개방하지 않습니다.
pub fn apply_command(
    command: &MutationCommand,
    session: &mut EditorSession,
) -> Result<MutationEffect> {
    let edit = match command {
        MutationCommand::AddBehavior { file, source } => Some(EditCommand::AddBehavior {
            file: file.clone(),
            source: source.clone(),
        }),
        MutationCommand::UpdateBehavior { id, file, source } => Some(EditCommand::UpdateBehavior {
            id: *id,
            file: file.clone(),
            source: source.clone(),
        }),
        MutationCommand::RemoveBehavior { id } => Some(EditCommand::RemoveBehavior { id: *id }),
        MutationCommand::BindBehavior { id, behavior_id } => Some(EditCommand::BindBehavior {
            id: *id,
            behavior_id: *behavior_id,
        }),
        MutationCommand::ValidateBehaviors => {
            return Ok(MutationEffect {
                diagnostics: engine_runtime::validate_behaviors(session.document()),
                ..Default::default()
            });
        }
        MutationCommand::RunStart
        | MutationCommand::RunPause { .. }
        | MutationCommand::RunStep { .. }
        | MutationCommand::RunStop { .. } => {
            return Err(failure(
                ErrorCode::InvalidRunState,
                "실행 명령은 GUI 실행 호스트가 처리합니다.",
            ));
        }
        MutationCommand::AddSprite {
            asset_id,
            name,
            position,
        } => Some(EditCommand::AddSprite {
            asset_id: *asset_id,
            name: name.clone(),
            position: *position,
        }),
        MutationCommand::RemoveEntity { id } => Some(EditCommand::RemoveEntity { id: *id }),
        MutationCommand::RenameEntity { id, name } => Some(EditCommand::RenameEntity {
            id: *id,
            name: name.clone(),
        }),
        MutationCommand::MoveEntity { id, position } => Some(EditCommand::MoveEntity {
            id: *id,
            position: *position,
        }),
        MutationCommand::Undo => {
            session.undo();
            None
        }
        MutationCommand::Redo => {
            session.redo();
            None
        }
        MutationCommand::Save => {
            session.save().map_err(ApiError::from)?;
            None
        }
    };
    match edit {
        Some(edit) => session
            .execute(edit)
            .map(|result| MutationEffect {
                entity_id: result.entity_id,
                behavior_id: result.behavior_id,
                diagnostics: Vec::new(),
            })
            .map_err(|error| failure(ErrorCode::CommandFailed, error.to_string())),
        None => Ok(MutationEffect::default()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SessionTracker;
    use engine_document::{EntityId, Position};
    fn fixture() -> (
        tempfile::TempDir,
        EditorSession,
        SessionTracker,
        LiveInspection,
        Arc<MutationHub>,
        MutationInbox,
    ) {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("문서.json");
        std::fs::write(&path, r#"{"schema_version":1,"name":"검증","logical_width":320,"logical_height":180,"scene":{"entities":[{"id":73,"asset_id":19,"name":"원본","position":{"x":1,"y":2}}]},"assets":[{"id":19,"path":"assets/test.png","width":1,"height":1}]}"#).unwrap();
        let session = EditorSession::open(path).unwrap();
        let mut tracker = SessionTracker::new().unwrap();
        let snapshot = tracker.observe(1, Some(&session), false).unwrap();
        let hub = Arc::new(MutationHub::new(snapshot.session_id.clone()));
        let inbox = hub.enable(|| {}).unwrap();
        (directory, session, tracker, snapshot, hub, inbox)
    }
    fn request(snapshot: &LiveInspection, id: &str) -> MutationRequest {
        MutationRequest {
            expected_session_id: snapshot.session_id.clone(),
            expected_revision: snapshot.revision,
            request_id: id.into(),
            timeout_ms: 1000,
            command: MutationCommand::MoveEntity {
                id: EntityId(73),
                position: Position { x: 40, y: 28 },
            },
        }
    }
    #[test]
    fn duplicate_lookup_conflict_and_same_revision_race_never_apply_twice() {
        let (_dir, mut session, mut tracker, current, hub, mut inbox) = fixture();
        let first = request(&current, "one");
        assert!(matches!(
            hub.submit(first.clone(), &current).unwrap().state,
            MutationState::Queued
        ));
        hub.submit(first.clone(), &current).unwrap();
        hub.submit(request(&current, "two"), &current).unwrap();
        let mut pending = inbox.try_recv().unwrap();
        let command = pending.begin(&current).unwrap();
        assert!(matches!(
            hub.lookup(&current.session_id, "one").unwrap().state,
            MutationState::Running
        ));
        let outcome = apply_command(&command, &mut session);
        let changed = tracker.observe(1, Some(&session), false).unwrap();
        let receipt = pending.finish(outcome, &changed).unwrap();
        assert!(matches!(
            receipt.state,
            MutationState::Applied { revision: 1, .. }
        ));
        assert_eq!(hub.submit(first.clone(), &changed).unwrap(), receipt);
        let mut conflicting = first;
        conflicting.timeout_ms = 999;
        assert_eq!(
            hub.submit(conflicting, &changed).unwrap_err().code,
            ErrorCode::RequestConflict
        );
        assert_eq!(
            inbox.try_recv().unwrap().begin(&changed).unwrap_err().code,
            ErrorCode::RevisionMismatch
        );
        assert!(matches!(
            hub.lookup(&current.session_id, "two").unwrap().state,
            MutationState::Rejected { .. }
        ));
        assert!(inbox.try_recv().is_none());
        assert!(session.undo());
        assert!(!session.is_dirty());
        assert!(!session.can_undo());
    }
    #[test]
    fn busy_is_checked_at_ingress_and_apply_without_committing_drafts() {
        let (_dir, _session, _tracker, current, hub, mut inbox) = fixture();
        let mut busy = current.clone();
        busy.busy = true;
        let req = request(&current, "busy");
        let rejected = hub.submit(req.clone(), &busy).unwrap();
        assert!(matches!(
            rejected.state,
            MutationState::Rejected {
                error: ApiError {
                    code: ErrorCode::EditorBusy,
                    ..
                },
                ..
            }
        ));
        assert_eq!(hub.submit(req, &current).unwrap(), rejected);
        hub.submit(request(&current, "race"), &current).unwrap();
        assert_eq!(
            inbox.try_recv().unwrap().begin(&busy).unwrap_err().code,
            ErrorCode::EditorBusy
        );
    }
    #[test]
    fn queued_expiry_is_terminal_and_session_replacement_invalidates_old_queue() {
        let (_dir, _session, _tracker, current, hub, mut inbox) = fixture();
        let req = request(&current, "expired");
        hub.submit(req.clone(), &current).unwrap();
        hub.ledger
            .lock()
            .unwrap()
            .records
            .get_mut("expired")
            .unwrap()
            .deadline = Instant::now() - Duration::from_millis(1);
        assert_eq!(
            hub.lookup(&current.session_id, "expired").unwrap().state,
            MutationState::ExpiredBeforeApply
        );
        assert_eq!(
            inbox.try_recv().unwrap().begin(&current).unwrap_err().code,
            ErrorCode::DeadlineExceeded
        );
        assert_eq!(
            hub.submit(req, &current).unwrap().state,
            MutationState::ExpiredBeforeApply
        );
        hub.submit(request(&current, "old"), &current).unwrap();
        let mut old = inbox.try_recv().unwrap();
        hub.reset_session("new-session").unwrap();
        assert_eq!(
            old.begin(&current).unwrap_err().code,
            ErrorCode::SessionMismatch
        );
        assert_eq!(
            hub.lookup(&current.session_id, "old").unwrap_err().code,
            ErrorCode::SessionMismatch
        );
    }
    #[test]
    fn queue_and_ledger_are_bounded_without_eviction() {
        let (_dir, _session, _tracker, current, hub, mut inbox) = fixture();
        for i in 0..MUTATION_QUEUE_CAPACITY {
            hub.submit(request(&current, &format!("q{i}")), &current)
                .unwrap();
        }
        assert_eq!(
            hub.submit(request(&current, "overflow"), &current)
                .unwrap_err()
                .code,
            ErrorCode::QueueFull
        );
        while let Some(mut pending) = inbox.try_recv() {
            let mut busy = current.clone();
            busy.busy = true;
            assert!(pending.begin(&busy).is_err());
        }
        let mut busy = current.clone();
        busy.busy = true;
        for i in MUTATION_QUEUE_CAPACITY..RECEIPT_CAPACITY {
            hub.submit(request(&current, &format!("q{i}")), &busy)
                .unwrap();
        }
        assert_eq!(
            hub.submit(request(&current, "full"), &current)
                .unwrap_err()
                .code,
            ErrorCode::LedgerFull
        );
        assert!(matches!(
            hub.lookup(&current.session_id, "q0").unwrap().state,
            MutationState::Rejected { .. }
        ));
        assert!(hub.submit(request(&current, "q0"), &current).is_ok());
    }
    #[test]
    fn command_and_save_failure_preserve_document_history_and_files() {
        let (dir, mut session, mut tracker, current, hub, mut inbox) = fixture();
        let original = session.document().clone();
        let mut invalid = request(&current, "invalid");
        invalid.command = MutationCommand::RemoveEntity { id: EntityId(999) };
        hub.submit(invalid, &current).unwrap();
        let mut pending = inbox.try_recv().unwrap();
        let outcome = apply_command(&pending.begin(&current).unwrap(), &mut session);
        assert!(matches!(
            pending.finish(outcome, &current).unwrap().state,
            MutationState::Rejected { .. }
        ));
        assert_eq!(*session.document(), original);
        assert!(!session.can_undo());
        apply_command(
            &MutationCommand::MoveEntity {
                id: EntityId(73),
                position: Position { x: 99, y: 7 },
            },
            &mut session,
        )
        .unwrap();
        let changed = tracker.observe(1, Some(&session), false).unwrap();
        std::fs::write(dir.path().join("문서.json"), b"external edit").unwrap();
        let mut save = request(&changed, "save");
        save.command = MutationCommand::Save;
        hub.submit(save, &changed).unwrap();
        let mut pending = inbox.try_recv().unwrap();
        let outcome = apply_command(&pending.begin(&changed).unwrap(), &mut session);
        assert!(matches!(
            pending.finish(outcome, &changed).unwrap().state,
            MutationState::Rejected { .. }
        ));
        assert_eq!(
            std::fs::read(dir.path().join("문서.json")).unwrap(),
            b"external edit"
        );
        assert!(session.is_dirty() && session.can_undo());
        assert_eq!(session.document().scene.entities[0].position.x, 99);
        apply_command(&MutationCommand::Undo, &mut session).unwrap();
        assert_eq!(*session.document(), original);
    }
}

#[cfg(test)]
mod extra_tests {
    use super::*;
    use crate::SessionTracker;
    #[test]
    fn duplicate_add_and_invalid_command_replay_preserve_exact_receipt() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("project.json");
        std::fs::write(&path,r#"{"schema_version":1,"name":"검증","logical_width":320,"logical_height":180,"scene":{"entities":[]},"assets":[{"id":19,"path":"assets/test.png","width":1,"height":1}]}"#).unwrap();
        let mut session = EditorSession::open(path).unwrap();
        let mut tracker = SessionTracker::new().unwrap();
        let initial = tracker.observe(1, Some(&session), false).unwrap();
        let hub = Arc::new(MutationHub::new(initial.session_id.clone()));
        let mut inbox = hub.enable(|| {}).unwrap();
        let request = MutationRequest {
            expected_session_id: initial.session_id.clone(),
            expected_revision: 0,
            request_id: "add-once".into(),
            timeout_ms: 1000,
            command: MutationCommand::AddSprite {
                asset_id: engine_document::AssetId(19),
                name: "신규".into(),
                position: engine_document::Position::default(),
            },
        };
        hub.submit(request.clone(), &initial).unwrap();
        hub.submit(request.clone(), &initial).unwrap();
        let mut pending = inbox.try_recv().unwrap();
        let command = pending.begin(&initial).unwrap();
        let outcome = apply_command(&command, &mut session);
        let changed = tracker.observe(1, Some(&session), false).unwrap();
        let receipt = pending.finish(outcome, &changed).unwrap();
        assert!(matches!(
            receipt.state,
            MutationState::Applied {
                effect: MutationEffect {
                    entity_id: Some(_),
                    ..
                },
                ..
            }
        ));
        assert_eq!(hub.submit(request, &changed).unwrap(), receipt);
        assert_eq!(
            hub.lookup(&initial.session_id, "add-once").unwrap(),
            receipt
        );
        assert_eq!(session.document().scene.entities.len(), 1);
        assert!(inbox.try_recv().is_none());
        let invalid = MutationRequest {
            expected_session_id: changed.session_id.clone(),
            expected_revision: changed.revision,
            request_id: "invalid".into(),
            timeout_ms: 1000,
            command: MutationCommand::RemoveEntity {
                id: engine_document::EntityId(999),
            },
        };
        hub.submit(invalid.clone(), &changed).unwrap();
        let mut pending = inbox.try_recv().unwrap();
        let command = pending.begin(&changed).unwrap();
        let outcome = apply_command(&command, &mut session);
        let rejected = pending.finish(outcome, &changed).unwrap();
        assert_eq!(hub.submit(invalid, &changed).unwrap(), rejected);
        assert_eq!(
            hub.lookup(&initial.session_id, "invalid").unwrap(),
            rejected
        );
        let queued = MutationRequest {
            expected_session_id: changed.session_id.clone(),
            expected_revision: changed.revision,
            request_id: "old-project".into(),
            timeout_ms: 1000,
            command: MutationCommand::Undo,
        };
        hub.submit(queued, &changed).unwrap();
        let fresh = tracker.observe(2, Some(&session), false).unwrap();
        hub.reset_session(&fresh.session_id).unwrap();
        assert!(inbox.try_recv().is_none());
        assert_eq!(
            hub.lookup(&changed.session_id, "old-project")
                .unwrap_err()
                .code,
            ErrorCode::SessionMismatch
        );
        assert_eq!(session.document().scene.entities.len(), 1);
    }
}
