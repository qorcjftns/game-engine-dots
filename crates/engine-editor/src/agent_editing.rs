//! 원격 요청도 GUI가 소유한 세션에서 직렬 실행한다. 사람의 입력 초안은 확정하지 않는다.

use std::time::Duration;

use eframe::egui;
use engine_control::{MutationCommand, MutationEffect, apply_command};
use engine_protocol::{ApiError, ErrorCode};

use crate::app::EditorApp;

impl EditorApp {
    pub(crate) fn drain_agent_requests(&mut self, context: &egui::Context) {
        if self.live_control.is_none() {
            return;
        }
        // 가려진 창에서도 logic이 실행될 기회를 유지한다. enqueue도 즉시 repaint를 요청한다.
        context.request_repaint_after(Duration::from_millis(100));
        for _ in 0..32 {
            let Some(current) = self.publish_live(false) else {
                return;
            };
            let pending = self
                .live_control
                .as_mut()
                .and_then(|control| control.inbox.try_recv());
            let Some(mut pending) = pending else {
                break;
            };
            let Ok(command) = pending.begin(&current) else {
                continue;
            };
            let outcome = self.apply_agent_command(context, &command);
            // 적용이 끝난 문서와 이력 상태를 게시한 후 같은 상태를 receipt에 기록한다.
            let Some(after) = self.publish_live(false) else {
                // 적용된 문서를 되돌리거나 이전 revision으로 성공을 기록하지 않는다.
                self.workspace.status = "문서 처리 후 연결이 중단되어 원격 결과 확인이 불가능합니다. 현재 문서를 확인하세요.".into();
                return;
            };
            if let Err(error) = pending.finish(outcome, &after) {
                self.workspace.status = format!("편집 결과를 원장에 기록하지 못했습니다: {error}");
            }
        }
    }

    pub(crate) fn apply_agent_command(
        &mut self,
        context: &egui::Context,
        command: &MutationCommand,
    ) -> Result<MutationEffect, ApiError> {
        if self.editor_busy(context) {
            return Err(ApiError::new(
                ErrorCode::EditorBusy,
                "사람이 편집 중입니다. 입력을 마친 후 다시 요청하세요.",
            ));
        }
        let saving = matches!(command, MutationCommand::Save);
        if saving {
            self.begin_native_io();
        }
        let Some(session) = &mut self.workspace.session else {
            if saving {
                self.end_native_io();
            }
            return Err(ApiError::new(
                ErrorCode::NoProject,
                "열린 프로젝트가 없습니다.",
            ));
        };
        let previous_assets = session.document().assets.clone();
        let outcome = apply_command(command, session);
        let reload_assets = previous_assets != session.document().assets;
        self.synchronize_selection();
        // 텍스처 파일을 읽기 전에 확정 문서 변경을 공개한다.
        self.publish_live(reload_assets);
        if saving {
            self.end_native_io();
        }
        if reload_assets {
            self.begin_native_io();
            if let Some(session) = &self.workspace.session {
                self.workspace.textures.reload(context, session);
            }
            self.end_native_io();
        }
        self.workspace.status = match &outcome {
            Ok(_) => "에이전트의 편집 요청을 처리했습니다.".into(),
            Err(error) => format!("에이전트 요청 실패: {error}"),
        };
        outcome
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_document::{EditorSession, Position};

    #[test]
    fn tcp_logic_only_agent_requests_wake_apply_once_and_reject_stale_busy_and_expired_work() {
        use eframe::App;
        use engine_control::{
            inspect_connection_file, lookup_connection_file, mutate_connection_file,
        };
        use engine_protocol::{MutationRequest, MutationState};
        use std::sync::{Arc, Mutex};
        let (_directory, mut app, id) = crate::editing_tests::fixture();
        let private = tempfile::tempdir().unwrap();
        let path = private.path().join("control.json");
        let context = egui::Context::default();
        let repaint_delays = Arc::new(Mutex::new(Vec::new()));
        let observed = repaint_delays.clone();
        context.set_repaint_observer(move |info: egui::RequestRepaintInfo| {
            observed.lock().unwrap().push(info.delay)
        });
        app.start_agent_control(
            &context,
            crate::startup::AgentControlOptions {
                connection_file: Some(path.clone()),
            },
        );
        assert!(app.live_control.is_some(), "{:?}", app.workspace.error);
        let first = inspect_connection_file(&path).unwrap();
        let request = MutationRequest {
            expected_session_id: first.session_id.clone(),
            expected_revision: first.revision,
            request_id: "apply-once".into(),
            timeout_ms: 5000,
            command: MutationCommand::MoveEntity {
                id,
                position: Position { x: 45, y: 55 },
            },
        };
        repaint_delays.lock().unwrap().clear();
        assert!(matches!(
            mutate_connection_file(&path, request.clone())
                .unwrap()
                .state,
            MutationState::Queued
        ));
        assert!(repaint_delays.lock().unwrap().contains(&Duration::ZERO));
        assert_eq!(
            app.workspace
                .session
                .as_ref()
                .unwrap()
                .document()
                .scene
                .entities[0]
                .position,
            Position { x: 20, y: 30 }
        );
        let mut frame = eframe::Frame::_new_kittest();
        app.logic(&context, &mut frame);
        assert!(
            repaint_delays
                .lock()
                .unwrap()
                .contains(&Duration::from_millis(100))
        );
        let receipt =
            lookup_connection_file(&path, &first.session_id, &request.request_id).unwrap();
        assert!(matches!(receipt.state, MutationState::Applied { .. }));
        let applied = inspect_connection_file(&path).unwrap();
        assert_eq!(
            applied.project.as_ref().unwrap().document.scene.entities[0].position,
            Position { x: 45, y: 55 }
        );
        assert_eq!(
            mutate_connection_file(&path, request.clone()).unwrap(),
            receipt
        );
        app.logic(&context, &mut frame);
        assert_eq!(
            inspect_connection_file(&path).unwrap().revision,
            applied.revision
        );

        let mut stale = request.clone();
        stale.request_id = "stale".into();
        assert!(matches!(
            mutate_connection_file(&path, stale).unwrap().state,
            MutationState::Rejected {
                error: ApiError {
                    code: ErrorCode::RevisionMismatch,
                    ..
                },
                ..
            }
        ));
        let mut busy = request.clone();
        busy.request_id = "busy-after-enqueue".into();
        busy.expected_revision = applied.revision;
        mutate_connection_file(&path, busy.clone()).unwrap();
        app.editing.draft.as_mut().unwrap().name = "보존할 사람 입력".into();
        app.logic(&context, &mut frame);
        assert!(matches!(
            lookup_connection_file(&path, &first.session_id, &busy.request_id)
                .unwrap()
                .state,
            MutationState::Rejected {
                error: ApiError {
                    code: ErrorCode::EditorBusy,
                    ..
                },
                ..
            }
        ));
        assert_eq!(app.editing.draft.as_ref().unwrap().name, "보존할 사람 입력");
        app.synchronize_selection();
        app.publish_live(false);

        let mut expired = busy.clone();
        expired.request_id = "expired-before-logic".into();
        expired.timeout_ms = 1;
        mutate_connection_file(&path, expired.clone()).unwrap();
        std::thread::sleep(Duration::from_millis(3));
        app.logic(&context, &mut frame);
        assert!(matches!(
            lookup_connection_file(&path, &first.session_id, &expired.request_id)
                .unwrap()
                .state,
            MutationState::ExpiredBeforeApply
        ));
        assert_eq!(
            inspect_connection_file(&path).unwrap().revision,
            applied.revision
        );

        let mut old_project = busy;
        old_project.request_id = "old-project".into();
        mutate_connection_file(&path, old_project).unwrap();
        let (_other_directory, mut other, _) = crate::editing_tests::fixture();
        app.workspace.session = other.workspace.session.take();
        app.workspace.generation += 1;
        app.synchronize_selection();
        app.logic(&context, &mut frame);
        assert_eq!(
            app.workspace
                .session
                .as_ref()
                .unwrap()
                .document()
                .scene
                .entities[0]
                .position,
            Position { x: 20, y: 30 }
        );
        assert_ne!(
            inspect_connection_file(&path).unwrap().session_id,
            first.session_id
        );
        assert_eq!(
            lookup_connection_file(&path, &first.session_id, "old-project")
                .unwrap_err()
                .code,
            ErrorCode::SessionMismatch
        );
    }

    #[test]
    fn agent_changes_share_history_selection_and_save_without_committing_human_drafts() {
        let (_directory, mut app, id) = crate::editing_tests::fixture();
        let context = egui::Context::default();
        let session = app.workspace.session.as_ref().unwrap();
        let original = session.document().clone();
        app.editing.draft.as_mut().unwrap().name = "사람이 입력 중".into();
        let command = MutationCommand::RenameEntity {
            id,
            name: "에이전트 이름".into(),
        };
        assert_eq!(
            app.apply_agent_command(&context, &command)
                .unwrap_err()
                .code,
            ErrorCode::EditorBusy
        );
        assert_eq!(
            *app.workspace.session.as_ref().unwrap().document(),
            original
        );
        assert_eq!(app.editing.draft.as_ref().unwrap().name, "사람이 입력 중");
        app.synchronize_selection();
        app.apply_agent_command(&context, &command).unwrap();
        assert_eq!(app.editing.draft.as_ref().unwrap().name, "에이전트 이름");
        app.apply_agent_command(
            &context,
            &MutationCommand::MoveEntity {
                id,
                position: Position { x: 50, y: 60 },
            },
        )
        .unwrap();
        assert_eq!(
            app.editing.draft.as_ref().unwrap().position,
            Position { x: 50, y: 60 }
        );
        app.apply_agent_command(&context, &MutationCommand::Undo)
            .unwrap();
        assert_eq!(
            app.editing.draft.as_ref().unwrap().position,
            Position { x: 20, y: 30 }
        );
        app.apply_agent_command(&context, &MutationCommand::Redo)
            .unwrap();
        app.apply_agent_command(&context, &MutationCommand::Save)
            .unwrap();
        let session = app.workspace.session.as_ref().unwrap();
        let reopened = EditorSession::open(session.project_file()).unwrap();
        assert_eq!(*reopened.document(), *session.document());
        assert!(!session.is_dirty());
        app.apply_agent_command(&context, &MutationCommand::RemoveEntity { id })
            .unwrap();
        assert!(app.editing.selected.is_none());
        assert!(app.editing.draft.is_none());
    }

    #[test]
    fn agent_save_failure_preserves_document_history_and_dirty_state() {
        let (_directory, mut app, id) = crate::editing_tests::fixture();
        let context = egui::Context::default();
        app.apply_agent_command(
            &context,
            &MutationCommand::RenameEntity {
                id,
                name: "보존할 편집".into(),
            },
        )
        .unwrap();
        let session = app.workspace.session.as_ref().unwrap();
        let path = session.project_file().to_path_buf();
        let document = session.document().clone();
        std::fs::write(&path, "외부 수정").unwrap();
        assert!(
            app.apply_agent_command(&context, &MutationCommand::Save)
                .is_err()
        );
        let session = app.workspace.session.as_ref().unwrap();
        assert_eq!(*session.document(), document);
        assert!(session.is_dirty());
        assert!(session.can_undo());
        assert!(!app.native_io_active);
        assert_eq!(std::fs::read_to_string(path).unwrap(), "외부 수정");
    }
}
