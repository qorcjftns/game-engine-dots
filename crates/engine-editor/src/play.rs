//! Play 수명은 편집 세션과 분리하며 GUI와 외부 제어가 같은 경로를 호출한다.
use crate::app::EditorApp;
use eframe::egui;
use engine_runtime::{PlaySession, RuntimeWorld};
impl EditorApp {
    pub(crate) fn apply_runtime_command(
        &mut self,
        context: &egui::Context,
        command: &engine_protocol::MutationCommand,
    ) -> Result<engine_protocol::MutationEffect, engine_protocol::ApiError> {
        use engine_protocol::{ApiError, ErrorCode, MutationCommand as C};
        let invalid = |message| ApiError::new(ErrorCode::InvalidRunState, message);
        if self.editor_busy(context) {
            return Err(ApiError::new(
                ErrorCode::EditorBusy,
                "대화상자와 입력을 마친 뒤 요청하세요.",
            ));
        }
        if matches!(command, C::RunStart) {
            self.start_play(context).map_err(invalid)?;
            return Ok(Default::default());
        }
        let requested = match command {
            C::RunPause { run_id, .. } | C::RunStep { run_id, .. } | C::RunStop { run_id } => {
                *run_id
            }
            _ => return Err(invalid("실행 명령이 아닙니다.".into())),
        };
        let run = self
            .play
            .as_mut()
            .ok_or_else(|| ApiError::new(ErrorCode::RunMismatch, "현재 실행 세션이 없습니다."))?;
        if run.status().run_id != requested {
            return Err(ApiError::new(
                ErrorCode::RunMismatch,
                "이전 실행 세션의 요청입니다.",
            ));
        }
        match command {
            C::RunPause { paused, .. } => run.pause(*paused),
            C::RunStep { ticks, input, .. } => {
                if !run.status().paused || !(1..=100000).contains(ticks) {
                    return Err(invalid("Pause 상태에서 1~100000 tick을 지정하세요.".into()));
                }
                run.step_with_input(*ticks, *input)
                    .map_err(|e| invalid(e.into()))?;
            }
            C::RunStop { .. } => self.stop_play(),
            _ => unreachable!(),
        }
        Ok(Default::default())
    }

    pub(crate) fn start_play(&mut self, context: &egui::Context) -> Result<(), String> {
        if self.play.is_some() {
            return Err("이미 실행 중입니다.".into());
        }
        if self.editor_busy(context) {
            return Err("입력과 대화상자를 마친 뒤 Play를 시작하세요.".into());
        }
        let session = self
            .workspace
            .session
            .as_ref()
            .ok_or("열린 프로젝트가 없습니다.")?;
        let world = RuntimeWorld::snapshot(session).map_err(|e| e.to_string())?;
        self.next_run_id = self
            .next_run_id
            .checked_add(1)
            .ok_or("실행 ID 범위를 초과했습니다.")?;
        self.play = Some(PlaySession::new(world, self.next_run_id));
        self.play_time = None;
        self.workspace.status =
            "Play · 게임 화면을 클릭하면 WASD/방향키·Space/마우스 입력을 확인합니다.".into();
        Ok(())
    }
    pub(crate) fn stop_play(&mut self) {
        self.play = None;
        self.play_time = None;
        self.workspace.status = "실행을 멈췄습니다. 편집을 계속할 수 있습니다.".into();
    }
    pub(crate) fn advance_play(&mut self, context: &egui::Context) {
        let Some(run) = &mut self.play else {
            return;
        };
        if !context.input(|i| i.focused)
            || context.text_edit_focused()
            || self.pending_transition.is_some()
            || self.show_about
        {
            run.clear_input();
            self.game_view.release_focus();
        }
        let now = std::time::Instant::now();
        if let Some(previous) = self.play_time.replace(now) {
            run.advance(now.duration_since(previous).as_secs_f64());
        }
        context.request_repaint_after(std::time::Duration::from_millis(16));
    }
    pub(crate) fn play_toolbar(&mut self, ui: &mut egui::Ui) {
        let mut start = false;
        let mut stop = false;
        ui.horizontal(|ui| {
            if let Some(run) = &mut self.play {
                if ui
                    .button(if run.status().paused {
                        "▶ Resume"
                    } else {
                        "Ⅱ Pause"
                    })
                    .clicked()
                {
                    run.pause(!run.status().paused);
                }
                if ui
                    .add_enabled(run.status().paused, egui::Button::new("1 tick"))
                    .clicked()
                {
                    let _ = run.step(1);
                }
                if ui.button("■ Stop").clicked() {
                    stop = true;
                }
                ui.label(format!(
                    "tick {} · frame {}",
                    run.status().tick,
                    run.status().frame
                ));
                let input = &run.status().input;
                ui.weak(format!(
                    "입력 ←{} →{} ↑{} ↓{} ●{}",
                    u8::from(input.left),
                    u8::from(input.right),
                    u8::from(input.up),
                    u8::from(input.down),
                    u8::from(input.primary)
                ));
            } else if ui
                .add_enabled(
                    self.workspace.session.is_some(),
                    egui::Button::new("▶ Play"),
                )
                .clicked()
            {
                start = true;
            }
        });
        if start && let Err(error) = self.start_play(ui.ctx()) {
            self.workspace.status = error;
        }
        if stop {
            self.stop_play();
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor_actions::EditorAction;
    use engine_document::{EditCommand, Position};
    #[test]
    fn play_preserves_dirty_history_and_blocks_document_actions() {
        let (_dir, mut app, id) = crate::editing_tests::fixture();
        let ctx = egui::Context::default();
        app.execute(EditCommand::MoveEntity {
            id,
            position: Position { x: 40, y: 50 },
        });
        app.synchronize_selection();
        let before = app.workspace.session.as_ref().unwrap().document().clone();
        let saved = std::fs::read(app.workspace.session.as_ref().unwrap().project_file()).unwrap();
        for _ in 0..3 {
            app.start_play(&ctx).unwrap();
            app.process_action(&ctx, EditorAction::Undo);
            app.process_action(&ctx, EditorAction::Save);
            assert!(!app.execute(EditCommand::RemoveEntity { id }));
            app.play.as_mut().unwrap().pause(true);
            app.play.as_mut().unwrap().step(73).unwrap();
            assert_eq!(app.play.as_ref().unwrap().status().tick, 73);
            app.stop_play();
        }
        let s = app.workspace.session.as_ref().unwrap();
        assert_eq!(s.document(), &before);
        assert!(s.is_dirty());
        assert!(s.can_undo());
        assert_eq!(std::fs::read(s.project_file()).unwrap(), saved);
        app.process_action(&ctx, EditorAction::Undo);
        assert!(app.workspace.session.as_ref().unwrap().can_redo());
        app.start_play(&ctx).unwrap();
        app.stop_play();
        assert!(app.workspace.session.as_ref().unwrap().can_redo());
    }
}

#[cfg(test)]
mod remote_tests {
    use super::*;
    use engine_control::{inspect_connection_file, lookup_connection_file, mutate_connection_file};
    use engine_protocol::{ErrorCode, MutationCommand as C, MutationRequest, MutationState};
    #[test]
    fn tcp_runtime_steps_deduplicate_and_old_runs_cannot_change_new_runs() {
        use eframe::App;
        let (_dir, mut app, _) = crate::editing_tests::fixture();
        let control = tempfile::tempdir().unwrap();
        let path = control.path().join("control.json");
        let ctx = egui::Context::default();
        app.start_agent_control(
            &ctx,
            crate::startup::AgentControlOptions {
                connection_file: Some(path.clone()),
            },
        );
        let first = inspect_connection_file(&path).unwrap();
        let request = |id: &str, command| MutationRequest {
            expected_session_id: first.session_id.clone(),
            expected_revision: first.revision,
            request_id: id.into(),
            timeout_ms: 5000,
            command,
        };
        let mut frame = eframe::Frame::_new_kittest();
        for r in [
            request("start", C::RunStart),
            request(
                "pause",
                C::RunPause {
                    run_id: 1,
                    paused: true,
                },
            ),
        ] {
            mutate_connection_file(&path, r.clone()).unwrap();
            app.logic(&ctx, &mut frame);
            assert!(matches!(
                lookup_connection_file(&path, &first.session_id, &r.request_id)
                    .unwrap()
                    .state,
                MutationState::Applied { .. }
            ));
        }
        let before = inspect_connection_file(&path)
            .unwrap()
            .runtime
            .unwrap()
            .tick;
        let step = request(
            "step",
            C::RunStep {
                run_id: 1,
                ticks: 73,
                input: Default::default(),
            },
        );
        mutate_connection_file(&path, step.clone()).unwrap();
        app.logic(&ctx, &mut frame);
        let receipt = lookup_connection_file(&path, &first.session_id, "step").unwrap();
        assert_eq!(mutate_connection_file(&path, step).unwrap(), receipt);
        app.logic(&ctx, &mut frame);
        assert_eq!(
            inspect_connection_file(&path)
                .unwrap()
                .runtime
                .unwrap()
                .tick,
            before + 73
        );
        for r in [
            request("stop", C::RunStop { run_id: 1 }),
            request("restart", C::RunStart),
        ] {
            mutate_connection_file(&path, r).unwrap();
            app.logic(&ctx, &mut frame);
        }
        mutate_connection_file(&path, request("stale", C::RunStop { run_id: 1 })).unwrap();
        app.logic(&ctx, &mut frame);
        assert!(matches!(
            lookup_connection_file(&path, &first.session_id, "stale")
                .unwrap()
                .state,
            MutationState::Rejected {
                error: engine_protocol::ApiError {
                    code: ErrorCode::RunMismatch,
                    ..
                },
                ..
            }
        ));
        assert_eq!(
            inspect_connection_file(&path)
                .unwrap()
                .runtime
                .unwrap()
                .run_id,
            2
        );
        assert_eq!(
            inspect_connection_file(&path).unwrap().revision,
            first.revision
        );
    }
}
