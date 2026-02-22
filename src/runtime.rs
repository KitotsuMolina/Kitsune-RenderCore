use std::thread;
use std::time::{Duration, Instant};

use crate::backend::{LayerBackend, create_default_backend};
use crate::config::RenderCoreConfig;
use crate::monitor::MonitorSurfaceSpec;
use crate::scheduler::FrameScheduler;
use crate::steam::SteamGameDetector;

pub struct RenderRuntime {
    config: RenderCoreConfig,
    backend: Box<dyn LayerBackend>,
    surfaces: Vec<MonitorSurfaceSpec>,
    scheduler: FrameScheduler,
    steam_detector: SteamGameDetector,
}

impl RenderRuntime {
    pub fn new(config: RenderCoreConfig) -> Self {
        let scheduler = FrameScheduler::new(config.target_fps);
        Self {
            config,
            backend: create_default_backend(),
            surfaces: Vec::new(),
            scheduler,
            steam_detector: SteamGameDetector::from_env(),
        }
    }

    pub fn bootstrap(&mut self) -> Result<(), String> {
        println!(
            "[rendercore] bootstrap: target_fps={} vsync={} pause_on_maximized={} max_frames={:?}",
            self.config.target_fps,
            self.config.use_vsync,
            self.config.pause_on_maximized,
            self.config.max_frames
        );
        self.backend.bootstrap()?;
        let monitors = self.backend.discover_monitors()?;
        self.surfaces = self.backend.build_surfaces(&monitors)?;
        println!(
            "[rendercore] backend={} monitors={}",
            self.backend.name(),
            monitors.len()
        );
        for surface in &self.surfaces {
            println!(
                "[rendercore] surface monitor={} {}x{}@{} layer={:?}",
                surface.monitor.name,
                surface.monitor.width,
                surface.monitor.height,
                surface.monitor.refresh_hz,
                surface.layer
            );
        }
        Ok(())
    }

    pub fn run(&mut self) -> Result<(), String> {
        println!(
            "[rendercore] scheduler frame_budget={:?}",
            self.scheduler.frame_budget()
        );
        if self.steam_detector.is_enabled() {
            println!("[rendercore] pause-on-steam-game enabled");
        }

        let mut frame: u64 = 0;
        let mut paused_for_steam = false;
        loop {
            if let Some(max) = self.config.max_frames {
                if frame >= max {
                    println!("[rendercore] reached max_frames={max}, exiting loop");
                    break;
                }
            }

            let game_running = self.steam_detector.steam_game_running();
            if game_running {
                if !paused_for_steam {
                    paused_for_steam = true;
                    println!("[rendercore] steam game detected -> pausing wallpaper render");
                }
                thread::sleep(Duration::from_millis(500));
                continue;
            }
            if paused_for_steam {
                paused_for_steam = false;
                println!("[rendercore] steam game closed -> resuming wallpaper render");
            }

            let frame_start = Instant::now();
            self.backend.render_frame(&self.surfaces)?;
            if frame % 120 == 0 {
                println!("[rendercore] frame={frame}");
            }
            frame += 1;

            let spent = frame_start.elapsed();
            if spent < self.scheduler.frame_budget() {
                thread::sleep(self.scheduler.frame_budget() - spent);
            }
        }
        Ok(())
    }
}
