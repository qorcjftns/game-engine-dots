//! Play 수명은 편집 세션과 분리하며 GUI와 외부 제어가 같은 경로를 호출한다.
use crate::app::EditorApp;
use eframe::egui;
use engine_runtime::{PlaySession, RuntimeWorld};
impl EditorApp {
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
        if start {
            if let Err(error) = self.start_play(ui.ctx()) {
                self.workspace.status = error;
            }
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
