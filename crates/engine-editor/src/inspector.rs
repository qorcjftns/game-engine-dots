//! 속성 입력은 draft에 보관하고 Enter·포커스 종료·저장 시 편집 명령으로 확정한다.

use eframe::egui;

use crate::{app::EditorApp, editor_actions::EditorAction};

impl EditorApp {
    pub(crate) fn inspector_panel(&mut self, ui: &mut egui::Ui) {
        egui::Panel::right("editor_inspector")
            .default_size(260.0)
            .size_range(200.0..=400.0)
            .show(ui, |ui| {
                ui.strong("속성");
                ui.separator();
                ui.add_space(10.0);
                if let Some(draft) = &mut self.editing.draft {
                    ui.small(format!("개체 #{}", draft.id.0));
                    ui.label("이름");
                    let name = ui.add(
                        egui::TextEdit::singleline(&mut draft.name)
                            .id_salt(("entity_name", draft.id.0)),
                    );
                    ui.add_space(10.0);
                    ui.label("픽셀 위치");
                    let (x, y) = ui
                        .horizontal(|ui| {
                            let x = ui.add(
                                egui::DragValue::new(&mut draft.position.x)
                                    .speed(1.0)
                                    .prefix("X "),
                            );
                            let y = ui.add(
                                egui::DragValue::new(&mut draft.position.y)
                                    .speed(1.0)
                                    .prefix("Y "),
                            );
                            (x, y)
                        })
                        .inner;
                    let enter = ui.input(|input| input.key_pressed(egui::Key::Enter));
                    self.commit_requested |= name.lost_focus()
                        || x.lost_focus()
                        || y.lost_focus()
                        || x.drag_stopped()
                        || y.drag_stopped()
                        || (enter && (name.has_focus() || x.has_focus() || y.has_focus()));
                    ui.small("Enter 또는 다른 곳을 누르면 적용됩니다.");
                    ui.add_space(12.0);
                    if ui.button("입력 되돌리기").clicked() {
                        self.queued_action = Some(EditorAction::ResetDraft);
                        self.commit_requested = false;
                    }
                    if ui.button("개체 삭제").clicked() {
                        self.queued_action = Some(EditorAction::Delete);
                    }
                } else if let Some(session) = &self.workspace.session {
                    let document = session.document();
                    ui.strong("프로젝트");
                    ui.label(&document.name);
                    ui.label(format!(
                        "논리 화면 {} × {}",
                        document.logical_width, document.logical_height
                    ));
                    ui.label(format!("PNG 리소스 {}개", document.assets.len()));
                    ui.add_space(12.0);
                    ui.weak("장면 목록이나 뷰포트에서 개체를 선택하세요.");
                } else {
                    ui.weak("열린 프로젝트 없음");
                }
            });
    }
}
