#[cfg(feature = "wayland-layer")]
mod wayland_layer;
#[cfg(not(feature = "wayland-layer"))]
mod wayland_stub;

use crate::monitor::{MonitorInfo, MonitorSurfaceSpec};

pub trait LayerBackend {
    fn name(&self) -> &'static str;
    fn bootstrap(&mut self) -> Result<(), String>;
    fn discover_monitors(&mut self) -> Result<Vec<MonitorInfo>, String>;
    fn build_surfaces(
        &mut self,
        monitors: &[MonitorInfo],
    ) -> Result<Vec<MonitorSurfaceSpec>, String>;
    fn render_frame(&mut self, surfaces: &[MonitorSurfaceSpec]) -> Result<(), String>;
}

pub fn create_default_backend() -> Box<dyn LayerBackend> {
    #[cfg(feature = "wayland-layer")]
    {
        return Box::new(wayland_layer::WaylandLayerBackend::default());
    }

    #[cfg(not(feature = "wayland-layer"))]
    {
        Box::new(wayland_stub::WaylandLayerStubBackend::default())
    }
}
