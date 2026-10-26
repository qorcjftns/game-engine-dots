//! 문서 해상도에 먼저 합성한 게임 화면. 에디터 패널의 UI 배율과 분리한다.

use eframe::egui::{
    self, Color32, ColorImage, Pos2, Rect, Sense, Stroke, StrokeKind, TextureHandle,
    TextureOptions, Vec2,
};
use engine_document::{EntityId, Position, ProjectDocument};

use crate::{raster, resources::ProjectTextures};

#[derive(Default)]
pub struct ScenePreview {
    texture: Option<TextureHandle>,
    rendered_document: Option<ProjectDocument>,
    resource_generation: u64,
}

pub struct CanvasInteraction {
    pub response: egui::Response,
    pub viewport: Option<SceneViewport>,
}

impl ScenePreview {
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        document: &ProjectDocument,
        textures: &ProjectTextures,
        selected: Option<EntityId>,
        preview: Option<(EntityId, Position)>,
    ) -> CanvasInteraction {
        let mut display_document = document.clone();
        if let Some((id, position)) = preview
            && let Some(entity) = display_document
                .scene
                .entities
                .iter_mut()
                .find(|entity| entity.id == id)
        {
            entity.position = position;
        }
        self.prepare(ui.ctx(), &display_document, textures);
        let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        painter.rect_filled(response.rect, 0.0, Color32::from_rgb(17, 20, 26));

        let Some(viewport) = SceneViewport::fit(
            response.rect,
            ui.ctx().pixels_per_point(),
            document.logical_width,
            document.logical_height,
        ) else {
            painter.text(
                response.rect.center(),
                egui::Align2::CENTER_CENTER,
                "표시 공간이 작습니다. 창을 넓히거나 패널 크기를 줄여 주세요.",
                egui::FontId::proportional(13.0),
                Color32::LIGHT_GRAY,
            );
            return CanvasInteraction {
                response,
                viewport: None,
            };
        };

        if let Some(texture) = &self.texture {
            painter.with_clip_rect(viewport.rect).image(
                texture.id(),
                viewport.rect,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
        }
        // 선택 테두리는 GUI 오버레이이며 논리 화면 텍스처에는 합성하지 않는다.
        if let Some(id) = selected
            && let Some(entity) = display_document
                .scene
                .entities
                .iter()
                .find(|entity| entity.id == id)
            && let Some(asset) = display_document
                .assets
                .iter()
                .find(|asset| asset.id == entity.asset_id)
        {
            let rect = viewport.entity_rect(entity.position, asset.width, asset.height);
            painter.with_clip_rect(viewport.rect).rect_stroke(
                rect,
                0.0,
                Stroke::new(1.5, Color32::from_rgb(127, 188, 172)),
                StrokeKind::Inside,
            );
        }
        CanvasInteraction {
            response,
            viewport: Some(viewport),
        }
    }

    fn prepare(
        &mut self,
        context: &egui::Context,
        document: &ProjectDocument,
        textures: &ProjectTextures,
    ) {
        if self.rendered_document.as_ref() == Some(document)
            && self.resource_generation == textures.generation
        {
            return;
        }

        let pixels = raster::render_scene(document, |asset_id| textures.pixels(asset_id));
        let image = ColorImage::from_rgba_unmultiplied(
            [
                document.logical_width as usize,
                document.logical_height as usize,
            ],
            &pixels,
        );
        if let Some(texture) = &mut self.texture {
            texture.set(image, TextureOptions::NEAREST);
        } else {
            self.texture =
                Some(context.load_texture("logical-game-screen", image, TextureOptions::NEAREST));
        }
        self.rendered_document = Some(document.clone());
        self.resource_generation = textures.generation;
    }
}

pub struct SceneViewport {
    pub rect: Rect,
    pub points_per_pixel: f32,
}

impl SceneViewport {
    fn fit(bounds: Rect, pixels_per_point: f32, width: u32, height: u32) -> Option<Self> {
        if width == 0 || height == 0 || !pixels_per_point.is_finite() || pixels_per_point <= 0.0 {
            return None;
        }

        // UI 단위의 경계를 안쪽 물리 픽셀에 맞춘다. Retina에서도 정수 배율을 유지한다.
        let minimum = (bounds.min.to_vec2() * pixels_per_point).ceil();
        let maximum = (bounds.max.to_vec2() * pixels_per_point).floor();
        let available = (maximum - minimum).max(Vec2::ZERO);
        let camera = engine_runtime::CameraViewport::fit(
            available.x as u32,
            available.y as u32,
            width,
            height,
        )?;
        let scale = camera.scale as f32;
        let size = Vec2::new(camera.width as f32, camera.height as f32);
        let origin = minimum + Vec2::new(camera.x as f32, camera.y as f32);
        Some(Self {
            rect: Rect::from_min_size(
                (origin / pixels_per_point).to_pos2(),
                size / pixels_per_point,
            ),
            points_per_pixel: scale / pixels_per_point,
        })
    }

    fn entity_rect(&self, position: Position, width: u32, height: u32) -> Rect {
        Rect::from_min_size(
            self.rect.min + Vec2::new(position.x as f32, position.y as f32) * self.points_per_pixel,
            Vec2::new(width as f32, height as f32) * self.points_per_pixel,
        )
    }

    pub fn hit_test(&self, document: &ProjectDocument, pointer: Pos2) -> Option<EntityId> {
        if !self.rect.contains(pointer) {
            return None;
        }
        document.scene.entities.iter().rev().find_map(|entity| {
            let asset = document
                .assets
                .iter()
                .find(|asset| asset.id == entity.asset_id)?;
            self.entity_rect(entity.position, asset.width, asset.height)
                .contains(pointer)
                .then_some(entity.id)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_resolution_determines_the_canvas_size() {
        let bounds = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        let viewport = SceneViewport::fit(bounds, 1.0, 160, 144).unwrap();
        assert_eq!(viewport.rect.size(), Vec2::new(640.0, 576.0));
    }

    #[test]
    fn retina_scales_in_physical_pixels() {
        let bounds = Rect::from_min_size(Pos2::new(10.25, 20.25), Vec2::new(320.0, 180.0));
        let viewport = SceneViewport::fit(bounds, 2.0, 160, 90).unwrap();
        assert_eq!(viewport.rect.size(), Vec2::new(240.0, 135.0));
        assert_eq!((viewport.rect.min.x * 2.0).fract(), 0.0);
        assert_eq!((viewport.rect.min.y * 2.0).fract(), 0.0);
    }

    #[test]
    fn too_small_view_does_not_squash_pixels() {
        let bounds = Rect::from_min_size(Pos2::ZERO, Vec2::new(319.0, 180.0));
        assert!(SceneViewport::fit(bounds, 1.0, 320, 180).is_none());
    }
}
