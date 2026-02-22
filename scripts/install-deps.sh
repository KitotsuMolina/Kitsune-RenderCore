#!/usr/bin/env bash
set -euo pipefail

log() {
  printf "[rendercore-deps] %s\n" "$*"
}

err() {
  printf "[rendercore-deps][error] %s\n" "$*" >&2
}

need_cmd() {
  command -v "$1" >/dev/null 2>&1
}

if [[ "${EUID:-$(id -u)}" -eq 0 ]]; then
  err "No ejecutes este script como root. Ejecutalo como usuario normal."
  exit 1
fi

if ! need_cmd sudo; then
  err "sudo es requerido."
  exit 1
fi

if [[ -f /etc/os-release ]]; then
  # shellcheck disable=SC1091
  source /etc/os-release
else
  err "No se pudo detectar distro (/etc/os-release)."
  exit 1
fi

install_arch() {
  local pkgs=(
    rustup
    vulkan-icd-loader
    vulkan-tools
    wayland
    wayland-protocols
    libxkbcommon
    mesa
    libdrm
    pkgconf
    clang
    cmake
    jq
  )
  sudo pacman -Syu --needed "${pkgs[@]}"
}

install_debian() {
  local pkgs=(
    curl
    pkg-config
    clang
    cmake
    libwayland-dev
    wayland-protocols
    libxkbcommon-dev
    libdrm-dev
    libvulkan-dev
    vulkan-tools
    jq
  )
  sudo apt-get update
  sudo apt-get install -y "${pkgs[@]}"
  if ! need_cmd rustup; then
    curl https://sh.rustup.rs -sSf | sh -s -- -y
    # shellcheck disable=SC1090
    source "${HOME}/.cargo/env"
  fi
}

install_fedora() {
  local pkgs=(
    rustup
    pkgconf-pkg-config
    clang
    cmake
    wayland-devel
    wayland-protocols-devel
    libxkbcommon-devel
    libdrm-devel
    vulkan-loader-devel
    vulkan-tools
    jq
  )
  sudo dnf install -y "${pkgs[@]}"
}

case "${ID:-}" in
  arch)
    log "Detectado Arch Linux"
    install_arch
    ;;
  debian|ubuntu|linuxmint|pop)
    log "Detectada familia Debian/Ubuntu (${ID})"
    install_debian
    ;;
  fedora)
    log "Detectado Fedora"
    install_fedora
    ;;
  *)
    err "Distro no soportada ID='${ID:-unknown}'. Instala manualmente: rustup, toolchain C (clang/cmake/pkg-config), wayland(-dev), wayland-protocols, libxkbcommon, libdrm, vulkan-loader, vulkan-tools."
    exit 2
    ;;
esac

if need_cmd rustup; then
  rustup default stable
fi

missing=()
for bin in cargo rustc pkg-config clang cmake vulkaninfo jq; do
  if ! need_cmd "$bin"; then
    missing+=("$bin")
  fi
done

if ((${#missing[@]} > 0)); then
  err "Faltan binarios tras instalación: ${missing[*]}"
  exit 3
fi

log "Dependencias instaladas y verificadas."
log "Siguiente paso: cargo run --features wayland-layer"
