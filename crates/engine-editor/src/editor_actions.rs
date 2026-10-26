//! 프레임 끝에서 편집 요청을 처리한다. 화면의 입력 중인 값도 저장 전에 확정한다.

use eframe::egui;
use engine_document::{AssetId, EditCommand, EntityId};

use crate::{app::EditorApp, editing::EditingState};

#[derive(Clone, Copy)]
pub enum EditorAction {
    NewProject,
    OpenProject,
    Save,
    ImportPng,
    AddSprite(AssetId),
    Select(Option<EntityId>),
    Undo,
    Redo,
    Delete,
    ResetDraft,
    Quit,
}

#[derive(Clone, Copy)]
pub enum Transition {
    NewProject,
    OpenProject,
    Quit,
}

#[derive(Clone, Copy)]
pub enum Confirmation {
    Save,
    Discard,
    Cancel,
}

impl EditorApp {
    pub(crate) fn handle_native_close(&mut self, context: &egui::Context) {
        if !context.input(|input| input.viewport().close_requested()) || self.close_approved {
            return;
        }
        if self.has_unsaved_changes() {
            context.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            if !matches!(self.pending_transition, Some(Transition::Quit)) {
                self.request_transition(context, Transition::Quit);
                // 최소화·가려짐 상태에서는 App.logic만 실행될 수 있다. 확인 창을 다시 보이게 한다.
                context.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                context.send_viewport_cmd(egui::ViewportCommand::Focus);
                context.request_repaint();
            }
        } else {
            // 이미 발생한 OS 닫기 요청은 취소하지 않고 그대로 허용한다.
            self.close_approved = true;
        }
    }

    pub(crate) fn has_unsaved_changes(&self) -> bool {
        self.workspace.session.as_ref().is_some_and(|session| {
            session.is_dirty()
                || self.editing.has_pending_changes(session.document())
                || self.behavior_pending()
        })
    }

    /// 입력 중인 이름과 좌표를 확정한다. 검증 실패 시 파일 저장도 진행하지 않는다.
    pub(crate) fn commit_edits(&mut self) -> bool {
        if !self.commit_behavior() {
            return false;
        }
        if let Some(drag) = self.editing.drag.take() {
            if !self.execute(EditCommand::MoveEntity {
                id: drag.id,
                position: drag.position,
            }) {
                self.editing.drag = Some(drag);
                return false;
            }
            if let Some(draft) = &mut self.editing.draft
                && draft.id == drag.id
            {
                draft.position = drag.position;
            }
        }
        let Some(draft) = self.editing.draft.clone() else {
            return true;
        };
        let entity = self.workspace.session.as_ref().and_then(|session| {
            session
                .document()
                .scene
                .entities
                .iter()
                .find(|entity| entity.id == draft.id)
                .cloned()
        });
        let Some(entity) = entity else {
            self.editing = EditingState::default();
            return true;
        };
        if draft.name != entity.name
            && !self.execute(EditCommand::RenameEntity {
                id: draft.id,
                name: draft.name,
            })
        {
            return false;
        }
        if draft.position != entity.position
            && !self.execute(EditCommand::MoveEntity {
                id: draft.id,
                position: draft.position,
            })
        {
            return false;
        }
        self.synchronize_selection();
        true
    }

    pub(crate) fn execute(&mut self, command: EditCommand) -> bool {
        if self.play.is_some() {
            return false;
        }
        let Some(session) = &mut self.workspace.session else {
            return false;
        };
        let success = match session.execute(command) {
            Ok(_) => {
                self.workspace.status = "편집 내용을 변경했습니다. 저장하여 보존하세요.".into();
                true
            }
            Err(error) => {
                self.workspace.error = Some(error.to_string());
                false
            }
        };
        self.publish_live(false);
        success
    }

    pub(crate) fn synchronize_selection(&mut self) {
        self.load_behavior(self.behavior_draft.as_ref().map(|d| d.id));
        if let Some(session) = &self.workspace.session {
            self.editing.synchronize(session.document());
        } else {
            self.editing = EditingState::default();
        }
    }

    pub(crate) fn process_action(&mut self, context: &egui::Context, action: EditorAction) {
        if self.play.is_some() && !matches!(action, EditorAction::Quit) {
            self.workspace.status = "Stop 후 편집·저장·프로젝트 전환을 할 수 있습니다.".into();
            return;
        }
        match action {
            EditorAction::NewProject => self.request_transition(context, Transition::NewProject),
            EditorAction::OpenProject => self.request_transition(context, Transition::OpenProject),
            EditorAction::Quit => self.request_transition(context, Transition::Quit),
            EditorAction::Save => {
                if self.commit_edits() {
                    self.save_with_publication();
                }
            }
            EditorAction::ResetDraft => {
                // 속성 패널의 취소는 그 패널의 입력만 취소한다. 행동 소스 draft는 별도로 보호한다.
                if let Some(session) = &self.workspace.session {
                    self.editing.synchronize(session.document());
                }
            }
            action => {
                if !self.commit_edits() {
                    return;
                }
                match action {
                    EditorAction::ImportPng => {
                        self.begin_native_io();
                        if self.workspace.import_png() {
                            //  새 문서를 먼저 게시하고 이후 디스크에서 텍스처를 읽는다.
                            self.publish_live(true);
                            if let Some(session) = &self.workspace.session {
                                self.workspace.textures.reload(context, session);
                            }
                        }
                        self.end_native_io();
                    }
                    EditorAction::AddSprite(asset_id) => {
                        self.editing.selected = self.workspace.add_sprite(asset_id);
                        self.publish_live(false);
                        self.synchronize_selection();
                    }
                    EditorAction::Select(id) => {
                        self.editing.selected = id;
                        self.synchronize_selection();
                    }
                    EditorAction::Delete => {
                        if let Some(id) = self.editing.selected {
                            self.execute(EditCommand::RemoveEntity { id });
                            self.synchronize_selection();
                        }
                    }
                    EditorAction::Undo | EditorAction::Redo => {
                        self.history_step(context, matches!(action, EditorAction::Redo))
                    }
                    _ => unreachable!("transition and save actions handled above"),
                }
            }
        }
    }

    fn history_step(&mut self, context: &egui::Context, redo: bool) {
        let mut reload_assets = false;
        if let Some(session) = &mut self.workspace.session {
            let old_assets = session.document().assets.clone();
            let changed = if redo { session.redo() } else { session.undo() };
            reload_assets = changed && old_assets != session.document().assets;
            self.workspace.status = if redo {
                "다시 실행했습니다."
            } else {
                "실행을 취소했습니다."
            }
            .into();
        }
        self.publish_live(reload_assets);
        if reload_assets {
            self.begin_native_io();
            if let Some(session) = &self.workspace.session {
                self.workspace.textures.reload(context, session);
            }
            self.end_native_io();
        }
        self.synchronize_selection();
        self.publish_live(false);
    }

    fn save_with_publication(&mut self) -> bool {
        self.begin_native_io();
        let saved = self.workspace.save_project();
        self.end_native_io();
        saved
    }

    fn request_transition(&mut self, context: &egui::Context, transition: Transition) {
        self.workspace.cancel_form();
        if self.has_unsaved_changes() {
            self.pending_transition = Some(transition);
        } else {
            self.perform_transition(context, transition);
        }
    }

    fn perform_transition(&mut self, context: &egui::Context, transition: Transition) {
        match transition {
            Transition::NewProject => self.workspace.request_new_project(),
            Transition::OpenProject => {
                self.begin_native_io();
                self.workspace.open_project(context);
                self.end_native_io();
            }
            Transition::Quit => {
                self.close_approved = true;
                context.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    pub(crate) fn confirm_transition(&mut self, context: &egui::Context, choice: Confirmation) {
        let Some(transition) = self.pending_transition else {
            return;
        };
        match choice {
            Confirmation::Save => {
                // 종료 저장은 실행 복사본을 버린 뒤 편집 원본만 저장한다.
                self.stop_play();
                if !self.commit_edits() || !self.save_with_publication() {
                    return;
                }
                self.pending_transition = None;
                self.perform_transition(context, transition);
            }
            Confirmation::Discard => {
                // saved 상태를 바꾸지 않는다. 파일 선택 취소·실패 시 기존 작업이 그대로 남는다.
                self.pending_transition = None;
                self.workspace.error = None;
                self.perform_transition(context, transition);
            }
            Confirmation::Cancel => {
                self.pending_transition = None;
                self.workspace.error = None;
            }
        }
    }

    pub(crate) fn unsaved_dialog(&mut self, context: &egui::Context) {
        if self.pending_transition.is_none() {
            return;
        }
        let mut choice = None;
        let response = egui::Modal::new(egui::Id::unique("unsaved_changes")).show(context, |ui| {
            ui.heading("변경을 저장할까요?");
            ui.label("입력 중인 속성을 포함해 저장하지 않은 변경이 있습니다.");
            if let Some(error) = &self.workspace.error {
                ui.colored_label(egui::Color32::from_rgb(220, 110, 130), error);
            }
            ui.horizontal(|ui| {
                if ui.button("저장 후 계속").clicked() {
                    choice = Some(Confirmation::Save);
                }
                if ui.button("변경 버리기").clicked() {
                    choice = Some(Confirmation::Discard);
                }
                if ui.button("취소").clicked() {
                    choice = Some(Confirmation::Cancel);
                }
            });
        });
        if response.should_close() {
            choice = Some(Confirmation::Cancel);
        }
        if let Some(choice) = choice {
            self.confirm_transition(context, choice);
        }
    }
}
