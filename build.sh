#!/usr/bin/env bash
# Builds the WebAssembly module and its JS glue into web/pkg (needs the wasm32-unknown-unknown target and wasm-bindgen 0.2.130).
set -euo pipefail
cd "$(dirname "$0")"
cargo build --release --target wasm32-unknown-unknown
wasm-bindgen --target web --out-dir web/pkg --no-typescript target/wasm32-unknown-unknown/release/dungeon_lab.wasm
ls -la web/pkg/*.wasm | awk '{print "web/pkg/dungeon_lab_bg.wasm: " $5 " bytes"}'
