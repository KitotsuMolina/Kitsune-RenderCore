# Kitsune-RenderCore

Base del nuevo renderer propio para live wallpapers en Linux/Wayland.

## Objetivo

Construir un motor layer-shell en un solo proceso para multi-monitor, con contexto GPU compartido y scheduler central.

Stack objetivo:

- `smithay-client-toolkit` + protocolo `wlr-layer-shell`
- `wgpu` (Vulkan como backend principal)
- decode por hardware (`ffmpeg` + VAAPI/NVDEC)

## Estado actual

Proyecto inicializado con esqueleto modular:

- `src/app.rs`
- `src/config.rs`
- `src/runtime.rs`
- `src/backend/*` (abstraccion de backend layer-shell)
- `src/monitor.rs`
- `src/scheduler.rs`
- `src/frame_source.rs` (fuente de frames por `ffmpeg` opcional)

## Roadmap corto

1. Integrar conexión Wayland y descubrimiento de monitores/salidas.
2. Crear una layer-surface por monitor.
3. Inicializar `wgpu` con un `Device/Queue` compartido.
4. Dibujar color/textura de prueba por surface (validar composición de fondo).
5. Integrar decode de video y subida de frames a textura.
6. Scheduler por monitor con políticas de ahorro (fullscreen/maximized).

## Ejecutar

```bash
cargo run
```

## Instalar dependencias del renderer (Wayland + Vulkan)

```bash
kitsune-rendercore install-deps
```

Validar dependencias sin instalar:

```bash
kitsune-rendercore check-deps
```

## Instalar comando global (`kitsune-rendercore`)

Instalación de usuario (`~/.local/bin`):

```bash
./scripts/install.sh --with-deps
```

Instalación de sistema (`/usr/local/bin`):

```bash
./scripts/install.sh --system
```

## Probar backend layer-shell (scaffold inicial)

```bash
cargo run --features wayland-layer
```

Para debug sin loop infinito:

```bash
KRC_MAX_FRAMES=120 cargo run --features wayland-layer
```

Para reproducir un video como fuente de frames del renderer:

```bash
KRC_VIDEO="/ruta/al/video.mp4" \
KRC_VIDEO_FPS=30 \
KRC_VIDEO_SPEED=1.0 \
KRC_QUALITY=high \
KRC_SOURCE_WIDTH=960 \
KRC_SOURCE_HEIGHT=540 \
cargo run --features wayland-layer
```

Para videos distintos por monitor:

```bash
KRC_VIDEO_MAP="DP-1:/home/kitotsu/Videos/LiveWallpapers/a.mp4;eDP-1:/home/kitotsu/Videos/LiveWallpapers/b.mp4;HDMI-A-1:/home/kitotsu/Videos/LiveWallpapers/c.mp4" \
KRC_VIDEO_DEFAULT="/home/kitotsu/Videos/LiveWallpapers/fallback.mp4" \
KRC_VIDEO_FPS=30 \
KRC_VIDEO_SPEED=1.0 \
KRC_QUALITY=high \
cargo run --features wayland-layer
```

Cambio en caliente de un solo monitor (sin reiniciar renderer):

```bash
target/debug/kitsune-rendercore set-video --monitor DP-1 --video /home/kitotsu/Videos/LiveWallpapers/new.mp4
```

Opcionalmente puedes elegir archivo de mapa:

```bash
target/debug/kitsune-rendercore set-video --monitor DP-1 --video /home/kitotsu/Videos/LiveWallpapers/new.mp4 --map-file /home/kitotsu/.config/kitsune-rendercore/video-map.conf
```

Aplicar el mismo video a todos los monitores:

```bash
target/debug/kitsune-rendercore set-video --all --video /home/kitotsu/Videos/LiveWallpapers/new.mp4
```

Aplicar el mismo video a todos excepto algunos monitores:

```bash
target/debug/kitsune-rendercore set-video --all --video /home/kitotsu/Videos/LiveWallpapers/new.mp4 --except eDP-1,HDMI-A-1
```

Eliminar mapeo de un monitor:

```bash
target/debug/kitsune-rendercore unset-video --monitor DP-1
```

Estado actual (config + servicio + mapeo por monitor):

```bash
target/debug/kitsune-rendercore status
```

Estado en JSON (para integración con otros CLI):

```bash
target/debug/kitsune-rendercore status --json
```

Estado en JSON compacto:

```bash
target/debug/kitsune-rendercore status --json --compact
```

Guardar estado JSON en archivo:

```bash
target/debug/kitsune-rendercore status --json --file /tmp/krc-status.json
```

Eliminar todos los mapeos:

```bash
target/debug/kitsune-rendercore unset-video --all
```

Eliminar todos los mapeos excepto algunos:

```bash
target/debug/kitsune-rendercore unset-video --all --except eDP-1,HDMI-A-1
```

Si no ves el render, detén wallpapers previos:

```bash
pkill -f mpvpaper || true
pkill -f swww-daemon || true
KRC_MAX_FRAMES=300 cargo run --features wayland-layer
```

Notas:
- Sin el feature `wayland-layer`, el runtime usa backend stub local.
- Con el feature `wayland-layer`, se activa backend nativo `wl_output + wlr-layer-shell + wgpu`.
- `KRC_VIDEO` usa `ffmpeg` por `stdout` raw RGBA y hace loop infinito (`-stream_loop -1`).
- `KRC_VIDEO_MAP` permite un video por monitor: `MONITOR:/ruta/video.mp4;MONITOR:/ruta/video.mp4`.
- `KRC_VIDEO_MAP_FILE` ruta a archivo de mapeo por monitor (default: `~/.config/kitsune-rendercore/video-map.conf`).
- `KRC_VIDEO_DEFAULT` actúa como fallback cuando un monitor no está en `KRC_VIDEO_MAP`.
- `KRC_VIDEO_SPEED` controla la velocidad (`1.0` normal, `0.5` lenta, `1.25` rápida).
- `KRC_HWACCEL` controla decode por hardware: `auto` (default), `nvdec`, `vaapi`, `none`.
- `KRC_WAVE_EFFECT=true|false` activa/desactiva efecto de ondas en shader (default: `false`).
- `KRC_QUALITY` presets: `low/720p`, `medium/1080p`, `high/1440p`, `ultra/4k`.
- `KRC_SOURCE_WIDTH/HEIGHT` tienen prioridad sobre `KRC_QUALITY`.
- Si la resolución pedida supera el límite de la GPU, se aplica fallback automático (clamp) sin panic.
- `KRC_PAUSE_ON_STEAM_GAME=true|false` pausa el render cuando detecta un juego de Steam (default: `true`).
- `KRC_STEAM_POLL_MS` controla cada cuánto escanea procesos Steam (default: `1500` ms).
- `KRC_STEAM_DEBUG=true` imprime qué PID/razón mantiene el modo pausa.
- Si `KRC_VIDEO` no está definido, renderiza textura procedural animada.

## Servicio systemd --user (optimizado)

Instalar archivos de servicio:

```bash
kitsune-rendercore install-service
```

Editar variables:

```bash
$EDITOR ~/.config/kitsune-rendercore/env
```

El archivo de mapeo en caliente por monitor es:

```bash
$EDITOR ~/.config/kitsune-rendercore/video-map.conf
```

Activar y arrancar:

```bash
kitsune-rendercore service enable
```

Ver estado/log:

```bash
kitsune-rendercore service status
kitsune-rendercore service logs
```

## Publicar release y AUR

Crear release en GitHub (tag + binary asset):

```bash
./scripts/release-github.sh
```

Con bump semántico:

```bash
./scripts/release-github.sh --patch
./scripts/release-github.sh --minor
./scripts/release-github.sh --major
```

O versión explícita:

```bash
./scripts/release-github.sh --set 1.2.3
```

Publicar paquete AUR:

```bash
./scripts/publish-aur.sh
```
