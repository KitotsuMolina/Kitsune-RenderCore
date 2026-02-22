use crate::config::RenderCoreConfig;
use crate::runtime::RenderRuntime;
use crate::video_map::{map_file_path_from_env, set_monitor_video};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub fn run() -> Result<(), String> {
    let args = std::env::args().collect::<Vec<_>>();
    match args.get(1).map(|s| s.as_str()) {
        Some("set-video") => return run_set_video(&args[2..]),
        Some("install-deps") => return run_script("install-deps.sh", &[]),
        Some("check-deps") => return run_script("check-deps.sh", &[]),
        Some("install-service") => return run_script("install-user-service.sh", &[]),
        Some("service") => return run_service(&args[2..]),
        Some("--help") | Some("-h") | Some("help") => {
            print_help();
            return Ok(());
        }
        _ => {}
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
                print_set_video_help();
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

fn run_service(args: &[String]) -> Result<(), String> {
    let action = args.first().map(|s| s.as_str()).unwrap_or("status");
    match action {
        "enable" => run_cmd(
            "systemctl",
            &["--user", "enable", "--now", "kitsune-rendercore.service"],
        ),
        "disable" => run_cmd(
            "systemctl",
            &["--user", "disable", "--now", "kitsune-rendercore.service"],
        ),
        "start" => run_cmd(
            "systemctl",
            &["--user", "start", "kitsune-rendercore.service"],
        ),
        "stop" => run_cmd(
            "systemctl",
            &["--user", "stop", "kitsune-rendercore.service"],
        ),
        "restart" => run_cmd(
            "systemctl",
            &["--user", "restart", "kitsune-rendercore.service"],
        ),
        "status" => run_cmd(
            "systemctl",
            &["--user", "status", "kitsune-rendercore.service"],
        ),
        "logs" => run_cmd(
            "journalctl",
            &["--user", "-u", "kitsune-rendercore.service", "-f"],
        ),
        "install" => run_script("install-user-service.sh", &[]),
        "--help" | "-h" | "help" => {
            print_service_help();
            Ok(())
        }
        other => Err(format!("unknown service action: {other}")),
    }
}

fn run_script(script_name: &str, extra_args: &[&str]) -> Result<(), String> {
    let path = find_script_path(script_name)
        .ok_or_else(|| format!("could not find script '{script_name}' in known locations"))?;
    let mut cmd = Command::new(path);
    cmd.args(extra_args);
    cmd.stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    let status = cmd
        .status()
        .map_err(|e| format!("failed to execute script {script_name}: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("script {script_name} exited with status: {status}"))
    }
}

fn run_cmd(bin: &str, args: &[&str]) -> Result<(), String> {
    let status = Command::new(bin)
        .args(args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|e| format!("failed to execute {bin}: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{bin} exited with status: {status}"))
    }
}

fn find_script_path(script_name: &str) -> Option<PathBuf> {
    let mut candidates = Vec::<PathBuf>::new();
    if let Ok(share) = std::env::var("KRC_SHARE_DIR") {
        candidates.push(Path::new(&share).join(script_name));
    }
    candidates.push(Path::new("/usr/share/kitsune-rendercore").join(script_name));

    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            // Source build: target/debug/kitsune-rendercore -> ../../scripts/*.sh
            candidates.push(exe_dir.join("../../scripts").join(script_name));
            // Optional packaged layout
            candidates.push(
                exe_dir
                    .join("../share/kitsune-rendercore")
                    .join(script_name),
            );
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("scripts").join(script_name));
    }

    candidates.into_iter().find(|p| p.is_file())
}

fn print_help() {
    println!("kitsune-rendercore - Wayland live wallpaper renderer");
    println!();
    println!("Usage:");
    println!("  kitsune-rendercore");
    println!("    Run renderer using current environment/configuration.");
    println!();
    println!(
        "  kitsune-rendercore set-video --monitor <MONITOR> --video <VIDEO_PATH> [--map-file <PATH>]"
    );
    println!("    Update a single monitor mapping for hot-reload without restarting the renderer.");
    println!();
    println!("  kitsune-rendercore check-deps");
    println!("    Validate runtime/build dependencies without installing anything.");
    println!();
    println!("  kitsune-rendercore install-deps");
    println!("    Install required dependencies for your distro (calls install-deps.sh).");
    println!();
    println!("  kitsune-rendercore install-service");
    println!("    Install user systemd service files and default env/map configs.");
    println!();
    println!(
        "  kitsune-rendercore service <install|enable|disable|start|stop|restart|status|logs>"
    );
    println!("    Manage the user systemd service.");
    println!();
    println!(
        "Run 'kitsune-rendercore service --help' or 'kitsune-rendercore set-video --help' for details."
    );
}

fn print_set_video_help() {
    println!("kitsune-rendercore set-video");
    println!("Usage:");
    println!(
        "  kitsune-rendercore set-video --monitor <MONITOR> --video <VIDEO_PATH> [--map-file <PATH>]"
    );
    println!();
    println!("Description:");
    println!("  Updates one monitor->video mapping in the map file.");
    println!("  If renderer is running, it reloads the changed mapping automatically.");
    println!();
    println!("Options:");
    println!("  --monitor <MONITOR>   Monitor name (e.g. DP-1, eDP-1, HDMI-A-1).");
    println!("  --video <VIDEO_PATH>  Absolute path to the video file.");
    println!("  --map-file <PATH>     Custom map file path.");
    println!();
    println!("Example:");
    println!(
        "  kitsune-rendercore set-video --monitor DP-1 --video /home/user/Videos/live/new.mp4"
    );
}

fn print_service_help() {
    println!("kitsune-rendercore service");
    println!("Usage:");
    println!(
        "  kitsune-rendercore service <install|enable|disable|start|stop|restart|status|logs>"
    );
    println!();
    println!("Actions:");
    println!("  install  Install service/env/map files for user session.");
    println!("  enable   Enable and start service now.");
    println!("  disable  Disable and stop service now.");
    println!("  start    Start service.");
    println!("  stop     Stop service.");
    println!("  restart  Restart service.");
    println!("  status   Show service status.");
    println!("  logs     Follow service logs (journalctl -f).");
}
