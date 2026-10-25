//! 마우스 이동은 preview만 갱신한다. 놓았을 때 MoveEntity 명령 한 건으로 확정한다.

use eframe::egui;
use engine_document::{EntityId, Position};

use crate::{app::EditorApp, editing::DragPreview, editor_actions::EditorAction};

impl EditorApp {
    pub(crate) fn quit_shortcut(&mut self, context: &egui::Context) {
        // 텍스트 입력·모달 중에도 종료 요청은 저장 확인으로 보낸다.
        if context.input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, egui::Key::Q)) {
            self.queued_action = Some(EditorAction::Quit);
        }
    }

    pub(crate) fn begin_drag(&mut self, id: EntityId) -> bool {
        if !self.commit_edits() {
            return false;
        }
        self.editing.selected = Some(id);
        self.synchronize_selection();
        if let Some(draft) = &self.editing.draft {
            self.editing.drag = Some(DragPreview {
                id,
                original_position: draft.position,
                position: draft.position,
            });
            true
        } else {
            false
        }
    }

    pub(crate) fn update_drag(&mut self, delta: egui::Vec2, points_per_pixel: f32) {
        if let Some(drag) = &mut self.editing.drag {
            let x = (delta.x / points_per_pixel).round() as i32;
            let y = (delta.y / points_per_pixel).round() as i32;
            drag.position = Position {
                x: drag.original_position.x.saturating_add(x),
                y: drag.original_position.y.saturating_add(y),
            };
        }
    }

    pub(crate) fn cancel_drag(&mut self) {
        self.editing.drag = None;
        self.editing.drag_origin = None;
        self.workspace.status = "드래그를 취소했습니다.".into();
    }

    pub(crate) fn handle_canvas(&mut self, context: &egui::Context) {
        if self.editing.drag.is_some()
            && context.input(|input| input.key_pressed(egui::Key::Escape) || !input.focused)
        {
            self.cancel_drag();
            self.canvas_interaction = None;
            return;
        }
        let Some(interaction) = self.canvas_interaction.take() else {
            return;
        };
        let Some(viewport) = interaction.viewport else {
            return;
        };
        let response = interaction.response;

        if response.drag_started() {
            let origin = context.input(|input| input.pointer.press_origin());
            let id = self
                .workspace
                .session
                .as_ref()
                .and_then(|session| viewport.hit_test(session.document(), origin?));
            if let Some(id) = id {
                if self.begin_drag(id) {
                    self.editing.drag_origin = origin;
                    context.memory_mut(|memory| memory.request_focus(response.id));
                }
            } else {
                self.queued_action = Some(EditorAction::Select(None));
            }
        }
        if (response.dragged() || response.drag_stopped())
            && let Some(origin) = self.editing.drag_origin
            && let Some(pointer) = context.input(|input| input.pointer.interact_pos())
        {
            // drag_delta는 프레임 증분이며 release에서는 0이다. 저장한 press 원점을 사용한다.
            self.update_drag(pointer - origin, viewport.points_per_pixel);
        }
        if response.drag_stopped() && self.editing.drag.is_some() {
            self.commit_edits();
        }
        if response.clicked() {
            let id = self.workspace.session.as_ref().and_then(|session| {
                viewport.hit_test(session.document(), response.interact_pointer_pos()?)
            });
            self.queued_action = Some(EditorAction::Select(id));
        }
    }

    pub(crate) fn keyboard_shortcuts(&mut self, context: &egui::Context) {
        if self.queued_action.is_some() {
            return;
        }
        if context.input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, egui::Key::S)) {
            self.queued_action = Some(EditorAction::Save);
            return;
        }
        // 텍스트·숫자 입력의 undo/Delete를 가로채지 않는다. 드래그 중 이력 조작도 막는다.
        if context.text_edit_focused() || self.editing.drag.is_some() {
            return;
        }
        let redo = egui::Modifiers {
            command: true,
            shift: true,
            ..Default::default()
        };
        self.queued_action = context.input_mut(|input| {
            if input.consume_key(redo, egui::Key::Z)
                || input.consume_key(egui::Modifiers::COMMAND, egui::Key::Y)
            {
                Some(EditorAction::Redo)
            } else if input.consume_key(egui::Modifiers::COMMAND, egui::Key::Z) {
                Some(EditorAction::Undo)
            } else if input.consume_key(egui::Modifiers::NONE, egui::Key::Delete)
                || input.consume_key(egui::Modifiers::NONE, egui::Key::Backspace)
            {
                Some(EditorAction::Delete)
            } else {
                None
            }
        });
    }
}
