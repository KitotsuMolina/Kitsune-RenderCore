#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN_NAME="kitsune-rendercore"

log() {
  printf "[rendercore-install] %s\n" "$*"
}

usage() {
  cat <<'EOF'
Usage: ./scripts/install.sh [OPTIONS]

Options:
  --with-deps           Run ./scripts/install-deps.sh first
  --system              Install to /usr/local/bin (requires sudo)
  --root <PATH>         Custom cargo install root (default: ~/.local)
  --no-force            Do not pass --force to cargo install
  -h, --help            Show this help
EOF
}

with_deps=false
system_install=false
cargo_root="${HOME}/.local"
force_flag="--force"

while (($#)); do
  case "$1" in
    --with-deps)
      with_deps=true
      ;;
    --system)
      system_install=true
      ;;
    --root)
      shift
      cargo_root="${1:-}"
      if [[ -z "${cargo_root}" ]]; then
        echo "Missing value for --root" >&2
        exit 1
      fi
      ;;
    --no-force)
      force_flag=""
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      usage
      exit 1
      ;;
  esac
  shift
done

cd "$ROOT_DIR"

if [[ "$with_deps" == true ]]; then
  ./scripts/install-deps.sh
fi

log "Installing ${BIN_NAME} with cargo (feature: wayland-layer)"
cargo install --path . --locked --features wayland-layer ${force_flag} --root "$cargo_root"

installed_bin="${cargo_root}/bin/${BIN_NAME}"
if [[ ! -x "$installed_bin" ]]; then
  echo "Install failed: binary not found at $installed_bin" >&2
  exit 2
fi

if [[ "$system_install" == true ]]; then
  if ! command -v sudo >/dev/null 2>&1; then
    echo "sudo is required for --system" >&2
    exit 3
  fi
  sudo install -Dm755 "$installed_bin" "/usr/local/bin/${BIN_NAME}"
  log "Installed system-wide: /usr/local/bin/${BIN_NAME}"
  log "Run: ${BIN_NAME} --help"
  exit 0
fi

if [[ ":${PATH}:" != *":${cargo_root}/bin:"* ]]; then
  log "Add this to your shell rc if command is not found:"
  log "  export PATH=\"${cargo_root}/bin:\$PATH\""
fi

log "Installed: ${installed_bin}"
log "Run: ${BIN_NAME} --help"
