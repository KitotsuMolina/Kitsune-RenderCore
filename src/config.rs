#[derive(Debug, Clone)]
pub struct RenderCoreConfig {
    pub target_fps: u32,
    pub use_vsync: bool,
    pub pause_on_maximized: bool,
    pub max_frames: Option<u64>,
}

impl Default for RenderCoreConfig {
    fn default() -> Self {
        let max_frames = std::env::var("KRC_MAX_FRAMES")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .filter(|v| *v > 0);
        Self {
            target_fps: 60,
            use_vsync: true,
            pause_on_maximized: true,
            max_frames,
        }
    }
}
