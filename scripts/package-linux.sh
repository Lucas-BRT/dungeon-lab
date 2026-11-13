#!/usr/bin/env bash
# Packs the release build of the desktop app as the tar.gz that install.sh unpacks:
#   scripts/package-linux.sh [OUT_DIR]     (default: dist)
# Build first, natively: cargo build --release --locked -p tapiti-desktop
set -euo pipefail
cd "$(dirname "$0")/.."

target=x86_64-unknown-linux-gnu
out=${1:-dist}
version=$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -1)
name=tapiti-$target
bin=target/release/tapiti-desktop
[ -x "$bin" ] || { echo "error: $bin is missing; build it first" >&2; exit 1; }

stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT
root=$stage/$name
mkdir -p "$root/bin" "$root/share/tapiti" "$root/share/applications" "$root/share/icons/hicolor/256x256/apps" "$root/share/doc/tapiti"
install -m 755 "$bin" "$root/bin/tapiti"
echo "$version" >"$root/share/tapiti/VERSION"
install -m 644 apps/desktop/packaging/tapiti.desktop "$root/share/applications/tapiti.desktop"
install -m 644 apps/desktop/assets/icon.png "$root/share/icons/hicolor/256x256/apps/tapiti.png"
install -m 644 LICENSE README.md "$root/share/doc/tapiti/"

mkdir -p "$out"
# Sorted names, no owner and a fixed time: the same build gives the same bytes.
tar --sort=name --owner=0 --group=0 --numeric-owner --mtime='2000-01-01 00:00Z' -C "$stage" -cf - "$name" | gzip -9n >"$out/$name.tar.gz"
echo "$out/$name.tar.gz"
