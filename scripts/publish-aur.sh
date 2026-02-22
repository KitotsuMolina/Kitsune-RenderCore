#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

PKGNAME="kitsune-rendercore"
AUR_REPO="ssh://aur@aur.archlinux.org/${PKGNAME}.git"
TMP_DIR="/tmp/${PKGNAME}-aur"

VERSION="${1:-}"
if [[ -z "$VERSION" ]]; then
  VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n1)"
fi
if [[ -z "$VERSION" ]]; then
  echo "Could not determine version from Cargo.toml" >&2
  exit 1
fi

if ! command -v git >/dev/null 2>&1; then
  echo "git is required" >&2
  exit 1
fi

if [[ -d "$TMP_DIR/.git" ]]; then
  git -C "$TMP_DIR" fetch origin
  git -C "$TMP_DIR" checkout master || git -C "$TMP_DIR" checkout main
  git -C "$TMP_DIR" pull --rebase
else
  rm -rf "$TMP_DIR"
  git clone "$AUR_REPO" "$TMP_DIR"
fi

cp -f aur/PKGBUILD "$TMP_DIR/PKGBUILD"
cp -f aur/.SRCINFO "$TMP_DIR/.SRCINFO"

sed -i "s/^pkgver=.*/pkgver=${VERSION}/" "$TMP_DIR/PKGBUILD"
sed -i "s|refs/tags/v[0-9][^\"']*|refs/tags/v${VERSION}|g" "$TMP_DIR/PKGBUILD"
sed -i "s/^pkgver = .*/pkgver = ${VERSION}/" "$TMP_DIR/.SRCINFO"
sed -i "s|refs/tags/v[0-9][^ ]*|refs/tags/v${VERSION}.tar.gz|g" "$TMP_DIR/.SRCINFO"
sed -i "s|${PKGNAME}-[0-9][^:]*.tar.gz::|${PKGNAME}-${VERSION}.tar.gz::|g" "$TMP_DIR/.SRCINFO"

git -C "$TMP_DIR" add PKGBUILD .SRCINFO
if git -C "$TMP_DIR" diff --cached --quiet; then
  echo "[aur] no changes to publish"
  exit 0
fi

git -C "$TMP_DIR" commit -m "Update to ${VERSION}"
git -C "$TMP_DIR" push origin HEAD

echo "[ok] AUR package published: ${PKGNAME} ${VERSION}"
