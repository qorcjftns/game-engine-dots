//! 문서의 PNG 픽셀을 GUI 텍스처로 보관한다. 문서에는 GPU 자료를 넣지 않는다.

use std::collections::HashMap;

use eframe::egui::{ColorImage, Context, TextureHandle, TextureOptions};
use engine_document::{AssetId, EditorSession, PngPixels};

struct LoadedAsset {
    texture: TextureHandle,
    pixels: PngPixels,
}

#[derive(Default)]
pub struct ProjectTextures {
    assets: HashMap<AssetId, LoadedAsset>,
    pub issues: Vec<String>,
    pub generation: u64,
}

impl ProjectTextures {
    /// 문서를 새로 열거나 가져온 리소스가 바뀐 때 호출한다. 매 프레임 IO를 하지 않는다.
    pub fn reload(&mut self, context: &Context, session: &EditorSession) {
        self.assets.clear();
        self.issues.clear();
        self.generation = self.generation.wrapping_add(1);

        for asset in &session.document().assets {
            match session.read_png_asset(asset.id) {
                Ok(pixels) => {
                    let image = ColorImage::from_rgba_unmultiplied(
                        [pixels.width as usize, pixels.height as usize],
                        &pixels.rgba,
                    );
                    let texture = context.load_texture(
                        format!("project-asset-{}", asset.id.0),
                        image,
                        TextureOptions::NEAREST,
                    );
                    self.assets
                        .insert(asset.id, LoadedAsset { texture, pixels });
                }
                Err(error) => self.issues.push(format!("{}: {error}", asset.path)),
            }
        }
    }

    pub fn get(&self, asset_id: AssetId) -> Option<&TextureHandle> {
        self.assets.get(&asset_id).map(|asset| &asset.texture)
    }

    pub fn pixels(&self, asset_id: AssetId) -> Option<&PngPixels> {
        self.assets.get(&asset_id).map(|asset| &asset.pixels)
    }
}
