//! 프레임 속도와 게임 계산 속도를 분리한다.

use crate::TICKS_PER_SECOND;

const SECONDS_PER_TICK: f64 = 1.0 / TICKS_PER_SECOND as f64;
const MAX_FRAME_SECONDS: f64 = 0.25;
const MAX_TICKS_PER_FRAME: u32 = 8;

#[derive(Debug, Default)]
pub struct FixedClock {
    remaining_seconds: f64,
}

impl FixedClock {
    /// 이번 프레임에 처리할 틱 수. 긴 정지 후에는 오래된 틱을 버린다.
    pub fn advance(&mut self, elapsed_seconds: f64) -> u32 {
        if !elapsed_seconds.is_finite() || elapsed_seconds <= 0.0 {
            return 0;
        }

        self.remaining_seconds += elapsed_seconds.min(MAX_FRAME_SECONDS);
        let pending_ticks = (self.remaining_seconds / SECONDS_PER_TICK).floor() as u32;
        self.remaining_seconds -= f64::from(pending_ticks) * SECONDS_PER_TICK;
        pending_ticks.min(MAX_TICKS_PER_FRAME)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_thirty_fps_frame_produces_two_ticks() {
        assert_eq!(FixedClock::default().advance(1.0 / 30.0), 2);
    }

    #[test]
    fn short_frames_accumulate_until_a_tick_is_ready() {
        let mut clock = FixedClock::default();
        assert_eq!(clock.advance(SECONDS_PER_TICK / 2.0), 0);
        assert_eq!(clock.advance(SECONDS_PER_TICK / 2.0), 1);
    }

    #[test]
    fn a_long_pause_does_not_create_an_unbounded_catch_up() {
        let mut clock = FixedClock::default();
        assert_eq!(clock.advance(10.0), MAX_TICKS_PER_FRAME);
        assert_eq!(clock.advance(SECONDS_PER_TICK), 1);
    }

    #[test]
    fn invalid_time_does_not_poison_the_clock() {
        let mut clock = FixedClock::default();
        for elapsed in [f64::NAN, f64::INFINITY, -1.0, 0.0] {
            assert_eq!(clock.advance(elapsed), 0);
        }
        assert_eq!(clock.advance(SECONDS_PER_TICK), 1);
    }
}
