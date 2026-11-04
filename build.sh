#!/usr/bin/env bash
# Builds the WebAssembly module and its JS glue.
#
#   ./build.sh            single-threaded            → web/pkg        works on any page, no special headers
#   ./build.sh simd       single-threaded, +simd128  → web/pkg-simd  only for the comparison in bench.mjs: it measured no faster
#
# Needs the wasm32-unknown-unknown target and wasm-bindgen 0.2.130.
set -euo pipefail
cd "$(dirname "$0")"
mode="${1:-st}"
target=wasm32-unknown-unknown
case "$mode" in
  st)
    cargo build --release --target $target --no-default-features
    out=web/pkg ;;
  simd)
    RUSTFLAGS="-C target-feature=+simd128" cargo build --release --target $target --no-default-features
    out=web/pkg-simd ;;
  *) echo "usage: $0 [st|simd]" >&2; exit 1 ;;
esac
wasm-bindgen --target web --out-dir "$out" --no-typescript target/$target/release/dungeon_lab.wasm
ls -l "$out"/*.wasm | awk -v o="$out" '{print o ": " $5 " bytes"}'
