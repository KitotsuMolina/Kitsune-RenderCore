use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

pub struct SteamGameDetector {
    enabled: bool,
    poll_interval: Duration,
    last_probe_at: Instant,
    last_result: bool,
}

impl SteamGameDetector {
    pub fn from_env() -> Self {
        let enabled = std::env::var("KRC_PAUSE_ON_STEAM_GAME")
            .ok()
            .map(|v| matches!(v.to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
            .unwrap_or(true);
        let poll_ms = std::env::var("KRC_STEAM_POLL_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .filter(|v| *v >= 100)
            .unwrap_or(1500);

        Self {
            enabled,
            poll_interval: Duration::from_millis(poll_ms),
            last_probe_at: Instant::now() - Duration::from_millis(poll_ms),
            last_result: false,
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn steam_game_running(&mut self) -> bool {
        if !self.enabled {
            return false;
        }
        if self.last_probe_at.elapsed() < self.poll_interval {
            return self.last_result;
        }
        self.last_probe_at = Instant::now();
        self.last_result = detect_steam_game_process();
        self.last_result
    }
}

fn detect_steam_game_process() -> bool {
    let proc_dir = Path::new("/proc");
    let Ok(entries) = fs::read_dir(proc_dir) else {
        return false;
    };
    let debug = std::env::var("KRC_STEAM_DEBUG")
        .ok()
        .map(|v| matches!(v.to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
        .unwrap_or(false);

    for entry in entries.flatten() {
        let name = entry.file_name();
        let pid = name.to_string_lossy();
        if !pid.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }

        let p = entry.path();
        if is_zombie_process(&p) {
            continue;
        }
        if let Some(reason) = steam_game_reason(&p) {
            if debug {
                eprintln!(
                    "[rendercore] steam-game-match pid={} reason={}",
                    pid, reason
                );
            }
            return true;
        }
    }
    false
}

fn steam_game_reason(proc_path: &Path) -> Option<String> {
    let cmdline = fs::read(proc_path.join("cmdline")).ok();
    let cmd = cmdline
        .as_ref()
        .map(|raw| nul_join(raw))
        .unwrap_or_default();
    let cmd_l = cmd.to_ascii_lowercase();
    if cmd_l.contains("steamwebhelper")
        || cmd_l.ends_with("/steam")
        || cmd_l.contains("/steam.sh")
        || cmd_l.contains("steam-runtime")
    {
        return None;
    }

    if cmd.contains("steamapps/common/") {
        return Some("cmdline:steamapps/common".to_string());
    }

    // Proton/Steam game processes usually export one of these env vars.
    let environ = fs::read(proc_path.join("environ")).ok();
    if let Some(raw) = environ.as_ref() {
        let env_blob = nul_join(raw);
        for key in ["SteamAppId", "SteamGameId", "STEAM_COMPAT_APP_ID"] {
            if let Some(v) = env_var_value(&env_blob, key) {
                if is_real_game_app_id(v) {
                    return Some(format!("environ:{key}={v}"));
                }
            }
        }
    }

    None
}

fn env_var_value<'a>(env_blob: &'a str, key: &str) -> Option<&'a str> {
    let prefix = format!("{key}=");
    env_blob
        .split_whitespace()
        .find_map(|entry| entry.strip_prefix(&prefix))
}

fn is_real_game_app_id(v: &str) -> bool {
    let Ok(id) = v.parse::<u32>() else {
        return false;
    };
    if id == 0 {
        return false;
    }
    // Steam client and non-game utility ids that cause false positives.
    !matches!(id, 7 | 228980 | 229000 | 480 | 769)
}

fn is_zombie_process(proc_path: &Path) -> bool {
    let Ok(stat) = fs::read_to_string(proc_path.join("stat")) else {
        return false;
    };
    let Some(end_comm) = stat.rfind(')') else {
        return false;
    };
    let tail = stat[end_comm + 1..].trim_start();
    let mut parts = tail.split_whitespace();
    matches!(parts.next(), Some("Z"))
}

fn nul_join(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).replace('\0', " ")
}
