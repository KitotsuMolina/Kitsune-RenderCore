#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

if ! command -v gh >/dev/null 2>&1; then
  echo "gh CLI is required. Install github-cli package." >&2
  exit 1
fi

usage() {
  cat <<'EOF'
Usage: ./scripts/release-github.sh [OPTIONS]

Version selection:
  --patch             Bump patch version (x.y.Z -> x.y.Z+1)
  --minor             Bump minor version (x.Y.z -> x.Y+1.0)
  --major             Bump major version (X.y.z -> X+1.0.0)
  --set <VERSION>     Set explicit version (e.g. 1.2.3)

Behavior:
  --no-commit         Do not create/push version bump commit
  -h, --help          Show this help
EOF
}

current_version() {
  sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n1
}

is_semver() {
  [[ "$1" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]
}

bump_semver() {
  local v="$1" mode="$2"
  IFS='.' read -r major minor patch <<<"$v"
  case "$mode" in
    patch) patch=$((patch + 1)) ;;
    minor) minor=$((minor + 1)); patch=0 ;;
    major) major=$((major + 1)); minor=0; patch=0 ;;
    *) echo "invalid bump mode: $mode" >&2; exit 1 ;;
  esac
  printf "%d.%d.%d\n" "$major" "$minor" "$patch"
}

set_version_files() {
  local version="$1"
  sed -i "0,/^version = \".*\"/s//version = \"${version}\"/" Cargo.toml
  sed -i "s/^pkgver=.*/pkgver=${version}/" aur/PKGBUILD
  sed -i "s/^pkgver = .*/pkgver = ${version}/" aur/.SRCINFO
  sed -i "s|refs/tags/v[0-9][^ ]*|refs/tags/v${version}.tar.gz|g" aur/.SRCINFO
  sed -i "s|kitsune-rendercore-[0-9][^:]*.tar.gz::|kitsune-rendercore-${version}.tar.gz::|g" aur/.SRCINFO
}

bump_mode=""
set_version=""
do_commit=true
while (($#)); do
  case "$1" in
    --patch) bump_mode="patch" ;;
    --minor) bump_mode="minor" ;;
    --major) bump_mode="major" ;;
    --set)
      shift
      set_version="${1:-}"
      ;;
    --no-commit) do_commit=false ;;
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

if [[ -n "$bump_mode" && -n "$set_version" ]]; then
  echo "Use either --set or --patch/--minor/--major, not both." >&2
  exit 1
fi

CURRENT_VERSION="$(current_version)"
if [[ -z "$CURRENT_VERSION" ]]; then
  echo "Could not determine version from Cargo.toml" >&2
  exit 2
fi

if ! is_semver "$CURRENT_VERSION"; then
  echo "Current Cargo.toml version is not simple semver (x.y.z): $CURRENT_VERSION" >&2
  exit 2
fi

VERSION="$CURRENT_VERSION"
if [[ -n "$set_version" ]]; then
  if ! is_semver "$set_version"; then
    echo "Invalid --set version (expected x.y.z): $set_version" >&2
    exit 1
  fi
  VERSION="$set_version"
elif [[ -n "$bump_mode" ]]; then
  VERSION="$(bump_semver "$CURRENT_VERSION" "$bump_mode")"
fi

if [[ "$VERSION" != "$CURRENT_VERSION" ]]; then
  echo "[release] version bump: $CURRENT_VERSION -> $VERSION"
  set_version_files "$VERSION"
  cargo generate-lockfile
  if [[ "$do_commit" == true ]]; then
    git add Cargo.toml Cargo.lock aur/PKGBUILD aur/.SRCINFO
    git commit -m "chore(release): v${VERSION}" || true
    git push origin main
  fi
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
