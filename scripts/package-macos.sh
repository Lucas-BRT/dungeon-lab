#!/usr/bin/env bash
# Packs the release build of the desktop app as Tapiti.app inside a tar.gz:
#   scripts/package-macos.sh [OUT_DIR]     (default: dist)
# Build first, on a Mac: cargo build --release --locked -p tapiti-desktop --target aarch64-apple-darwin
set -euo pipefail
cd "$(dirname "$0")/.."

target=aarch64-apple-darwin
out=${1:-dist}
version=$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -1)
name=tapiti-$target
bin=target/$target/release/tapiti-desktop
[ -x "$bin" ] || { echo "error: $bin is missing; build it first" >&2; exit 1; }

stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT
app=$stage/$name/Tapiti.app
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
install -m 755 "$bin" "$app/Contents/MacOS/tapiti"
sed "s/@VERSION@/$version/g" apps/desktop/packaging/Info.plist >"$app/Contents/Info.plist"
install -m 644 apps/desktop/assets/icon.icns "$app/Contents/Resources/tapiti.icns"
cp LICENSE README.md "$stage/$name/"
# Without a developer certificate the bundle is signed ad hoc, so macOS runs it after the "unidentified developer" prompt.
if command -v codesign >/dev/null; then
    codesign --force --deep --sign - "$app"
fi

mkdir -p "$out"
tar -C "$stage" -czf "$out/$name.tar.gz" "$name"
echo "$out/$name.tar.gz"
