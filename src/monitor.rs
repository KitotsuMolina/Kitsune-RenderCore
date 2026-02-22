#[derive(Debug, Clone)]
pub struct MonitorInfo {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub refresh_hz: u32,
}

#[derive(Debug, Clone)]
pub struct MonitorSurfaceSpec {
    pub monitor: MonitorInfo,
    pub layer: LayerRole,
}

#[derive(Debug, Clone, Copy)]
pub enum LayerRole {
    Background,
}
