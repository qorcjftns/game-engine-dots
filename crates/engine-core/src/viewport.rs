//! 논리 화면을 늘이거나 자르지 않고 정수 배율로 중앙에 배치한다.

use crate::{LOGICAL_HEIGHT, LOGICAL_WIDTH};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Viewport {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub scale: u32,
}

impl Viewport {
    /// 1배 화면도 들어가지 않으면 표시할 수 없음을 반환한다.
    pub fn fit(window_width: u32, window_height: u32) -> Option<Self> {
        let scale = (window_width / LOGICAL_WIDTH).min(window_height / LOGICAL_HEIGHT);
        if scale == 0 {
            return None;
        }

        let width = LOGICAL_WIDTH * scale;
        let height = LOGICAL_HEIGHT * scale;
        Some(Self {
            x: (window_width - width) / 2,
            y: (window_height - height) / 2,
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
    fn exact_multiple_fills_the_window() {
        let viewport = Viewport::fit(960, 540).unwrap();
        assert_eq!(viewport.scale, 3);
        assert_eq!((viewport.x, viewport.y), (0, 0));
        assert_eq!((viewport.width, viewport.height), (960, 540));
    }

    #[test]
    fn odd_window_keeps_pixels_square_and_centers_within_one_pixel() {
        let viewport = Viewport::fit(1001, 701).unwrap();
        assert_eq!(viewport.scale, 3);
        assert_eq!((viewport.x, viewport.y), (20, 80));
        assert_eq!((viewport.width, viewport.height), (960, 540));
    }

    #[test]
    fn height_limits_scaling_in_a_wide_window() {
        let viewport = Viewport::fit(1920, 400).unwrap();
        assert_eq!(viewport.scale, 2);
        assert_eq!((viewport.x, viewport.y), (640, 20));
    }

    #[test]
    fn too_small_windows_do_not_use_fractional_scaling() {
        assert_eq!(Viewport::fit(319, 180), None);
        assert_eq!(Viewport::fit(320, 179), None);
        assert_eq!(Viewport::fit(0, 0), None);
        assert_eq!(Viewport::fit(320, 180).unwrap().scale, 1);
    }
}
