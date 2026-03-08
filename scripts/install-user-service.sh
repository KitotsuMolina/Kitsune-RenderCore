#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

find_asset() {
  local rel="$1"
  local candidates=()

  # source-tree layout
  candidates+=("$SCRIPT_DIR/../systemd/$rel")
  # installed layout: /usr/share/kitsune-rendercore/*
  candidates+=("$SCRIPT_DIR/$rel")
  candidates+=("/usr/share/kitsune-rendercore/$rel")

  if [[ -n "${KRC_SHARE_DIR:-}" ]]; then
    candidates=("${KRC_SHARE_DIR}/$rel" "${candidates[@]}")
  fi

  for c in "${candidates[@]}"; do
    if [[ -f "$c" ]]; then
      printf "%s\n" "$c"
      return 0
    fi
  done
  return 1
}

SERVICE_SRC="$(find_asset "kitsune-rendercore.service" || true)"
ENV_EXAMPLE="$(find_asset "kitsune-rendercore.env.example" || true)"

if [[ -z "$SERVICE_SRC" ]]; then
  echo "[error] could not locate kitsune-rendercore.service" >&2
  exit 1
fi
if [[ -z "$ENV_EXAMPLE" ]]; then
  echo "[error] could not locate kitsune-rendercore.env.example" >&2
  exit 1
fi

USER_SYSTEMD_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user"
APP_CONFIG_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/kitsune-rendercore"
SERVICE_DST="$USER_SYSTEMD_DIR/kitsune-rendercore.service"
ENV_DST="$APP_CONFIG_DIR/env"
MAP_DST="$APP_CONFIG_DIR/video-map.conf"
BIN_PATH="${KRC_BIN_PATH:-$(command -v kitsune-rendercore || true)}"

if [[ -z "$BIN_PATH" ]]; then
  BIN_PATH="/usr/bin/kitsune-rendercore"
fi

mkdir -p "$USER_SYSTEMD_DIR" "$APP_CONFIG_DIR"
cp -f "$SERVICE_SRC" "$SERVICE_DST"
sed -i "s|^ExecStart=.*|ExecStart=${BIN_PATH}|" "$SERVICE_DST"

if [[ ! -f "$ENV_DST" ]]; then
  cp -f "$ENV_EXAMPLE" "$ENV_DST"
  echo "[ok] created $ENV_DST from example"
else
  echo "[ok] keeping existing $ENV_DST"
fi

# Migrate legacy hardcoded /home/<user>/ paths to current logged-in user HOME.
if grep -Eq '^KRC_VIDEO_(DEFAULT|MAP_FILE)=/home/[^/]+' "$ENV_DST" || \
   grep -Eq '^KRC_VIDEO_MAP=.*?/home/[^/]+' "$ENV_DST"; then
  cp -f "$ENV_DST" "${ENV_DST}.bak"
  HOME_ESCAPED="$(printf '%s' "$HOME" | sed 's/[\/&]/\\&/g')"
  sed -i \
    -e "s|^KRC_VIDEO_MAP_FILE=/home/[^/]*/.config/kitsune-rendercore/video-map.conf|KRC_VIDEO_MAP_FILE=${HOME_ESCAPED}/.config/kitsune-rendercore/video-map.conf|" \
    -e "s|^KRC_VIDEO_DEFAULT=/home/[^/]*/|KRC_VIDEO_DEFAULT=${HOME_ESCAPED}/|" \
    -e "/^KRC_VIDEO_MAP=/ s|/home/[^/]*/|${HOME_ESCAPED}/|g" \
    "$ENV_DST"
  echo "[ok] migrated hardcoded home paths in $ENV_DST to $HOME (backup: ${ENV_DST}.bak)"
fi

if [[ ! -f "$MAP_DST" ]]; then
  cat > "$MAP_DST" <<'EOF'
# monitor=/absolute/path/video.mp4
# DP-1=/home/user/Videos/LiveWallpapers/a.mp4
EOF
  echo "[ok] created $MAP_DST"
else
  echo "[ok] keeping existing $MAP_DST"
fi

systemctl --user daemon-reload
echo "[ok] installed $SERVICE_DST"
echo "[ok] using ExecStart=${BIN_PATH}"
echo "[next] edit $ENV_DST, then run:"
echo "  systemctl --user enable --now kitsune-rendercore.service"
