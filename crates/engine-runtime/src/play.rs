//! 실행 시간과 action 입력. 실시간 따라잡기 제한과 명시적 step을 분리한다.
use crate::RuntimeWorld;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionInput {
    pub left: bool,
    pub right: bool,
    pub up: bool,
    pub down: bool,
    pub primary: bool,
    pub pointer: Option<[i32; 2]>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunStatus {
    pub run_id: u64,
    pub paused: bool,
    pub tick: u64,
    pub frame: u64,
    pub input: ActionInput,
    pub pressed: ActionInput,
    pub released: ActionInput,
    pub frame_hash: String,
}
pub struct PlaySession {
    pub world: RuntimeWorld,
    status: RunStatus,
    pending: ActionInput,
    pressed: ActionInput,
    released: ActionInput,
    remaining: f64,
}
fn edges(a: ActionInput, b: ActionInput) -> ActionInput {
    ActionInput {
        left: a.left && !b.left,
        right: a.right && !b.right,
        up: a.up && !b.up,
        down: a.down && !b.down,
        primary: a.primary && !b.primary,
        pointer: None,
    }
}
fn merge(a: &mut ActionInput, b: ActionInput) {
    a.left |= b.left;
    a.right |= b.right;
    a.up |= b.up;
    a.down |= b.down;
    a.primary |= b.primary;
}
impl PlaySession {
    pub fn new(world: RuntimeWorld, run_id: u64) -> Self {
        let frame_hash = pixel_hash(&world.render());
        Self {
            world,
            status: RunStatus {
                run_id,
                paused: false,
                tick: 0,
                frame: 0,
                input: ActionInput::default(),
                pressed: ActionInput::default(),
                released: ActionInput::default(),
                frame_hash,
            },
            pending: ActionInput::default(),
            pressed: ActionInput::default(),
            released: ActionInput::default(),
            remaining: 0.0,
        }
    }
    pub fn status(&self) -> &RunStatus {
        &self.status
    }
    pub fn set_input(&mut self, input: ActionInput) {
        merge(&mut self.pressed, edges(input, self.pending));
        merge(&mut self.released, edges(self.pending, input));
        self.pending = input;
    }
    pub fn clear_input(&mut self) {
        self.set_input(ActionInput::default());
    }
    pub fn pause(&mut self, paused: bool) {
        self.status.paused = paused;
        self.remaining = 0.0;
        self.pending = ActionInput::default();
        self.pressed = ActionInput::default();
        self.released = ActionInput::default();
    }
    pub fn step(&mut self, count: u32) -> Result<(), &'static str> {
        if !self.status.paused {
            return Err("step에는 pause가 필요합니다.");
        }
        if !(1..=100_000).contains(&count) {
            return Err("tick 수는 1~100000이어야 합니다.");
        }
        if self.status.tick.checked_add(u64::from(count)).is_none() {
            return Err("tick 범위를 초과했습니다.");
        }
        for _ in 0..count {
            self.tick();
        }
        Ok(())
    }
    fn tick(&mut self) {
        self.status.tick += 1;
        self.status.input = self.pending;
        self.status.pressed = std::mem::take(&mut self.pressed);
        self.status.released = std::mem::take(&mut self.released);
    }
    pub fn advance(&mut self, seconds: f64) {
        if self.status.paused || !seconds.is_finite() || seconds <= 0.0 {
            return;
        }
        self.remaining += seconds.min(0.25);
        let count = (self.remaining * 60.0).floor() as u32;
        self.remaining -= f64::from(count) / 60.0;
        for _ in 0..count.min(8) {
            self.tick();
        }
    }
    /// 실제 호스트의 표시 요청 횟수. tick과 별개이며 GPU 완료를 의미하지 않는다.
    pub fn presented(&mut self) {
        self.status.frame = self.status.frame.saturating_add(1);
    }
}
pub fn pixel_hash(bytes: &[u8]) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash = (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}
#[cfg(test)]
mod tests {
    use super::*;
    fn run() -> PlaySession {
        let dir = tempfile::tempdir().unwrap();
        let s =
            engine_document::EditorSession::create(&dir.path().join("p.json"), "p", 2, 2).unwrap();
        PlaySession::new(RuntimeWorld::snapshot(&s).unwrap(), 1)
    }
    #[test]
    fn exact_step_is_not_realtime_catchup() {
        let mut r = run();
        r.advance(10.0);
        assert_eq!(r.status.tick, 8);
        r.pause(true);
        r.advance(10.0);
        r.step(61).unwrap();
        assert_eq!(r.status.tick, 69);
        assert!(r.step(0).is_err());
    }
    #[test]
    fn input_edges_survive_frames_without_ticks_and_focus_release() {
        let mut r = run();
        let i = ActionInput {
            left: true,
            ..Default::default()
        };
        r.set_input(i);
        r.advance(0.001);
        r.clear_input();
        r.advance(1.0 / 60.0);
        assert!(r.status.pressed.left);
        assert!(r.status.released.left);
        assert!(!r.status.input.left);
        r.advance(1.0 / 60.0);
        assert!(!r.status.pressed.left);
    }
}

#[cfg(test)]
mod pause_regression {
    use super::*;
    #[test]
    fn pause_discards_pending_taps() {
        let dir = tempfile::tempdir().unwrap();
        let s =
            engine_document::EditorSession::create(&dir.path().join("p.json"), "p", 2, 2).unwrap();
        let mut run = PlaySession::new(crate::RuntimeWorld::snapshot(&s).unwrap(), 1);
        run.set_input(ActionInput {
            left: true,
            ..Default::default()
        });
        run.pause(true);
        run.step(1).unwrap();
        assert!(!run.status().pressed.left);
        assert!(!run.status().released.left);
        assert!(!run.status().input.left);
    }
}
