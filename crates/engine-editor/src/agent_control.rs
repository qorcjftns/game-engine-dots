//! 문서 소유권을 넘기지 않고 확정 문서의 읽기 전용 사본만 게시한다.

use eframe::egui;
use engine_control::{MutationInbox, QueryServer, SessionTracker, SnapshotPublisher};
use engine_document::EditorSession;
use engine_protocol::LiveInspection;

use crate::{app::EditorApp, startup::AgentControlOptions};

pub(crate) struct LiveControl {
    // 서버를 앱 수명 동안 보관한다. drop 시 연결과 descriptor를 정리한다.
    _server: QueryServer,
    tracker: SessionTracker,
    publisher: SnapshotPublisher,
    context: egui::Context,
    pub(crate) inbox: MutationInbox,
}

impl LiveControl {
    fn start(
        path: &std::path::Path,
        context: &egui::Context,
        generation: u64,
        session: Option<&EditorSession>,
        busy: bool,
    ) -> engine_control::Result<Self> {
        let mut tracker = SessionTracker::new()?;
        let initial = tracker.observe(generation, session, busy)?;
        let server = QueryServer::start_with_snapshot(path, initial)?;
        let publisher = server.publisher();
        let wake_context = context.clone();
        let inbox = server.enable_mutations(move || wake_context.request_repaint())?;
        Ok(Self {
            _server: server,
            tracker,
            publisher,
            context: context.clone(),
            inbox,
        })
    }

    fn publish(
        &mut self,
        generation: u64,
        session: Option<&EditorSession>,
        busy: bool,
    ) -> engine_control::Result<LiveInspection> {
        let snapshot = self.tracker.observe(generation, session, busy)?;
        self.publisher.publish(snapshot.clone())?;
        Ok(snapshot)
    }
}

impl EditorApp {
    pub(crate) fn start_agent_control(
        &mut self,
        context: &egui::Context,
        options: AgentControlOptions,
    ) {
        let Some(path) = options.connection_file else {
            return;
        };
        // 이미 열린 프로젝트에는 비밀 파일을 잠깐이라도 만들지 않는다.
        if let Some(session) = &self.workspace.session {
            let external_parent = path.parent().and_then(|parent| parent.canonicalize().ok());
            let Some(parent) = external_parent else {
                self.workspace.error = Some("연결 파일의 상위 폴더가 존재해야 합니다. 프로젝트 밖의 사용자 전용 폴더를 지정하세요.".into());
                return;
            };
            if session
                .project_file()
                .parent()
                .is_some_and(|root| parent.starts_with(root))
            {
                self.workspace.error =
                    Some("연결 파일이 열린 프로젝트 안에 있어 연결을 시작하지 않았습니다.".into());
                return;
            }
        }
        match LiveControl::start(
            &path,
            context,
            self.workspace.generation,
            self.workspace.session.as_ref(),
            self.editor_busy(context),
        ) {
            Ok(control) => {
                self.live_control = Some(control);
                self.publish_live(false);
                if self.live_control.is_some() {
                    self.workspace.status = "에이전트의 로컬 공동 편집 연결을 시작했습니다.".into();
                }
            }
            Err(error) => {
                // 서버 시작 실패로 기존 편집 기능까지 잃지 않는다. 연결 성공으로도 표시하지 않는다.
                self.workspace.error =
                    Some(format!("에이전트 연결을 시작하지 못했습니다: {error}"));
            }
        }
    }

    pub(crate) fn editor_busy(&self, context: &egui::Context) -> bool {
        self.native_io_active
            || self.close_approved
            || self.pending_transition.is_some()
            || self.workspace.form_is_open()
            || self.show_about
            || self.workspace.error.is_some()
            || self.editing.drag.is_some()
            || context.text_edit_focused()
            || context.input(|input| input.pointer.any_down())
            || self
                .workspace
                .session
                .as_ref()
                .is_some_and(|session| self.editing.has_pending_changes(session.document()))
    }

    pub(crate) fn publish_live(&mut self, force_busy: bool) -> Option<LiveInspection> {
        let context = self
            .live_control
            .as_ref()
            .map(|control| control.context.clone())?;
        let busy = force_busy || self.editor_busy(&context);
        if let (Some(control), Some(session)) = (&self.live_control, &self.workspace.session)
            && let Some(project_root) = session.project_file().parent()
            && control._server.connection_file().starts_with(project_root)
        {
            self.live_control = None;
            self.workspace.error = Some("연결 파일이 열린 프로젝트 안에 있어 연결을 중단했습니다. 프로젝트 밖의 사용자 전용 경로로 다시 시작하세요.".into());
            return None;
        }
        let result = self.live_control.as_mut().unwrap().publish(
            self.workspace.generation,
            self.workspace.session.as_ref(),
            busy,
        );
        match result {
            Ok(snapshot) => Some(snapshot),
            Err(error) => {
                // 오래된 사본을 계속 제공하지 않는다. 문서는 유지하고 연결만 닫는다.
                self.live_control = None;
                self.workspace.error = Some(format!("에이전트 연결을 중단했습니다: {error}"));
                None
            }
        }
    }

    pub(crate) fn begin_native_io(&mut self) {
        self.native_io_active = true;
        self.publish_live(true);
    }

    pub(crate) fn end_native_io(&mut self) {
        self.native_io_active = false;
        self.publish_live(false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{editing::DragPreview, editor_actions::EditorAction};
    use engine_control::inspect_connection_file;
    use engine_document::{EditCommand, Position};

    #[test]
    fn transient_editor_work_is_busy_without_committing_the_document() {
        let (_directory, mut app, id) = crate::editing_tests::fixture();
        let context = egui::Context::default();
        assert!(!app.editor_busy(&context));
        let original = app.workspace.session.as_ref().unwrap().document().clone();
        app.editing.draft.as_mut().unwrap().name = "아직 입력 중".into();
        assert!(app.editor_busy(&context));
        assert_eq!(
            *app.workspace.session.as_ref().unwrap().document(),
            original
        );
        app.synchronize_selection();
        app.editing.drag = Some(DragPreview {
            id,
            original_position: Position { x: 20, y: 30 },
            position: Position { x: 21, y: 31 },
        });
        assert!(app.editor_busy(&context));
        app.synchronize_selection();
        app.begin_native_io();
        assert!(app.editor_busy(&context));
        app.end_native_io();
        assert!(!app.editor_busy(&context));
        app.workspace.request_new_project();
        assert!(app.editor_busy(&context));
        app.workspace.cancel_form();
        app.show_about = true;
        assert!(app.editor_busy(&context));
    }

    #[test]
    fn tcp_gui_logic_publishes_committed_edits_and_reads_while_gui_is_blocked() {
        use eframe::App;
        let (_project, mut app, id) = crate::editing_tests::fixture();
        let connection = tempfile::tempdir().unwrap();
        let path = connection.path().join("연결.json");
        let context = egui::Context::default();
        app.start_agent_control(
            &context,
            AgentControlOptions {
                connection_file: Some(path.clone()),
            },
        );
        assert!(app.live_control.is_some(), "{:?}", app.workspace.error);
        let first = inspect_connection_file(&path).unwrap();
        assert!(!first.busy);
        app.editing.draft.as_mut().unwrap().name = "공개하면 안 되는 입력".into();
        let mut frame = eframe::Frame::_new_kittest();
        app.logic(&context, &mut frame);
        let draft = inspect_connection_file(&path).unwrap();
        assert!(draft.busy);
        assert_eq!(draft.revision, first.revision);
        assert_eq!(draft.project, first.project);
        app.synchronize_selection();
        assert!(app.execute(EditCommand::MoveEntity {
            id,
            position: Position { x: 40, y: 50 }
        }));
        // 실제 commit_edits 경로처럼 확정한 좌표를 GUI 초안에도 반영한다.
        app.synchronize_selection();
        let changed = inspect_connection_file(&path).unwrap();
        assert!(changed.revision > first.revision);
        assert!(changed.is_dirty);
        assert_eq!(
            changed.project.as_ref().unwrap().document.scene.entities[0].position,
            Position { x: 40, y: 50 }
        );
        app.process_action(&context, EditorAction::Undo);
        let undone = inspect_connection_file(&path).unwrap();
        assert!(undone.revision > changed.revision);
        assert!(!undone.is_dirty);
        app.begin_native_io();
        // GUI가 추가 프레임을 전혀 처리하지 않아도 worker는 게시된 상태를 반환한다.
        let read_path = path.clone();
        let modal = std::thread::spawn(move || inspect_connection_file(&read_path))
            .join()
            .unwrap()
            .unwrap();
        assert!(modal.busy);
        assert_eq!(modal.project, undone.project);
        app.end_native_io();
        assert!(!inspect_connection_file(&path).unwrap().busy);
        let previous_id = undone.session_id;
        app.workspace.generation += 1;
        app.logic(&context, &mut frame);
        assert_ne!(
            inspect_connection_file(&path).unwrap().session_id,
            previous_id
        );
        drop(app);
        assert!(!path.exists());
    }

    #[test]
    fn tcp_later_opening_a_project_containing_the_descriptor_stops_control() {
        let (_project, mut fixture, _) = crate::editing_tests::fixture();
        let session = fixture.workspace.session.take().unwrap();
        let path = session
            .project_file()
            .parent()
            .unwrap()
            .join("connection.json");
        let mut app = EditorApp::default();
        app.start_agent_control(
            &egui::Context::default(),
            AgentControlOptions {
                connection_file: Some(path.clone()),
            },
        );
        assert!(app.live_control.is_some(), "{:?}", app.workspace.error);
        assert!(path.exists());
        app.workspace.session = Some(session);
        app.workspace.generation += 1;
        app.publish_live(false);
        assert!(app.live_control.is_none());
        assert!(!path.exists());
        assert!(
            app.workspace
                .error
                .as_ref()
                .unwrap()
                .contains("프로젝트 안")
        );
    }

    #[test]
    fn known_project_rejects_a_connection_file_before_starting_control() {
        let (_project, mut app, _) = crate::editing_tests::fixture();
        let path = app
            .workspace
            .session
            .as_ref()
            .unwrap()
            .project_file()
            .parent()
            .unwrap()
            .join("connection.json");
        app.start_agent_control(
            &egui::Context::default(),
            AgentControlOptions {
                connection_file: Some(path.clone()),
            },
        );
        assert!(app.live_control.is_none());
        assert!(
            app.workspace
                .error
                .as_ref()
                .unwrap()
                .contains("프로젝트 안")
        );
        assert!(!path.exists());
    }
}
