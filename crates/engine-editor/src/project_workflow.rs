//! GUI에서 발생한 파일 작업을 편집 세션에 연결한다. 실패하면 현재 세션을 보존한다.

use eframe::egui;
use engine_document::{AssetId, EditCommand, EditorSession, Position};

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
            new_project: NewProjectForm::default(),
        }
    }
}

impl ProjectWorkspace {
    /// 첫 연결 단계에서는 자동 저장하거나 변경을 버리지 않고 전환을 차단한다.
    pub fn allow_close_or_replace(&mut self) -> bool {
        if self.session.as_ref().is_some_and(EditorSession::is_dirty) {
            self.error = Some("미저장 변경이 있습니다. 먼저 저장한 뒤 다시 시도해 주세요.".into());
            false
        } else {
            true
        }
    }

    pub fn request_new_project(&mut self) {
        if self.allow_close_or_replace() {
            self.new_project.open = true;
        }
    }

    pub fn open_project(&mut self, context: &egui::Context) {
        if !self.allow_close_or_replace() {
            return;
        }
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

    pub fn save_project(&mut self) {
        let Some(session) = &mut self.session else {
            return;
        };
        match session.save() {
            Ok(()) => self.status = format!("저장 완료: {}", session.project_file().display()),
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    pub fn import_png(&mut self, context: &egui::Context) {
        let Some(session) = &mut self.session else {
            return;
        };
        let Some(path) = rfd::FileDialog::new()
            .add_filter("PNG 이미지", &["png"])
            .pick_file()
        else {
            return;
        };
        match session.import_png(&path) {
            Ok(_) => {
                self.textures.reload(context, session);
                self.status = "PNG를 가져왔습니다. 리소스의 ‘장면에 배치’를 눌러 주세요.".into();
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    pub fn add_sprite(&mut self, asset_id: AssetId) {
        let Some(session) = &mut self.session else {
            return;
        };
        let document = session.document();
        let Some(asset) = document.assets.iter().find(|asset| asset.id == asset_id) else {
            return;
        };
        let position = Position {
            x: (document.logical_width as i32 - asset.width as i32) / 2,
            y: (document.logical_height as i32 - asset.height as i32) / 2,
        };
        match session.execute(EditCommand::AddSprite {
            asset_id,
            name: format!("스프라이트 {}", asset_id.0),
            position,
        }) {
            Ok(_) => self.status = "스프라이트를 장면에 배치했습니다. 저장하여 보존하세요.".into(),
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    pub fn dialogs(&mut self, context: &egui::Context) {
        if let Some(request) = self.new_project.show(context) {
            self.create_project(context, request);
        }

        if let Some(message) = self.error.clone() {
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
        // 이름 입력 창을 연 뒤에도 기존 문서가 바뀌었을 수 있으므로 다시 확인한다.
        if !self.allow_close_or_replace() {
            return;
        }
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
    }
}
