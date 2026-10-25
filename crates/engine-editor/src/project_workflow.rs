//! GUI에서 발생한 파일 작업을 편집 세션에 연결한다. 실패하면 현재 세션을 보존한다.

use eframe::egui;
use engine_document::{AssetId, EditCommand, EditorSession, EntityId, Position};

use crate::{
    canvas::ScenePreview,
    new_project::{NewProjectForm, NewProjectRequest},
    resources::ProjectTextures,
};

pub struct ProjectWorkspace {
    pub session: Option<EditorSession>,
    pub textures: ProjectTextures,
    pub preview: ScenePreview,
    pub status: String,
    pub error: Option<String>,
    pub generation: u64,
    new_project: NewProjectForm,
}

impl Default for ProjectWorkspace {
    fn default() -> Self {
        Self {
            session: None,
            textures: ProjectTextures::default(),
            preview: ScenePreview::default(),
            status: "프로젝트를 생성하거나 열어 주세요.".into(),
            error: None,
            generation: 0,
            new_project: NewProjectForm::default(),
        }
    }
}

impl ProjectWorkspace {
    pub fn form_is_open(&self) -> bool {
        self.new_project.open
    }

    pub fn cancel_form(&mut self) {
        self.new_project.open = false;
    }

    /// 전환 승인과 미저장 보호는 EditorApp의 공통 전환 경로에서 처리한다.
    pub fn request_new_project(&mut self) {
        self.new_project.open = true;
    }

    pub fn open_project(&mut self, context: &egui::Context) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Engine-1 프로젝트", &["json"])
            .pick_file()
        else {
            return;
        };
        match EditorSession::open(&path) {
            Ok(session) => self.replace_session(context, session),
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    pub fn save_project(&mut self) -> bool {
        let Some(session) = &mut self.session else {
            return false;
        };
        match session.save() {
            Ok(()) => {
                self.status = format!("저장 완료: {}", session.project_file().display());
                self.error = None;
                true
            }
            Err(error) => {
                self.error = Some(error.to_string());
                false
            }
        }
    }

    pub fn import_png(&mut self) -> bool {
        let Some(session) = &mut self.session else {
            return false;
        };
        let Some(path) = rfd::FileDialog::new()
            .add_filter("PNG 이미지", &["png"])
            .pick_file()
        else {
            return false;
        };
        match session.import_png(&path) {
            Ok(_) => {
                self.status = "PNG를 가져왔습니다. 리소스의 ‘장면에 배치’를 눌러 주세요.".into();
                true
            }
            Err(error) => {
                self.error = Some(error.to_string());
                false
            }
        }
    }

    pub fn add_sprite(&mut self, asset_id: AssetId) -> Option<EntityId> {
        let session = self.session.as_mut()?;
        let document = session.document();
        let asset = document.assets.iter().find(|asset| asset.id == asset_id)?;
        let position = Position {
            x: (document.logical_width as i32 - asset.width as i32) / 2,
            y: (document.logical_height as i32 - asset.height as i32) / 2,
        };
        match session.execute(EditCommand::AddSprite {
            asset_id,
            name: format!("스프라이트 {}", asset_id.0),
            position,
        }) {
            Ok(result) => {
                self.status = "스프라이트를 장면에 배치했습니다. 저장하여 보존하세요.".into();
                result.entity_id
            }
            Err(error) => {
                self.error = Some(error.to_string());
                None
            }
        }
    }

    pub fn dialogs(&mut self, context: &egui::Context, show_errors: bool) {
        if let Some(request) = self.new_project.show(context) {
            self.create_project(context, request);
        }

        if let Some(message) = self.error.clone().filter(|_| show_errors) {
            let mut open = true;
            let mut acknowledged = false;
            egui::Window::new("작업을 완료하지 못했습니다")
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .show(context, |ui| {
                    ui.label(message);
                    acknowledged = ui.button("확인").clicked();
                });
            if !open || acknowledged {
                self.error = None;
            }
        }
    }

    fn create_project(&mut self, context: &egui::Context, request: NewProjectRequest) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Engine-1 프로젝트", &["json"])
            .set_file_name("project.json")
            .save_file()
        else {
            return;
        };
        match EditorSession::create(&path, &request.name, request.width, request.height) {
            Ok(session) => self.replace_session(context, session),
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    fn replace_session(&mut self, context: &egui::Context, session: EditorSession) {
        self.textures.reload(context, &session);
        self.status = format!("프로젝트 열림: {}", session.project_file().display());
        self.preview = ScenePreview::default();
        self.session = Some(session);
        self.error = None;
        self.generation = self.generation.wrapping_add(1);
    }
}
