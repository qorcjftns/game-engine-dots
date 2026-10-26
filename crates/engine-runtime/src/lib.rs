mod behavior;
pub use behavior::{BehaviorDiagnostic, BehaviorLog, BehaviorState, validate_behaviors};
mod play;
pub use play::{ActionInput, PlaySession, RunStatus, pixel_hash};
// GUI나 편집 이력을 소유하지 않는 실행 snapshot과 공통 논리 픽셀 출력.
mod raster;
use engine_document::{AssetId, EditorSession, PngPixels, ProjectDocument};
pub use raster::{BACKGROUND, render_scene};
use std::collections::HashMap;

/// 시작 시 문서와 디코드된 자산을 복제한다. 이후 파일 교체나 편집과 독립적이다.
pub struct RuntimeWorld {
    document: ProjectDocument,
    assets: HashMap<AssetId, PngPixels>,
    behaviors: behavior::Behaviors,
    visual_revision: u64,
}
impl RuntimeWorld {
    pub fn snapshot(session: &EditorSession) -> engine_document::Result<Self> {
        session.document().validate()?;
        // Editor cache에 더해 소유하는 디코드 자료를 256 MiB로 제한한다.
        let bytes = session.document().assets.iter().try_fold(0u64, |sum, a| {
            sum.checked_add(u64::from(a.width) * u64::from(a.height) * 4)
        });
        if bytes.is_none_or(|bytes| bytes > 256 * 1024 * 1024) {
            return Err(engine_document::DocumentError::InvalidDocument(
                "runtime PNG 디코드 예산 256 MiB를 초과했습니다.".into(),
            ));
        }
        let mut assets = HashMap::new();
        for asset in &session.document().assets {
            assets.insert(asset.id, session.read_png_asset(asset.id)?);
        }
        let behaviors = behavior::Behaviors::compile(session.document()).map_err(|error| {
            engine_document::DocumentError::InvalidDocument(format!(
                "{}:{}:{} 개체 {:?}: {}",
                error.file, error.line, error.column, error.entity_id, error.message
            ))
        })?;
        Ok(Self {
            behaviors,
            visual_revision: 0,
            document: session.document().clone(),
            assets,
        })
    }
    pub fn document(&self) -> &ProjectDocument {
        &self.document
    }
    pub fn visual_revision(&self) -> u64 {
        self.visual_revision
    }
    pub(crate) fn has_behaviors(&self) -> bool {
        self.behaviors.has_bindings()
    }
    pub(crate) fn behavior_states(&self) -> Vec<BehaviorState> {
        self.behaviors.states(&self.document)
    }
    pub(crate) fn tick_behaviors(
        &mut self,
        tick: u64,
        input: ActionInput,
        pressed: ActionInput,
        released: ActionInput,
    ) -> Result<Vec<BehaviorLog>, BehaviorDiagnostic> {
        let (changed, logs) =
            self.behaviors
                .tick(&mut self.document, tick, input, pressed, released)?;
        if changed {
            self.visual_revision = self.visual_revision.saturating_add(1);
        }
        Ok(logs)
    }
    pub fn render(&self) -> Vec<u8> {
        render_scene(&self.document, |id| self.assets.get(&id))
    }
}

/// 양 호스트에서 사용하는 물리 픽셀 정수 배율 camera fit. 작은 뷰는 축소하지 않는다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CameraViewport {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub scale: u32,
}
impl CameraViewport {
    pub fn fit(
        available_width: u32,
        available_height: u32,
        width: u32,
        height: u32,
    ) -> Option<Self> {
        if width == 0 || height == 0 {
            return None;
        }
        let scale = (available_width / width).min(available_height / height);
        if scale == 0 {
            return None;
        }
        let width = width * scale;
        let height = height * scale;
        Some(Self {
            x: (available_width - width) / 2,
            y: (available_height - height) / 2,
            width,
            height,
            scale,
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn snapshot_preserves_dirty_history_and_saved_bytes() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("project.json");
        let session = EditorSession::create(&path, "실행", 32, 18).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        let original = session.document().clone();
        for _ in 0..3 {
            let world = RuntimeWorld::snapshot(&session).unwrap();
            assert_eq!(world.document(), &original);
            assert_eq!(world.render(), BACKGROUND.repeat(32 * 18));
        }
        assert!(!session.is_dirty());
        assert!(!session.can_undo());
        assert_eq!(std::fs::read(path).unwrap(), bytes);
    }
    #[test]
    fn camera_preserves_integer_pixels_and_rejects_small_views() {
        assert_eq!(
            CameraViewport::fit(1001, 701, 320, 180).unwrap(),
            CameraViewport {
                x: 20,
                y: 80,
                width: 960,
                height: 540,
                scale: 3
            }
        );
        assert!(CameraViewport::fit(319, 180, 320, 180).is_none());
    }
}

#[cfg(test)]
mod asset_tests {
    use super::*;
    use engine_document::{EditCommand, Position};
    #[test]
    fn dirty_asset_snapshot_survives_source_loss_and_failure_preserves_editor() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.png");
        image::RgbaImage::from_pixel(1, 1, image::Rgba([250, 10, 20, 255]))
            .save(&source)
            .unwrap();
        let path = dir.path().join("project.json");
        let mut session = EditorSession::create(&path, "snapshot", 4, 4).unwrap();
        let id = session.import_png(&source).unwrap();
        session
            .execute(EditCommand::AddSprite {
                asset_id: id,
                name: "sprite".into(),
                position: Position { x: 0, y: 0 },
            })
            .unwrap();
        let before = session.document().clone();
        let saved = std::fs::read(&path).unwrap();
        let world = RuntimeWorld::snapshot(&session).unwrap();
        let pixels = world.render();
        std::fs::remove_file(dir.path().join(&before.assets[0].path)).unwrap();
        assert_eq!(world.render(), pixels);
        assert!(RuntimeWorld::snapshot(&session).is_err());
        assert_eq!(session.document(), &before);
        assert!(session.is_dirty());
        assert!(session.can_undo());
        assert!(!session.can_redo());
        assert_eq!(std::fs::read(path).unwrap(), saved);
    }
}

#[cfg(test)]
mod budget_tests {
    use super::*;
    #[test]
    fn oversized_asset_budget_is_rejected_before_reading_missing_pngs() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("p.json");
        let assets = (1..=5)
            .map(|id| {
                format!(r#"{{"id":{id},"path":"assets/{id}.png","width":4096,"height":4096}}"#)
            })
            .collect::<Vec<_>>()
            .join(",");
        let data = format!(
            r#"{{"schema_version":1,"name":"budget","logical_width":320,"logical_height":180,"assets":[{assets}],"scene":{{"entities":[]}}}}"#
        );
        std::fs::write(&path, &data).unwrap();
        let session = EditorSession::open(&path).unwrap();
        let error = RuntimeWorld::snapshot(&session).err().unwrap();
        assert!(error.to_string().contains("256 MiB"));
        assert!(!session.is_dirty());
        assert_eq!(std::fs::read_to_string(path).unwrap(), data);
    }
}
