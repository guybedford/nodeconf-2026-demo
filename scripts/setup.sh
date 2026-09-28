#!/usr/bin/env bash
# Install the pinned Emscripten SDK and Pumpkin sources under .work and apply
# the patches under patches/.
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

mkdir -p "$WORK"

if [ ! -x "$EMSDK/upstream/bin/clang" ]; then
  echo "==> Installing Emscripten SDK $EMSDK_VERSION"
  [ -d "$EMSDK/.git" ] || git clone --depth 1 --branch "$EMSDK_VERSION" https://github.com/emscripten-core/emsdk "$EMSDK"
  "$EMSDK/emsdk" install "$EMSDK_VERSION"
fi

if [ ! -f "$EMSCRIPTEN/.patched" ]; then
  echo "==> Patching Emscripten"
  for p in "$REPO"/patches/emscripten/*.patch; do
    echo "  $(basename "$p")"
    patch -p1 -N -s -d "$EMSCRIPTEN" -i "$p"
  done
  # The release ships a populated sysroot; force emcc to reinstall the
  # headers the patches add.
  rm -f "$EMSCRIPTEN/cache/sysroot_install.stamp"
  touch "$EMSCRIPTEN/.patched"
fi

# Pinned Pumpkin sources for emscripten-minecraft, with the single-threaded
# and memory patches from danlapid/rust-workers-minecraft.
PUMPKIN="$WORK/pumpkin"
if [ ! -d "$PUMPKIN/.git" ]; then
  echo "==> Cloning Pumpkin"
  git clone --filter=blob:none --single-branch https://github.com/Pumpkin-MC/Pumpkin "$PUMPKIN"
  git -C "$PUMPKIN" switch --detach "$PUMPKIN_COMMIT"
fi
if [ ! -f "$PUMPKIN/.patched" ]; then
  echo "==> Patching Pumpkin"
  for p in "$REPO"/patches/pumpkin/*.patch; do
    echo "  $(basename "$p")"
    git -C "$PUMPKIN" apply "$p"
  done
  touch "$PUMPKIN/.patched"
fi

NODE_JS="$(ls -d "$EMSDK"/node/*/bin/node | head -n1)"
cat > "$EM_CONFIG" <<EOF
LLVM_ROOT = '$EMSDK/upstream/bin'
BINARYEN_ROOT = '$EMSDK/upstream'
NODE_JS = '$NODE_JS'
EOF

echo "Setup complete."
