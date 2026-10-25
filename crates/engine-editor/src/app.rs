//! 프로젝트와 편집 문서를 표시하는 GUI. 실제 변경은 편집 세션의 명령을 사용한다.

use eframe::egui::{self, Align2, Color32, FontId, RichText, Sense, Stroke, StrokeKind, Vec2};
use engine_core::{LOGICAL_HEIGHT, LOGICAL_WIDTH, viewport::Viewport};

use crate::project_workflow::ProjectWorkspace;
use crate::{
    canvas::CanvasInteraction,
    editing::EditingState,
    editor_actions::{EditorAction, Transition},
};

const ACCENT: Color32 = Color32::from_rgb(127, 188, 172);

pub struct EditorApp {
    pub(crate) live_control: Option<crate::agent_control::LiveControl>,
    pub(crate) native_io_active: bool,
    pub(crate) workspace: ProjectWorkspace,
    pub(crate) editing: EditingState,
    pub(crate) pending_transition: Option<Transition>,
    pub(crate) queued_action: Option<EditorAction>,
    pub(crate) close_approved: bool,
    pub(crate) commit_requested: bool,
    pub(crate) canvas_interaction: Option<CanvasInteraction>,
    show_scene: bool,
    show_inspector: bool,
    show_resources: bool,
    pub(crate) show_about: bool,
}

impl Default for EditorApp {
    fn default() -> Self {
        Self {
            live_control: None,
            native_io_active: false,
            workspace: ProjectWorkspace::default(),
            editing: EditingState::default(),
            pending_transition: None,
            queued_action: None,
            close_approved: false,
            commit_requested: false,
            canvas_interaction: None,
            show_scene: true,
            show_inspector: true,
            show_resources: true,
            show_about: false,
        }
    }
}

impl EditorApp {
    pub fn new(creation: &eframe::CreationContext<'_>) -> Self {
        configure_context(&creation.egui_ctx);
        Self::default()
    }

    fn menu(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("editor_menu")
            .resizable(false)
            .show(ui, |ui| {
                egui::MenuBar::new().ui(ui, |ui| {
                    ui.label(RichText::new("ENGINE-1").strong().color(ACCENT));
                    ui.separator();
                    ui.menu_button("파일", |ui| {
                        if ui.button("새 프로젝트…").clicked() {
                            self.queued_action = Some(EditorAction::NewProject);
                            ui.close();
                        }
                        if ui.button("프로젝트 열기…").clicked() {
                            self.queued_action = Some(EditorAction::OpenProject);
                            ui.close();
                        }
                        if ui
                            .add_enabled(
                                self.workspace.session.is_some(),
                                egui::Button::new("저장"),
                            )
                            .clicked()
                        {
                            self.queued_action = Some(EditorAction::Save);
                            ui.close();
                        }
                        ui.separator();
                        if ui.button("에디터 종료").clicked() {
                            self.queued_action = Some(EditorAction::Quit);
                        }
                    });
                    ui.menu_button("편집", |ui| {
                        let can_undo = self.workspace.session.as_ref().is_some_and(|session| {
                            session.can_undo()
                                || self.editing.has_pending_changes(session.document())
                        });
                        let can_redo = self
                            .workspace
                            .session
                            .as_ref()
                            .is_some_and(|session| session.can_redo());
                        if ui
                            .add_enabled(can_undo, egui::Button::new("실행 취소"))
                            .clicked()
                        {
                            self.queued_action = Some(EditorAction::Undo);
                            ui.close();
                        }
                        if ui
                            .add_enabled(can_redo, egui::Button::new("다시 실행"))
                            .clicked()
                        {
                            self.queued_action = Some(EditorAction::Redo);
                            ui.close();
                        }
                        if ui
                            .add_enabled(
                                self.editing.selected.is_some(),
                                egui::Button::new("개체 삭제"),
                            )
                            .clicked()
                        {
                            self.queued_action = Some(EditorAction::Delete);
                            ui.close();
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
                    if let Some(session) = &self.workspace.session {
                        ui.label(RichText::new(&session.document().name).strong());
                        if self.has_unsaved_changes() {
                            ui.label(RichText::new("● 미저장 변경").color(ACCENT));
                        }
                        ui.separator();
                        ui.weak(format!(
                            "개체 {}개",
                            session.document().scene.entities.len()
                        ));
                    } else {
                        ui.label(RichText::new("프로젝트 없음").strong());
                        ui.separator();
                        ui.weak("파일 메뉴에서 프로젝트를 생성하거나 여세요");
                    }
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
                    ui.small(&self.workspace.status);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.small(format!(
                            "UI 배율 {:.0}%",
                            ui.ctx().pixels_per_point() * 100.0
                        ));
                    });
                });
            });
    }

    fn scene_panel(&mut self, ui: &mut egui::Ui) {
        if self.show_scene {
            egui::Panel::left("editor_scene")
                .default_size(210.0)
                .size_range(160.0..=360.0)
                .show(ui, |ui| {
                    panel_heading(ui, "장면", "SCENE");
                    ui.add_space(14.0);
                    if let Some(session) = &self.workspace.session {
                        if session.document().scene.entities.is_empty() {
                            ui.weak("빈 장면");
                            ui.small("PNG 리소스를 가져와 장면에 배치하세요.");
                        }
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            for entity in &session.document().scene.entities {
                                if ui
                                    .selectable_label(
                                        self.editing.selected == Some(entity.id),
                                        &entity.name,
                                    )
                                    .clicked()
                                {
                                    self.queued_action =
                                        Some(EditorAction::Select(Some(entity.id)));
                                }
                                ui.small(format!(
                                    "위치 {}, {}",
                                    entity.position.x, entity.position.y
                                ));
                                ui.separator();
                            }
                        });
                    } else {
                        ui.weak("열린 장면 없음");
                        ui.add_space(6.0);
                        ui.small("프로젝트를 열면 개체 목록이 표시됩니다.");
                    }
                });
        }
    }

    fn resource_panel(&mut self, ui: &mut egui::Ui) {
        if self.show_resources {
            egui::Panel::bottom("editor_resources")
                .default_size(190.0)
                .size_range(100.0..=320.0)
                .show(ui, |ui| {
                    panel_heading(ui, "리소스", "RESOURCES");
                    if ui
                        .add_enabled(
                            self.workspace.session.is_some(),
                            egui::Button::new("PNG 가져오기…"),
                        )
                        .clicked()
                    {
                        self.queued_action = Some(EditorAction::ImportPng);
                    }
                    let assets = self
                        .workspace
                        .session
                        .as_ref()
                        .map(|session| session.document().assets.clone())
                        .unwrap_or_default();
                    if assets.is_empty() {
                        ui.weak("프로젝트 리소스가 없습니다.");
                    }
                    let mut add_asset = None;
                    egui::ScrollArea::horizontal().show(ui, |ui| {
                        ui.horizontal(|ui| {
                            for asset in assets {
                                ui.group(|ui| {
                                    ui.set_width(120.0);
                                    if let Some(texture) = self.workspace.textures.get(asset.id) {
                                        let aspect = asset.width as f32 / asset.height as f32;
                                        let size = Vec2::new(
                                            36.0 * aspect.min(1.0),
                                            36.0 / aspect.max(1.0),
                                        );
                                        ui.image((texture.id(), size));
                                    }
                                    ui.small(format!(
                                        "PNG #{} · {}×{}",
                                        asset.id.0, asset.width, asset.height
                                    ));
                                    if ui
                                        .add_enabled(
                                            self.workspace.textures.get(asset.id).is_some(),
                                            egui::Button::new("장면에 배치"),
                                        )
                                        .clicked()
                                    {
                                        add_asset = Some(asset.id);
                                    }
                                });
                            }
                        });
                    });
                    if let Some(asset_id) = add_asset {
                        self.queued_action = Some(EditorAction::AddSprite(asset_id));
                    }
                    for issue in &self.workspace.textures.issues {
                        ui.colored_label(Color32::from_rgb(220, 110, 130), issue);
                    }
                });
        }
    }

    fn viewport(&mut self, ui: &mut egui::Ui) {
        egui::CentralPanel::default().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("게임 뷰포트").strong());
                if let Some(session) = &self.workspace.session {
                    ui.weak(format!(
                        "논리 화면 {} × {}",
                        session.document().logical_width,
                        session.document().logical_height
                    ));
                } else {
                    ui.weak(format!("기본 논리 화면 {LOGICAL_WIDTH} × {LOGICAL_HEIGHT}"));
                }
            });
            ui.separator();

            if let Some(session) = &self.workspace.session {
                self.canvas_interaction = Some(self.workspace.preview.show(
                    ui,
                    session.document(),
                    &self.workspace.textures,
                    self.editing.selected,
                    self.editing.preview_position(session.document()),
                ));
                return;
            }

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
    fn logic(&mut self, context: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_native_close(context);
        self.publish_live(false);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let context = ui.ctx().clone();
        let generation = self.workspace.generation;
        self.handle_native_close(&context);
        self.menu(ui);
        self.toolbar(ui);
        self.status_bar(ui);
        self.scene_panel(ui);
        if self.show_inspector {
            self.inspector_panel(ui);
        }
        self.resource_panel(ui);
        self.viewport(ui);
        if !self.workspace.form_is_open() && self.pending_transition.is_none() && !self.show_about {
            if self.commit_requested {
                self.commit_edits();
                self.commit_requested = false;
            }
            self.handle_canvas(&context);
            self.keyboard_shortcuts(&context);
        } else {
            self.canvas_interaction = None;
        }
        self.quit_shortcut(&context);
        if let Some(action) = self.queued_action.take() {
            self.process_action(&context, action);
        }
        self.unsaved_dialog(&context);
        // 네이티브 대화상자 중에도 worker가 확정 문서와 busy를 반환할 수 있다.
        self.publish_live(false);
        self.workspace
            .dialogs(&context, self.pending_transition.is_none());
        if generation != self.workspace.generation {
            self.editing = EditingState::default();
        }
        self.publish_live(false);

        egui::Window::new("Engine-1 정보")
            .open(&mut self.show_about)
            .resizable(false)
            .show(ui.ctx(), |ui| {
                ui.heading("Engine-1");
                ui.label("GUI 게임 에디터와 자체 2D 도트 엔진");
                ui.separator();
                ui.label("프로젝트 생성·열기, PNG 가져오기·배치와 저장을 지원합니다.");
                ui.label("장면에서 개체를 선택하고 이름과 픽셀 위치를 편집할 수 있습니다.");
                ui.label("드래그·실행 취소·다시 실행과 미저장 변경 보호를 지원합니다.");
                ui.label("플레이 미리보기는 아직 연결되지 않았습니다.");
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

pub(crate) fn configure_context(context: &egui::Context) {
    // egui 기본 Command+Q도 프레임 끝에 Close를 직접 보낸다. 공통 저장 확인 경로만 사용한다.
    context.options_mut(|options| options.quit_shortcuts.clear());
}
