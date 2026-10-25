//! GUI 셸. 프로젝트 문서와 편집 명령은 후속 단계에서 연결한다.

use eframe::egui::{self, Align2, Color32, FontId, RichText, Sense, Stroke, StrokeKind, Vec2};
use engine_core::{LOGICAL_HEIGHT, LOGICAL_WIDTH, viewport::Viewport};

const ACCENT: Color32 = Color32::from_rgb(127, 188, 172);

pub struct EditorApp {
    show_scene: bool,
    show_inspector: bool,
    show_resources: bool,
    show_about: bool,
}

impl EditorApp {
    pub fn new(_creation: &eframe::CreationContext<'_>) -> Self {
        Self {
            show_scene: true,
            show_inspector: true,
            show_resources: true,
            show_about: false,
        }
    }

    fn menu(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("editor_menu")
            .resizable(false)
            .show(ui, |ui| {
                egui::MenuBar::new().ui(ui, |ui| {
                    ui.label(RichText::new("ENGINE-1").strong().color(ACCENT));
                    ui.separator();
                    ui.menu_button("파일", |ui| {
                        ui.add_enabled(false, egui::Button::new("새 프로젝트…"));
                        ui.add_enabled(false, egui::Button::new("프로젝트 열기…"));
                        ui.add_enabled(false, egui::Button::new("저장"));
                        ui.small("프로젝트 기능은 다음 단계에서 연결합니다.");
                        ui.separator();
                        if ui.button("에디터 종료").clicked() {
                            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    });
                    ui.menu_button("보기", |ui| {
                        ui.checkbox(&mut self.show_scene, "장면 패널");
                        ui.checkbox(&mut self.show_inspector, "속성 패널");
                        ui.checkbox(&mut self.show_resources, "리소스 패널");
                        if ui.button("기본 패널 배치").clicked() {
                            self.show_scene = true;
                            self.show_inspector = true;
                            self.show_resources = true;
                        }
                    });
                    ui.menu_button("도움말", |ui| {
                        if ui.button("에디터 정보").clicked() {
                            self.show_about = true;
                        }
                    });
                    ui.separator();
                    ui.weak("2D 게임 에디터");
                });
            });
    }

    fn toolbar(&self, ui: &mut egui::Ui) {
        egui::Panel::top("editor_toolbar")
            .resizable(false)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("프로젝트 없음").strong());
                    ui.separator();
                    ui.weak("아직 열린 장면이 없습니다");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_enabled(false, egui::Button::new("플레이"))
                            .on_hover_text("플레이 미리보기는 아직 연결되지 않았습니다.");
                        ui.label("편집 모드");
                    });
                });
            });
    }

    fn status_bar(&self, ui: &mut egui::Ui) {
        egui::Panel::bottom("editor_status")
            .resizable(false)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.small("GUI 셸 · 프로젝트 기능 연결 전");
                    ui.separator();
                    ui.small("macOS / Windows");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.small(format!(
                            "UI 배율 {:.0}%",
                            ui.ctx().pixels_per_point() * 100.0
                        ));
                    });
                });
            });
    }

    fn scene_panel(&self, ui: &mut egui::Ui) {
        if self.show_scene {
            egui::Panel::left("editor_scene")
                .default_size(210.0)
                .size_range(160.0..=360.0)
                .show(ui, |ui| {
                    panel_heading(ui, "장면", "SCENE");
                    ui.add_space(14.0);
                    ui.weak("열린 장면 없음");
                    ui.add_space(6.0);
                    ui.small("프로젝트를 열면 개체 목록이 표시됩니다.");
                });
        }
    }

    fn inspector_panel(&self, ui: &mut egui::Ui) {
        if self.show_inspector {
            egui::Panel::right("editor_inspector")
                .default_size(260.0)
                .size_range(200.0..=400.0)
                .show(ui, |ui| {
                    panel_heading(ui, "속성", "INSPECTOR");
                    ui.add_space(14.0);
                    ui.weak("선택한 개체 없음");
                    ui.add_space(6.0);
                    ui.small("개체의 이름과 위치 등 편집 속성이 표시될 영역입니다.");
                });
        }
    }

    fn resource_panel(&self, ui: &mut egui::Ui) {
        if self.show_resources {
            egui::Panel::bottom("editor_resources")
                .default_size(160.0)
                .size_range(100.0..=320.0)
                .show(ui, |ui| {
                    panel_heading(ui, "리소스", "RESOURCES");
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        ui.add_enabled(false, egui::Button::new("PNG 가져오기…"));
                        ui.weak("프로젝트 리소스가 없습니다.");
                    });
                    ui.add_space(6.0);
                    ui.small("이미지 가져오기와 장면 배치는 프로젝트 기능과 함께 연결합니다.");
                });
        }
    }

    fn viewport(&self, ui: &mut egui::Ui) {
        egui::CentralPanel::default().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("게임 뷰포트").strong());
                ui.weak(format!("기본 논리 화면 {LOGICAL_WIDTH} × {LOGICAL_HEIGHT}"));
            });
            ui.separator();

            let available = ui.available_size();
            let (response, painter) = ui.allocate_painter(available, Sense::hover());
            let bounds = response.rect;
            painter.rect_filled(bounds, 0.0, Color32::from_rgb(17, 20, 26));

            // egui의 좌표는 UI 단위다. 정수 배율은 물리 픽셀 기준으로 계산한다.
            let pixels_per_point = ui.ctx().pixels_per_point();
            let width = (bounds.width() * pixels_per_point).floor() as u32;
            let height = (bounds.height() * pixels_per_point).floor() as u32;
            if let Some(viewport) = Viewport::fit(width, height) {
                let origin =
                    bounds.min + Vec2::new(viewport.x as f32, viewport.y as f32) / pixels_per_point;
                let size =
                    Vec2::new(viewport.width as f32, viewport.height as f32) / pixels_per_point;
                let canvas = egui::Rect::from_min_size(origin, size);
                painter.rect_filled(canvas, 0.0, Color32::from_rgb(28, 33, 42));
                painter.rect_stroke(
                    canvas,
                    0.0,
                    Stroke::new(1.0, Color32::from_rgb(64, 73, 87)),
                    StrokeKind::Inside,
                );
                painter.text(
                    canvas.center() - Vec2::new(0.0, 12.0),
                    Align2::CENTER_CENTER,
                    "프로젝트를 열면 장면이 표시됩니다",
                    FontId::proportional(16.0),
                    Color32::from_rgb(156, 168, 184),
                );
                painter.text(
                    canvas.center() + Vec2::new(0.0, 16.0),
                    Align2::CENTER_CENTER,
                    "빈 뷰포트 · 게임 실행 화면이 아닙니다",
                    FontId::proportional(12.0),
                    Color32::from_rgb(114, 128, 147),
                );
            } else {
                painter.text(
                    bounds.center(),
                    Align2::CENTER_CENTER,
                    "뷰포트가 작습니다. 패널 크기를 줄이거나 창을 넓혀 주세요.",
                    FontId::proportional(13.0),
                    Color32::LIGHT_GRAY,
                );
            }
        });
    }
}

impl eframe::App for EditorApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.menu(ui);
        self.toolbar(ui);
        self.status_bar(ui);
        self.scene_panel(ui);
        self.inspector_panel(ui);
        self.resource_panel(ui);
        self.viewport(ui);

        egui::Window::new("Engine-1 정보")
            .open(&mut self.show_about)
            .resizable(false)
            .show(ui.ctx(), |ui| {
                ui.heading("Engine-1");
                ui.label("GUI 게임 에디터와 자체 2D 도트 엔진");
                ui.separator();
                ui.label("현재는 패널 배치와 게임 뷰포트를 확인하는 GUI 셸입니다.");
                ui.label("프로젝트 편집·저장·플레이 기능은 아직 연결되지 않았습니다.");
            });
    }
}

fn panel_heading(ui: &mut egui::Ui, title: &str, subtitle: &str) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(title).strong());
        ui.weak(subtitle);
    });
    ui.separator();
}
