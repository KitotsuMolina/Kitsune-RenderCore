# Kitsune-RenderCore Command Reference

## Main command

`kitsune-rendercore`  
Starts the wallpaper renderer with current environment variables/config files.

Example:

```bash
KRC_VIDEO="/absolute/path/video.mp4" KRC_QUALITY=high kitsune-rendercore
```

## `--help`

`kitsune-rendercore --help`  
Shows all available commands with short descriptions.

## Set one monitor video (hot reload)

`kitsune-rendercore set-video --monitor <MONITOR> --video <VIDEO_PATH> [--map-file <PATH>]`  
Updates only one monitor mapping. If the renderer is running, it reloads automatically (no full restart).

Examples:

```bash
kitsune-rendercore set-video --monitor DP-1 --video /home/user/Videos/live/a.mp4
```

```bash
kitsune-rendercore set-video --monitor HDMI-A-1 --video /home/user/Videos/live/c.mp4 --map-file /home/user/.config/kitsune-rendercore/video-map.conf
```

## Check dependencies (no install)

`kitsune-rendercore check-deps`  
Checks required binaries and core runtime dependencies. Does not install anything.

## Install dependencies

`kitsune-rendercore install-deps`  
Installs dependencies for the detected distro (wrapper over `install-deps.sh`).

## Install service files

`kitsune-rendercore install-service`  
Installs user service assets:
- `~/.config/systemd/user/kitsune-rendercore.service`
- `~/.config/kitsune-rendercore/env`
- `~/.config/kitsune-rendercore/video-map.conf`

## Service management

`kitsune-rendercore service <ACTION>`  
Manages `systemd --user` service.

Available actions:
- `install`: same as `install-service`
- `enable`: enable + start
- `disable`: disable + stop
- `start`: start service
- `stop`: stop service
- `restart`: restart service
- `status`: show status
- `logs`: follow logs

Examples:

```bash
kitsune-rendercore service enable
```

```bash
kitsune-rendercore service status
```

```bash
kitsune-rendercore service logs
```

## Build/install project command

Build local binary:

```bash
cargo build --features wayland-layer
```

Install command to user PATH (`~/.local/bin`):

```bash
./scripts/install.sh --with-deps
```

Install command system-wide (`/usr/local/bin`):

```bash
./scripts/install.sh --system
```

## Release and AUR publish helpers

Create GitHub release (tag + binary asset):

```bash
./scripts/release-github.sh
```

Semantic bump + release:

```bash
./scripts/release-github.sh --patch
./scripts/release-github.sh --minor
./scripts/release-github.sh --major
```

Set explicit version + release:

```bash
./scripts/release-github.sh --set 1.2.3
```

Publish/update AUR package:

```bash
./scripts/publish-aur.sh
```

## Important environment variables

- `KRC_VIDEO`: single default video for all monitors.
- `KRC_VIDEO_MAP`: per-monitor map `MONITOR:/path.mp4;MONITOR:/path.mp4`.
- `KRC_VIDEO_MAP_FILE`: map file path (default `~/.config/kitsune-rendercore/video-map.conf`).
- `KRC_VIDEO_DEFAULT`: fallback video if monitor not mapped.
- `KRC_VIDEO_FPS`: input decode FPS.
- `KRC_VIDEO_SPEED`: playback speed (`1.0` normal).
- `KRC_QUALITY`: `low|720p`, `medium|1080p`, `high|1440p`, `ultra|4k`.
- `KRC_SOURCE_WIDTH`: force source width.
- `KRC_SOURCE_HEIGHT`: force source height.
- `KRC_PAUSE_ON_STEAM_GAME`: pause renderer while Steam game is active (`true|false`).
- `KRC_STEAM_POLL_MS`: Steam process poll interval.
- `KRC_STEAM_DEBUG`: print Steam detection reasons.
- `KRC_MAX_FRAMES`: stop after N frames (debug/testing).
