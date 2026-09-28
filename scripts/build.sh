#!/usr/bin/env bash
# Build the Rust bin of the calling example for wasm32-unknown-emscripten
# and collect emcc's JS + wasm output into ./build.
# usage: bash ../scripts/build.sh <bin-name>
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

BIN="$1"
toolchain_ready && [ -f "$WORK/pumpkin/.patched" ] || bash "$REPO/scripts/setup.sh"
export_toolchain

cargo build --release --bin "$BIN"

OUT="target/wasm32-unknown-emscripten/release"
# emcc names the wasm after the crate (underscored), not the bin.
WASM="$(grep -o '[A-Za-z0-9_-]*\.wasm' "$OUT/$BIN.js" | head -n1)"
mkdir -p build
cp "$OUT/$BIN.js" "$OUT/$WASM" build/
echo "Built build/$BIN.js"
