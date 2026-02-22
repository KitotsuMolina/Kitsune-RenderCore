#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

if ! command -v gh >/dev/null 2>&1; then
  echo "gh CLI is required. Install github-cli package." >&2
  exit 1
fi

VERSION="${1:-}"
if [[ -z "$VERSION" ]]; then
  VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n1)"
fi
if [[ -z "$VERSION" ]]; then
  echo "Could not determine version from Cargo.toml" >&2
  exit 2
fi

TAG="v${VERSION}"

echo "[release] building release binary"
cargo build --release --locked --features wayland-layer

ASSET_DIR="$ROOT_DIR/dist"
mkdir -p "$ASSET_DIR"
cp -f "target/release/kitsune-rendercore" "$ASSET_DIR/kitsune-rendercore-linux-x86_64"

if ! git rev-parse "$TAG" >/dev/null 2>&1; then
  git tag "$TAG"
fi
git push origin "$TAG"

echo "[release] creating/updating GitHub release $TAG"
gh release create "$TAG" \
  "$ASSET_DIR/kitsune-rendercore-linux-x86_64#kitsune-rendercore-linux-x86_64" \
  --generate-notes \
  --latest \
  || gh release upload "$TAG" "$ASSET_DIR/kitsune-rendercore-linux-x86_64#kitsune-rendercore-linux-x86_64" --clobber

echo "[ok] release published: $TAG"
