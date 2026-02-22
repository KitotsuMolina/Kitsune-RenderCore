use std::time::Duration;

pub struct FrameScheduler {
    frame_budget: Duration,
}

impl FrameScheduler {
    pub fn new(target_fps: u32) -> Self {
        let safe_fps = target_fps.max(1);
        let frame_budget = Duration::from_nanos(1_000_000_000u64 / safe_fps as u64);
        Self { frame_budget }
    }

    pub fn frame_budget(&self) -> Duration {
        self.frame_budget
    }
}
