#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SERVICE_SRC="$ROOT_DIR/systemd/kitsune-rendercore.service"
ENV_EXAMPLE="$ROOT_DIR/systemd/kitsune-rendercore.env.example"

USER_SYSTEMD_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user"
APP_CONFIG_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/kitsune-rendercore"
SERVICE_DST="$USER_SYSTEMD_DIR/kitsune-rendercore.service"
ENV_DST="$APP_CONFIG_DIR/env"
MAP_DST="$APP_CONFIG_DIR/video-map.conf"

mkdir -p "$USER_SYSTEMD_DIR" "$APP_CONFIG_DIR"
cp -f "$SERVICE_SRC" "$SERVICE_DST"

if [[ ! -f "$ENV_DST" ]]; then
  cp -f "$ENV_EXAMPLE" "$ENV_DST"
  echo "[ok] created $ENV_DST from example"
else
  echo "[ok] keeping existing $ENV_DST"
fi

if [[ ! -f "$MAP_DST" ]]; then
  cat > "$MAP_DST" <<'EOF'
# monitor=/absolute/path/video.mp4
# DP-1=/home/kitotsu/Videos/LiveWallpapers/a.mp4
EOF
  echo "[ok] created $MAP_DST"
else
  echo "[ok] keeping existing $MAP_DST"
fi

systemctl --user daemon-reload
echo "[ok] installed $SERVICE_DST"
echo "[next] edit $ENV_DST, then run:"
echo "  systemctl --user enable --now kitsune-rendercore.service"
