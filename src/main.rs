mod app;
mod backend;
mod config;
#[cfg(feature = "wayland-layer")]
mod frame_source;
mod monitor;
mod runtime;
mod scheduler;
mod steam;
mod video_map;

fn main() {
    if let Err(err) = app::run() {
        eprintln!("rendercore error: {err}");
        std::process::exit(1);
    }
}
