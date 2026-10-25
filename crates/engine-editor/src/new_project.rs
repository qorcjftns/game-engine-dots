//! 새 프로젝트의 이름과 논리 해상도를 입력받는다. 파일 생성은 세션이 담당한다.

use eframe::egui;

pub struct NewProjectRequest {
    pub name: String,
    pub width: u32,
    pub height: u32,
}

pub struct NewProjectForm {
    pub open: bool,
    name: String,
    width: u32,
    height: u32,
}

impl Default for NewProjectForm {
    fn default() -> Self {
        Self {
            open: false,
            name: "새 프로젝트".to_owned(),
            width: 320,
            height: 180,
        }
    }
}

impl NewProjectForm {
    pub fn show(&mut self, context: &egui::Context) -> Option<NewProjectRequest> {
        if !self.open {
            return None;
        }
        let mut submitted = false;
        let mut cancelled = false;
        let response =
            egui::Modal::new(egui::Id::unique("new_project_dialog")).show(context, |ui| {
                ui.heading("새 프로젝트");
                ui.label("프로젝트 이름");
                ui.text_edit_singleline(&mut self.name);
                ui.add_space(10.0);
                ui.label("게임 화면의 논리 해상도");
                ui.horizontal(|ui| {
                    ui.add(
                        egui::DragValue::new(&mut self.width)
                            .range(1..=4096)
                            .prefix("가로 "),
                    );
                    ui.label("×");
                    ui.add(
                        egui::DragValue::new(&mut self.height)
                            .range(1..=4096)
                            .prefix("세로 "),
                    );
                });
                ui.small("이 크기는 게임 화면에만 적용됩니다. 패널과 글자 크기는 바뀌지 않습니다.");
                ui.add_space(12.0);
                submitted = ui
                    .add_enabled(
                        !self.name.trim().is_empty(),
                        egui::Button::new("저장 위치 선택…"),
                    )
                    .clicked();
                cancelled = ui.button("취소").clicked();
            });
        self.open = !submitted && !cancelled && !response.should_close();
        submitted.then(|| NewProjectRequest {
            name: self.name.clone(),
            width: self.width,
            height: self.height,
        })
    }
}
