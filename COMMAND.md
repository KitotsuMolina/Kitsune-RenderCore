# Kitsune-RenderCore Commands

## Build

```bash
cd /home/kitotsu/Programacion/Personal/Wallpaper/Kitsune-RenderCore
cargo build --features wayland-layer
```

## Install command globally

User install (`~/.local/bin`):

```bash
./scripts/install.sh --with-deps
```

System-wide install (`/usr/local/bin`):

```bash
./scripts/install.sh --system
```

## Run (single video for all monitors)

```bash
KRC_VIDEO="/absolute/path/video.mp4" \
KRC_VIDEO_FPS=30 \
KRC_VIDEO_SPEED=1.0 \
KRC_QUALITY=high \
target/debug/kitsune-rendercore
```

## Run (different video per monitor)

```bash
KRC_VIDEO_MAP="DP-1:/path/a.mp4;eDP-1:/path/b.mp4;HDMI-A-1:/path/c.mp4" \
KRC_VIDEO_DEFAULT="/path/fallback.mp4" \
KRC_VIDEO_FPS=30 \
KRC_VIDEO_SPEED=1.0 \
KRC_QUALITY=high \
KRC_PAUSE_ON_STEAM_GAME=true \
KRC_STEAM_POLL_MS=1000 \
target/debug/kitsune-rendercore
```

## Hot change one monitor (no restart)

```bash
target/debug/kitsune-rendercore set-video --monitor DP-1 --video /absolute/path/new-video.mp4
```

Use custom map file:

```bash
target/debug/kitsune-rendercore set-video --monitor DP-1 --video /absolute/path/new-video.mp4 --map-file /home/kitotsu/.config/kitsune-rendercore/video-map.conf
```

Show set-video help:

```bash
target/debug/kitsune-rendercore set-video --help
```

## Debug / test

Run for a fixed number of frames:

```bash
KRC_MAX_FRAMES=300 target/debug/kitsune-rendercore
```

Steam pause debug:

```bash
KRC_STEAM_DEBUG=true target/debug/kitsune-rendercore
```

## Monitor names (for `KRC_VIDEO_MAP`)

```bash
hyprctl -j monitors | jq -r '.[].name'
```

## systemd --user service

Install service files:

```bash
./scripts/install-user-service.sh
```

Enable and start:

```bash
systemctl --user enable --now kitsune-rendercore.service
```

Status:

```bash
systemctl --user status kitsune-rendercore.service
```

Logs:

```bash
journalctl --user -u kitsune-rendercore.service -f
```

Restart:

```bash
systemctl --user restart kitsune-rendercore.service
```

Stop / disable:

```bash
systemctl --user stop kitsune-rendercore.service
systemctl --user disable kitsune-rendercore.service
```

## Release / publish

GitHub release:

```bash
./scripts/release-github.sh
```

AUR publish:

```bash
./scripts/publish-aur.sh
```

## Environment variables

- `KRC_VIDEO`: default video for all monitors.
- `KRC_VIDEO_MAP`: monitor map `MONITOR:/path.mp4;MONITOR:/path.mp4`.
- `KRC_VIDEO_MAP_FILE`: map file path (default: `~/.config/kitsune-rendercore/video-map.conf`).
- `KRC_VIDEO_DEFAULT`: fallback video when monitor is not mapped.
- `KRC_VIDEO_FPS`: decode/render fps target for input stream.
- `KRC_VIDEO_SPEED`: playback speed (`1.0` normal).
- `KRC_QUALITY`: `low|720p`, `medium|1080p`, `high|1440p`, `ultra|4k`.
- `KRC_SOURCE_WIDTH`: force source width (overrides preset).
- `KRC_SOURCE_HEIGHT`: force source height (overrides preset).
- `KRC_PAUSE_ON_STEAM_GAME`: `true|false`.
- `KRC_STEAM_POLL_MS`: Steam process polling interval in ms.
- `KRC_STEAM_DEBUG`: Steam detection debug logs.
- `KRC_MAX_FRAMES`: stop after N frames (debug).
