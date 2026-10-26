//! 작은 기준 렌더러. 논리 픽셀에 먼저 합성한 뒤 GUI에서 정수 배율로 확대한다.

use engine_document::{AssetId, PngPixels, ProjectDocument};

pub const BACKGROUND: [u8; 4] = [30, 35, 45, 255];

pub fn render_scene<'a>(
    document: &ProjectDocument,
    asset_pixels: impl Fn(AssetId) -> Option<&'a PngPixels>,
) -> Vec<u8> {
    let mut result = BACKGROUND.repeat((document.logical_width * document.logical_height) as usize);

    for entity in &document.scene.entities {
        let Some(pixels) = asset_pixels(entity.asset_id) else {
            continue;
        };
        let left = i64::from(entity.position.x);
        let top = i64::from(entity.position.y);
        let right = (left + i64::from(pixels.width)).clamp(0, i64::from(document.logical_width));
        let bottom = (top + i64::from(pixels.height)).clamp(0, i64::from(document.logical_height));

        for y in top.max(0)..bottom {
            for x in left.max(0)..right {
                let source = (((y - top) * i64::from(pixels.width) + x - left) * 4) as usize;
                let destination = ((y * i64::from(document.logical_width) + x) * 4) as usize;
                let alpha = u32::from(pixels.rgba[source + 3]);
                for channel in 0..3 {
                    let foreground = u32::from(pixels.rgba[source + channel]);
                    let background = u32::from(result[destination + channel]);
                    result[destination + channel] =
                        ((foreground * alpha + background * (255 - alpha) + 127) / 255) as u8;
                }
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use engine_document::{AssetId, EntityId, Position, Scene, SpriteEntity};

    use super::*;

    fn document(x: i32, y: i32) -> ProjectDocument {
        ProjectDocument {
            behaviors: vec![],
            schema_version: 2,
            name: "렌더링 검사".into(),
            logical_width: 2,
            logical_height: 2,
            assets: Vec::new(),
            scene: Scene {
                entities: vec![SpriteEntity {
                    behavior_id: None,
                    id: EntityId(1),
                    asset_id: AssetId(1),
                    name: "픽셀".into(),
                    position: Position { x, y },
                }],
            },
        }
    }

    #[test]
    fn transparent_pixels_leave_the_background_and_half_alpha_blends() {
        let pixels = PngPixels {
            width: 2,
            height: 1,
            rgba: vec![255, 0, 0, 0, 255, 0, 0, 128],
        };
        let result = render_scene(&document(0, 0), |_| Some(&pixels));
        assert_eq!(&result[..4], &BACKGROUND);
        assert_eq!(&result[4..8], &[143, 17, 22, 255]);
    }

    #[test]
    fn negative_position_clips_the_source_without_shifting_its_pixels() {
        let pixels = PngPixels {
            width: 2,
            height: 2,
            rgba: vec![
                255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 250, 240, 230, 255,
            ],
        };
        let result = render_scene(&document(-1, -1), |_| Some(&pixels));
        assert_eq!(&result[..4], &[250, 240, 230, 255]);
        assert_eq!(&result[4..], &BACKGROUND.repeat(3));
    }

    #[test]
    fn right_bottom_edge_and_fully_outside_positions_are_clipped() {
        let pixels = PngPixels {
            width: 2,
            height: 1,
            rgba: vec![1, 2, 3, 255, 4, 5, 6, 255],
        };
        let result = render_scene(&document(1, 1), |_| Some(&pixels));
        assert_eq!(&result[..12], &BACKGROUND.repeat(3));
        assert_eq!(&result[12..], &[1, 2, 3, 255]);
        assert_eq!(
            render_scene(&document(i32::MAX, i32::MIN), |_| Some(&pixels)),
            BACKGROUND.repeat(4)
        );
    }
}
