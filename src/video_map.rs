use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub fn default_map_file_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    Path::new(&home)
        .join(".config")
        .join("kitsune-rendercore")
        .join("video-map.conf")
}

pub fn map_file_path_from_env() -> PathBuf {
    std::env::var("KRC_VIDEO_MAP_FILE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| default_map_file_path())
}

#[cfg(feature = "wayland-layer")]
pub fn parse_video_map_env(raw: &str) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for entry in raw.split(';') {
        let trimmed = entry.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Some((monitor, path)) = trimmed.split_once(':') else {
            continue;
        };
        let monitor = monitor.trim();
        let path = path.trim();
        if monitor.is_empty() || path.is_empty() {
            continue;
        }
        map.insert(monitor.to_string(), path.to_string());
    }
    map
}

pub fn parse_video_map_file(path: &Path) -> BTreeMap<String, String> {
    let Ok(contents) = fs::read_to_string(path) else {
        return BTreeMap::new();
    };
    let mut map = BTreeMap::new();
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((monitor, video)) = line.split_once('=') else {
            continue;
        };
        let monitor = monitor.trim();
        let video = video.trim();
        if monitor.is_empty() || video.is_empty() {
            continue;
        }
        map.insert(monitor.to_string(), video.to_string());
    }
    map
}

#[cfg(feature = "wayland-layer")]
pub fn merge_maps(
    env_map: BTreeMap<String, String>,
    file_map: BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    let mut merged = env_map;
    for (k, v) in file_map {
        merged.insert(k, v);
    }
    merged
}

pub fn set_monitor_video(path: &Path, monitor: &str, video: &str) -> Result<(), String> {
    if monitor.trim().is_empty() {
        return Err("monitor is empty".to_string());
    }
    if video.trim().is_empty() {
        return Err("video path is empty".to_string());
    }

    let mut map = parse_video_map_file(path);
    map.insert(monitor.to_string(), video.to_string());

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("failed to create map directory {}: {e}", parent.display()))?;
    }

    let mut out = String::from("# monitor=/absolute/path/video.mp4\n");
    for (k, v) in map {
        out.push_str(&format!("{k}={v}\n"));
    }
    fs::write(path, out).map_err(|e| format!("failed to write {}: {e}", path.display()))
}
