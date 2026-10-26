//! editor/player가 공유하는 egui 표시와 입력 변환. 편집 문서에는 접근하지 않는다.
use egui::{self, Color32, Pos2, Rect, Sense, Vec2};
use engine_runtime::{ActionInput, CameraViewport, PlaySession};
#[derive(Default)]
pub struct GameView {
    texture: Option<egui::TextureHandle>,
    run_id: Option<u64>,
    focused: bool,
    keys: std::collections::HashSet<egui::Key>,
    mouse: bool,
}
impl GameView {
    pub fn release_focus(&mut self) {
        self.keys.clear();
        self.mouse = false;
        self.focused = false;
    }
    pub fn show(&mut self, ui: &mut egui::Ui, run: &mut PlaySession, allow_input: bool) {
        let doc = run.world.document();
        let (width, height) = (doc.logical_width, doc.logical_height);
        if self.run_id != Some(run.status().run_id) {
            self.texture = Some(ui.ctx().load_texture(
                "runtime-screen",
                egui::ColorImage::from_rgba_unmultiplied(
                    [width as usize, height as usize],
                    &run.world.render(),
                ),
                egui::TextureOptions::NEAREST,
            ));
            self.run_id = Some(run.status().run_id);
            self.focused = false;
        }
        let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::click());
        painter.rect_filled(response.rect, 0.0, Color32::from_rgb(17, 20, 26));
        let ppp = ui.ctx().pixels_per_point();
        let min = (response.rect.min.to_vec2() * ppp).ceil();
        let max = (response.rect.max.to_vec2() * ppp).floor();
        let size = (max - min).max(Vec2::ZERO);
        let Some(camera) = CameraViewport::fit(size.x as u32, size.y as u32, width, height) else {
            self.keys.clear();
            self.mouse = false;
            self.focused = false;
            run.clear_input();
            painter.text(
                response.rect.center(),
                egui::Align2::CENTER_CENTER,
                "표시 공간이 작습니다. 창을 넓혀 주세요.",
                egui::FontId::proportional(14.0),
                Color32::WHITE,
            );
            return;
        };
        let rect = Rect::from_min_size(
            ((min + Vec2::new(camera.x as f32, camera.y as f32)) / ppp).to_pos2(),
            Vec2::new(camera.width as f32, camera.height as f32) / ppp,
        );
        painter.image(
            self.texture.as_ref().unwrap().id(),
            rect,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            Color32::WHITE,
        );
        for event in ui.input(|i| i.events.clone()) {
            if let egui::Event::PointerButton {
                pos, pressed: true, ..
            } = event
            {
                self.focused = rect.contains(pos);
                if self.focused {
                    response.request_focus();
                }
            }
        }
        let enabled = allow_input
            && self.focused
            && response.has_focus()
            && !ui.ctx().text_edit_focused()
            && ui.input(|i| i.focused);
        if enabled {
            let final_pointer = ui
                .input(|i| i.pointer.hover_pos())
                .filter(|p| rect.contains(*p))
                .map(|p| {
                    [
                        ((p.x - rect.min.x) * ppp / camera.scale as f32).floor() as i32,
                        ((p.y - rect.min.y) * ppp / camera.scale as f32).floor() as i32,
                    ]
                });
            let mut pointer = final_pointer;
            for event in ui.input(|i| i.events.clone()) {
                match event {
                    egui::Event::Key { key, pressed, .. } => {
                        if pressed {
                            self.keys.insert(key);
                        } else {
                            self.keys.remove(&key);
                        }
                    }
                    egui::Event::PointerButton {
                        button: egui::PointerButton::Primary,
                        pressed,
                        pos,
                        ..
                    } => {
                        pointer = rect.contains(pos).then(|| {
                            [
                                ((pos.x - rect.min.x) * ppp / camera.scale as f32).floor() as i32,
                                ((pos.y - rect.min.y) * ppp / camera.scale as f32).floor() as i32,
                            ]
                        });
                        self.mouse = pressed;
                    }
                    _ => {}
                }
                run.set_input(self.actions(pointer));
            }
            run.set_input(self.actions(final_pointer));
        } else {
            self.keys.clear();
            self.mouse = false;
            run.clear_input();
        }
        run.presented();
    }
    fn actions(&self, pointer: Option<[i32; 2]>) -> ActionInput {
        let held = |a, b| self.keys.contains(&a) || self.keys.contains(&b);
        ActionInput {
            left: held(egui::Key::A, egui::Key::ArrowLeft),
            right: held(egui::Key::D, egui::Key::ArrowRight),
            up: held(egui::Key::W, egui::Key::ArrowUp),
            down: held(egui::Key::S, egui::Key::ArrowDown),
            primary: self.keys.contains(&egui::Key::Space) || (self.mouse && pointer.is_some()),
            pointer,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn key(pressed: bool) -> egui::Event {
        egui::Event::Key {
            key: egui::Key::A,
            physical_key: Some(egui::Key::A),
            pressed,
            repeat: false,
            modifiers: Default::default(),
        }
    }
    fn click(pos: Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        }
    }
    fn frame(
        ctx: &egui::Context,
        view: &mut GameView,
        run: &mut PlaySession,
        events: Vec<egui::Event>,
        focused: bool,
        allow: bool,
    ) {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(640.0, 360.0))),
                focused,
                events,
                ..Default::default()
            },
            |ui| view.show(ui, run, allow),
        );
        output.textures_delta.clear();
    }
    #[test]
    fn real_egui_taps_keep_edges_and_suppressed_focus_clears_keys() {
        let dir = tempfile::tempdir().unwrap();
        let s = engine_document::EditorSession::create(&dir.path().join("p.json"), "p", 32, 18)
            .unwrap();
        let mut run = PlaySession::new(engine_runtime::RuntimeWorld::snapshot(&s).unwrap(), 1);
        run.pause(true);
        let ctx = egui::Context::default();
        let mut view = GameView::default();
        let inside = Pos2::new(320.0, 180.0);
        frame(&ctx, &mut view, &mut run, vec![], true, true);
        frame(
            &ctx,
            &mut view,
            &mut run,
            vec![
                egui::Event::PointerMoved(inside),
                click(inside, true),
                click(inside, false),
            ],
            true,
            true,
        );
        run.step(1).unwrap();
        frame(
            &ctx,
            &mut view,
            &mut run,
            vec![key(true), key(false)],
            true,
            true,
        );
        run.step(1).unwrap();
        assert!(run.status().pressed.left && run.status().released.left);
        assert!(!run.status().input.left);
        frame(
            &ctx,
            &mut view,
            &mut run,
            vec![
                click(inside, true),
                click(inside, false),
                egui::Event::PointerMoved(Pos2::new(900.0, 500.0)),
            ],
            true,
            true,
        );
        run.step(1).unwrap();
        assert!(run.status().pressed.primary && run.status().released.primary);
        assert_eq!(run.status().input.pointer, None);
        frame(&ctx, &mut view, &mut run, vec![key(true)], true, true);
        run.step(1).unwrap();
        assert!(run.status().input.left);
        frame(&ctx, &mut view, &mut run, vec![], false, true);
        run.step(1).unwrap();
        assert!(!run.status().input.left);
        frame(&ctx, &mut view, &mut run, vec![key(true)], true, false);
        run.step(1).unwrap();
        assert!(!run.status().input.left);
    }
}
