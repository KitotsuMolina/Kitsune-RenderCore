use crate::backend::LayerBackend;
use crate::monitor::{LayerRole, MonitorInfo, MonitorSurfaceSpec};

#[derive(Default)]
pub struct WaylandLayerStubBackend {
    bootstrapped: bool,
}

impl LayerBackend for WaylandLayerStubBackend {
    fn name(&self) -> &'static str {
        "wayland-layer-stub"
    }

    fn bootstrap(&mut self) -> Result<(), String> {
        self.bootstrapped = true;
        println!("[backend:{}] bootstrap ok", self.name());
        Ok(())
    }

    fn discover_monitors(&mut self) -> Result<Vec<MonitorInfo>, String> {
        if !self.bootstrapped {
            return Err("backend not bootstrapped".to_string());
        }

        // Stub topology used until smithay-client-toolkit integration.
        Ok(vec![
            MonitorInfo {
                name: "DP-1".to_string(),
                width: 1920,
                height: 1080,
                refresh_hz: 60,
            },
            MonitorInfo {
                name: "HDMI-A-1".to_string(),
                width: 1920,
                height: 1080,
                refresh_hz: 60,
            },
        ])
    }

    fn build_surfaces(
        &mut self,
        monitors: &[MonitorInfo],
    ) -> Result<Vec<MonitorSurfaceSpec>, String> {
        if !self.bootstrapped {
            return Err("backend not bootstrapped".to_string());
        }

        let surfaces = monitors
            .iter()
            .cloned()
            .map(|m| MonitorSurfaceSpec {
                monitor: m,
                layer: LayerRole::Background,
            })
            .collect();
        Ok(surfaces)
    }

    fn render_frame(&mut self, surfaces: &[MonitorSurfaceSpec]) -> Result<(), String> {
        if !self.bootstrapped {
            return Err("backend not bootstrapped".to_string());
        }

        println!(
            "[backend:{}] render frame surfaces={}",
            self.name(),
            surfaces.len()
        );
        Ok(())
    }
}
