#!/usr/bin/env bash
set -euo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORK="$REPO/.work"
EMSDK_VERSION=6.0.10
EMSDK="$WORK/emsdk"
EMSCRIPTEN="$EMSDK/upstream/emscripten"
EM_CONFIG="$WORK/emscripten.config"
PUMPKIN_COMMIT=b5b9b9d7010e793806a83c495af223c67e1d35ee

toolchain_ready() {
  [ -x "$EMSDK/upstream/bin/clang" ] &&
    [ -x "$EMSCRIPTEN/emcc" ] &&
    [ -f "$EMSCRIPTEN/.patched" ] &&
    [ -f "$EM_CONFIG" ]
}

export_toolchain() {
  export EM_CONFIG
  export CARGO_TARGET_WASM32_UNKNOWN_EMSCRIPTEN_LINKER="$EMSCRIPTEN/emcc"
  export EMCC_CFLAGS="${EMCC_CFLAGS:+$EMCC_CFLAGS }-fwasm-exceptions -sWASM_LEGACY_EXCEPTIONS=0"
  export PATH="$EMSCRIPTEN:$PATH"
}
