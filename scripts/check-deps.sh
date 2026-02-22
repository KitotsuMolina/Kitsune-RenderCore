#!/usr/bin/env bash
set -euo pipefail

ok() { printf "[check-deps][ok] %s\n" "$*"; }
warn() { printf "[check-deps][warn] %s\n" "$*"; }
err() { printf "[check-deps][error] %s\n" "$*" >&2; }

need_cmd() {
  command -v "$1" >/dev/null 2>&1
}

missing=()

check_bin() {
  local b="$1"
  if need_cmd "$b"; then
    ok "binary found: $b"
  else
    missing+=("$b")
    err "binary missing: $b"
  fi
}

check_bin cargo
check_bin rustc
check_bin pkg-config
check_bin clang
check_bin cmake
check_bin ffmpeg
check_bin vulkaninfo
check_bin jq

if need_cmd pkg-config; then
  for pc in wayland-client wayland-protocols xkbcommon libdrm; do
    if pkg-config --exists "$pc"; then
      ok "pkg-config module found: $pc"
    else
      warn "pkg-config module missing: $pc"
    fi
  done
fi

if need_cmd vulkaninfo; then
  if vulkaninfo --summary >/dev/null 2>&1; then
    ok "vulkan runtime appears healthy"
  else
    warn "vulkaninfo exists but failed to query Vulkan runtime/driver"
  fi
fi

if ((${#missing[@]} > 0)); then
  err "missing required binaries: ${missing[*]}"
  err "run ./scripts/install-deps.sh to install dependencies"
  exit 1
fi

ok "all required binaries are present"
