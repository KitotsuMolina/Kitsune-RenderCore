use crate::config::RenderCoreConfig;
use crate::runtime::RenderRuntime;
use crate::video_map::{map_file_path_from_env, set_monitor_video};

pub fn run() -> Result<(), String> {
    let args = std::env::args().collect::<Vec<_>>();
    if args.get(1).map(|s| s.as_str()) == Some("set-video") {
        return run_set_video(&args[2..]);
    }

    let cfg = RenderCoreConfig::default();
    let mut runtime = RenderRuntime::new(cfg);
    runtime.bootstrap()?;
    runtime.run()
}

fn run_set_video(args: &[String]) -> Result<(), String> {
    let mut monitor = None::<String>;
    let mut video = None::<String>;
    let mut map_file = None::<String>;

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--monitor" => {
                i += 1;
                monitor = args.get(i).cloned();
            }
            "--video" => {
                i += 1;
                video = args.get(i).cloned();
            }
            "--map-file" => {
                i += 1;
                map_file = args.get(i).cloned();
            }
            "--help" | "-h" => {
                println!(
                    "Usage: kitsune-rendercore set-video --monitor <MONITOR> --video <VIDEO_PATH> [--map-file <PATH>]"
                );
                return Ok(());
            }
            unknown => {
                return Err(format!("unknown argument for set-video: {unknown}"));
            }
        }
        i += 1;
    }

    let monitor = monitor.ok_or_else(|| "missing --monitor".to_string())?;
    let video = video.ok_or_else(|| "missing --video".to_string())?;
    let map_path = map_file
        .map(std::path::PathBuf::from)
        .unwrap_or_else(map_file_path_from_env);

    set_monitor_video(&map_path, &monitor, &video)?;
    println!(
        "[ok] updated monitor mapping: {} -> {} (map={})",
        monitor,
        video,
        map_path.display()
    );
    println!("[ok] if renderer is running, it will reload this mapping automatically.");
    Ok(())
}
