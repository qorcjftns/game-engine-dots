//! 창이나 GPU 없이 실행하는 엔진 규칙. CLI와 데스크톱이 함께 사용한다.

pub mod clock;
pub mod viewport;

pub const LOGICAL_WIDTH: u32 = 320;
pub const LOGICAL_HEIGHT: u32 = 180;
pub const TICKS_PER_SECOND: u32 = 60;
