use crate::config::RenderCoreConfig;
use crate::runtime::RenderRuntime;
use crate::steam::SteamGameDetector;
use crate::video_map::{
    map_file_path_from_env, parse_video_map_env, parse_video_map_file, set_monitor_video,
    unset_all_monitors, unset_monitor_video,
};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub fn run() -> Result<(), String> {
    let args = std::env::args().collect::<Vec<_>>();
    match args.get(1).map(|s| s.as_str()) {
        Some("set-video") => return run_set_video(&args[2..]),
        Some("unset-video") => return run_unset_video(&args[2..]),
        Some("status") => return run_status(&args[2..]),
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
    let mut all = false;
    let mut except_raw = None::<String>;

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--all" => {
                all = true;
            }
            "--monitor" => {
                i += 1;
                monitor = args.get(i).cloned();
            }
            "--video" => {
                i += 1;
                video = args.get(i).cloned();
            }
            "--except" => {
                i += 1;
                except_raw = args.get(i).cloned();
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

    let video = video.ok_or_else(|| "missing --video".to_string())?;
    let map_path = map_file
        .map(std::path::PathBuf::from)
        .unwrap_or_else(map_file_path_from_env);
    let except = except_raw
        .as_deref()
        .map(parse_csv_list)
        .unwrap_or_default();

    if all {
        let monitors = detect_monitor_names()?;
        if monitors.is_empty() {
            return Err("no monitors found via hyprctl".to_string());
        }
        let mut applied = 0usize;
        for m in &monitors {
            if except.iter().any(|x| x == m) {
                println!("[ok] skipped monitor by --except: {}", m);
                continue;
            }
            set_monitor_video(&map_path, m, &video)?;
            println!("[ok] updated monitor mapping: {} -> {}", m, video);
            applied += 1;
        }
        println!(
            "[ok] updated {} monitors (detected={}, map={})",
            applied,
            monitors.len(),
            map_path.display()
        );
    } else {
        if !except.is_empty() {
            return Err("--except requires --all".to_string());
        }
        let monitor = monitor.ok_or_else(|| "missing --monitor (or use --all)".to_string())?;
        set_monitor_video(&map_path, &monitor, &video)?;
        println!(
            "[ok] updated monitor mapping: {} -> {} (map={})",
            monitor,
            video,
            map_path.display()
        );
    }
    println!("[ok] if renderer is running, it will reload this mapping automatically.");
    Ok(())
}

fn run_unset_video(args: &[String]) -> Result<(), String> {
    let mut monitor = None::<String>;
    let mut map_file = None::<String>;
    let mut all = false;
    let mut except_raw = None::<String>;

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--all" => {
                all = true;
            }
            "--monitor" => {
                i += 1;
                monitor = args.get(i).cloned();
            }
            "--except" => {
                i += 1;
                except_raw = args.get(i).cloned();
            }
            "--map-file" => {
                i += 1;
                map_file = args.get(i).cloned();
            }
            "--help" | "-h" => {
                print_unset_video_help();
                return Ok(());
            }
            unknown => return Err(format!("unknown argument for unset-video: {unknown}")),
        }
        i += 1;
    }

    let map_path = map_file
        .map(std::path::PathBuf::from)
        .unwrap_or_else(map_file_path_from_env);
    let except = except_raw
        .as_deref()
        .map(parse_csv_list)
        .unwrap_or_default();

    if all {
        let removed = unset_all_monitors(&map_path, &except)?;
        println!(
            "[ok] removed {} mappings via --all (kept {} via --except, map={})",
            removed,
            except.len(),
            map_path.display()
        );
    } else {
        if !except.is_empty() {
            return Err("--except requires --all".to_string());
        }
        let monitor = monitor.ok_or_else(|| "missing --monitor (or use --all)".to_string())?;
        let removed = unset_monitor_video(&map_path, &monitor)?;
        if removed {
            println!(
                "[ok] removed monitor mapping: {} (map={})",
                monitor,
                map_path.display()
            );
        } else {
            println!(
                "[ok] mapping not present for monitor: {} (map={})",
                monitor,
                map_path.display()
            );
        }
    }
    println!("[ok] if renderer is running, it will reload this mapping automatically.");
    Ok(())
}

fn run_status(args: &[String]) -> Result<(), String> {
    let mut as_json = false;
    let mut json_pretty = true;
    let mut out_file = None::<String>;
    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--json" => as_json = true,
            "--pretty" => json_pretty = true,
            "--compact" => json_pretty = false,
            "--file" => {
                i += 1;
                out_file = args.get(i).cloned();
            }
            "--help" | "-h" => {
                print_status_help();
                return Ok(());
            }
            other => return Err(format!("unknown argument for status: {other}")),
        }
        i += 1;
    }
    if out_file.is_some() && !as_json {
        return Err("--file requires --json".to_string());
    }

    let map_path = map_file_path_from_env();
    let file_map = parse_video_map_file(&map_path);
    let env_map = std::env::var("KRC_VIDEO_MAP")
        .ok()
        .map(|v| parse_video_map_env(&v))
        .unwrap_or_default();
    let default_video = std::env::var("KRC_VIDEO_DEFAULT")
        .ok()
        .or_else(|| std::env::var("KRC_VIDEO").ok());
    let mut steam = SteamGameDetector::from_env();
    let steam_running = steam.steam_game_running();
    let fps = std::env::var("KRC_VIDEO_FPS").unwrap_or_else(|_| "30".to_string());
    let speed = std::env::var("KRC_VIDEO_SPEED").unwrap_or_else(|_| "1.0".to_string());
    let quality = std::env::var("KRC_QUALITY").unwrap_or_else(|_| "default".to_string());
    let hwaccel = std::env::var("KRC_HWACCEL").unwrap_or_else(|_| "auto".to_string());

    let service_state = if let Ok(active) = run_cmd_capture(
        "systemctl",
        &["--user", "is-active", "kitsune-rendercore.service"],
    ) {
        active.trim().to_string()
    } else {
        "<unknown>".to_string()
    };

    let monitors = detect_monitor_names().unwrap_or_default();
    let mut mapped = Vec::<(String, String)>::new();
    for m in &monitors {
        let selected = file_map
            .get(m)
            .cloned()
            .or_else(|| env_map.get(m).cloned())
            .or_else(|| default_video.clone())
            .unwrap_or_else(|| "<none>".to_string());
        mapped.push((m.clone(), selected));
    }

    if as_json {
        let out = build_status_json(
            &map_path.display().to_string(),
            default_video.as_deref().unwrap_or("<none>"),
            &fps,
            &speed,
            &quality,
            &hwaccel,
            steam.is_enabled(),
            steam_running,
            &service_state,
            &mapped,
            json_pretty,
        );
        if let Some(path) = out_file {
            let p = std::path::PathBuf::from(&path);
            if let Some(parent) = p.parent() {
                if !parent.as_os_str().is_empty() {
                    std::fs::create_dir_all(parent).map_err(|e| {
                        format!(
                            "failed to create parent directory {}: {e}",
                            parent.display()
                        )
                    })?;
                }
            }
            std::fs::write(&p, out)
                .map_err(|e| format!("failed to write status file {}: {e}", p.display()))?;
            println!("[ok] wrote status json: {}", p.display());
        } else {
            println!("{}", out);
        }
        return Ok(());
    }

    println!("kitsune-rendercore status");
    println!("map_file={}", map_path.display());
    println!(
        "default_video={}",
        default_video.as_deref().unwrap_or("<none>")
    );
    println!(
        "runtime_cfg: fps={} speed={} quality={} hwaccel={}",
        fps, speed, quality, hwaccel
    );
    println!("steam_pause_enabled={}", steam.is_enabled());
    println!("steam_game_running={}", steam_running);
    println!("service_state={}", service_state);
    if monitors.is_empty() {
        println!("monitors=<unavailable>");
    } else {
        println!("monitors:");
        for (m, selected) in mapped {
            println!("  {} -> {}", m, selected);
        }
    }
    Ok(())
}

fn build_status_json(
    map_file: &str,
    default_video: &str,
    fps: &str,
    speed: &str,
    quality: &str,
    hwaccel: &str,
    steam_pause_enabled: bool,
    steam_game_running: bool,
    service_state: &str,
    mapped: &[(String, String)],
    pretty: bool,
) -> String {
    if pretty {
        let mut out = String::new();
        out.push_str("{\n");
        out.push_str(&format!("  \"map_file\": \"{}\",\n", escape_json(map_file)));
        out.push_str(&format!(
            "  \"default_video\": \"{}\",\n",
            escape_json(default_video)
        ));
        out.push_str("  \"runtime\": {\n");
        out.push_str(&format!("    \"fps\": \"{}\",\n", escape_json(fps)));
        out.push_str(&format!("    \"speed\": \"{}\",\n", escape_json(speed)));
        out.push_str(&format!("    \"quality\": \"{}\",\n", escape_json(quality)));
        out.push_str(&format!("    \"hwaccel\": \"{}\"\n", escape_json(hwaccel)));
        out.push_str("  },\n");
        out.push_str(&format!(
            "  \"steam_pause_enabled\": {},\n",
            steam_pause_enabled
        ));
        out.push_str(&format!(
            "  \"steam_game_running\": {},\n",
            steam_game_running
        ));
        out.push_str(&format!(
            "  \"service_state\": \"{}\",\n",
            escape_json(service_state)
        ));
        out.push_str("  \"monitors\": [\n");
        for (idx, (m, v)) in mapped.iter().enumerate() {
            let comma = if idx + 1 == mapped.len() { "" } else { "," };
            out.push_str(&format!(
                "    {{\"name\":\"{}\",\"video\":\"{}\"}}{}\n",
                escape_json(m),
                escape_json(v),
                comma
            ));
        }
        out.push_str("  ]\n");
        out.push('}');
        return out;
    }

    let monitors_json = mapped
        .iter()
        .map(|(m, v)| {
            format!(
                "{{\"name\":\"{}\",\"video\":\"{}\"}}",
                escape_json(m),
                escape_json(v)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"map_file\":\"{}\",\"default_video\":\"{}\",\"runtime\":{{\"fps\":\"{}\",\"speed\":\"{}\",\"quality\":\"{}\",\"hwaccel\":\"{}\"}},\"steam_pause_enabled\":{},\"steam_game_running\":{},\"service_state\":\"{}\",\"monitors\":[{}]}}",
        escape_json(map_file),
        escape_json(default_video),
        escape_json(fps),
        escape_json(speed),
        escape_json(quality),
        escape_json(hwaccel),
        steam_pause_enabled,
        steam_game_running,
        escape_json(service_state),
        monitors_json
    )
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

fn run_cmd_capture(bin: &str, args: &[&str]) -> Result<String, String> {
    let output = Command::new(bin)
        .args(args)
        .output()
        .map_err(|e| format!("failed to execute {bin}: {e}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(format!("{bin} exited with status: {}", output.status))
    }
}

fn detect_monitor_names() -> Result<Vec<String>, String> {
    let json = run_cmd_capture("hyprctl", &["-j", "monitors"])?;
    let mut names = Vec::new();
    let mut rest = json.as_str();
    while let Some(idx) = rest.find("\"name\"") {
        rest = &rest[idx + 6..];
        if let Some(colon) = rest.find(':') {
            rest = &rest[colon + 1..];
            let trimmed = rest.trim_start();
            if let Some(stripped) = trimmed.strip_prefix('"') {
                if let Some(end) = stripped.find('"') {
                    let name = &stripped[..end];
                    if !name.is_empty() {
                        names.push(name.to_string());
                    }
                    rest = &stripped[end + 1..];
                }
            }
        }
    }
    names.sort();
    names.dedup();
    Ok(names)
}

fn parse_csv_list(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn escape_json(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
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
    println!("  kitsune-rendercore status");
    println!(
        "    Show current config, service state, Steam pause state, and monitor->video mapping."
    );
    println!();
    println!(
        "  kitsune-rendercore set-video (--monitor <MONITOR> | --all) --video <VIDEO_PATH> [--except <MON1,MON2>] [--map-file <PATH>]"
    );
    println!(
        "    Update one monitor (or all monitors) mapping for hot-reload without restarting the renderer."
    );
    println!();
    println!(
        "  kitsune-rendercore unset-video (--monitor <MONITOR> | --all) [--except <MON1,MON2>] [--map-file <PATH>]"
    );
    println!("    Remove one mapping, or all mappings with optional exclusions.");
    println!();
    println!("  kitsune-rendercore status [--json] [--pretty|--compact] [--file <PATH>]");
    println!("    Show current runtime/service/monitor mapping in text or JSON.");
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
        "  kitsune-rendercore set-video (--monitor <MONITOR> | --all) --video <VIDEO_PATH> [--except <MON1,MON2>] [--map-file <PATH>]"
    );
    println!();
    println!("Description:");
    println!("  Updates one monitor->video mapping in the map file.");
    println!("  If renderer is running, it reloads the changed mapping automatically.");
    println!();
    println!("Options:");
    println!("  --monitor <MONITOR>   Monitor name (e.g. DP-1, eDP-1, HDMI-A-1).");
    println!("  --all                 Apply same video to all detected monitors.");
    println!("  --except <LIST>       Comma-separated monitor names to skip (only with --all).");
    println!("  --video <VIDEO_PATH>  Absolute path to the video file.");
    println!("  --map-file <PATH>     Custom map file path.");
    println!();
    println!("Example:");
    println!(
        "  kitsune-rendercore set-video --monitor DP-1 --video /home/user/Videos/live/new.mp4"
    );
    println!("  kitsune-rendercore set-video --all --video /home/user/Videos/live/new.mp4");
}

fn print_unset_video_help() {
    println!("kitsune-rendercore unset-video");
    println!("Usage:");
    println!(
        "  kitsune-rendercore unset-video (--monitor <MONITOR> | --all) [--except <MON1,MON2>] [--map-file <PATH>]"
    );
    println!();
    println!("Description:");
    println!("  Removes one monitor mapping, or all mappings with --all.");
    println!();
    println!("Options:");
    println!("  --monitor <MONITOR>   Remove one mapping.");
    println!("  --all                 Remove all mappings.");
    println!("  --except <LIST>       Comma-separated monitor names to keep (only with --all).");
    println!("  --map-file <PATH>     Custom map file path.");
}

fn print_status_help() {
    println!("kitsune-rendercore status");
    println!("Usage:");
    println!("  kitsune-rendercore status [--json] [--pretty|--compact] [--file <PATH>]");
    println!();
    println!("Description:");
    println!("  Shows runtime config, Steam pause state, user service state,");
    println!("  and effective monitor->video mapping.");
    println!();
    println!("Options:");
    println!("  --json       Print status as JSON for automation/CLI integration.");
    println!("  --pretty     Pretty JSON output (default when using --json).");
    println!("  --compact    Compact single-line JSON output.");
    println!("  --file PATH  Write JSON output to file (requires --json).");
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
