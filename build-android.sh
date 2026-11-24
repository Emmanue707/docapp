#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")" && pwd)"
cd "$ROOT"

API=26
ABIS=(aarch64-linux-android x86_64-linux-android)

need_targets=()
for abi in "${ABIS[@]}"; do
  if ! rustup target list --installed | grep -qx "$abi"; then
    need_targets+=("$abi")
  fi
done
if ((${#need_targets[@]} > 0)); then
  echo "Missing Rust targets. Install with:"
  echo "rustup target add ${need_targets[*]}"
  exit 1
fi

if [[ -n "${ANDROID_NDK_HOME:-}" ]]; then
  NDK_HOME="$ANDROID_NDK_HOME"
elif [[ -d "$HOME/Android/Sdk/ndk" ]]; then
  NDK_HOME="$(ls -1d "$HOME/Android/Sdk/ndk"/* 2>/dev/null | sort -V | tail -1)"
else
  echo "Set ANDROID_NDK_HOME or install the Android NDK under ~/Android/Sdk/ndk"
  exit 1
fi

JNI_LIBS="$ROOT/apps/android/app/src/main/jniLibs"
JAVA_OUT="$ROOT/apps/android/app/src/main/java"
mkdir -p "$JNI_LIBS" "$JAVA_OUT"

build_abi() {
  local abi="$1"
  local jni_dir
  case "$abi" in
    aarch64-linux-android) jni_dir="$JNI_LIBS/arm64-v8a" ;;
    x86_64-linux-android) jni_dir="$JNI_LIBS/x86_64" ;;
    *) echo "unknown abi $abi"; exit 1 ;;
  esac
  mkdir -p "$jni_dir"

  if command -v cargo-ndk >/dev/null 2>&1; then
    cargo ndk --target "$abi" -p docsuite-ffi --platform "$API" build --release --features ocrs
  else
    local prebuilt="$NDK_HOME/toolchains/llvm/prebuilt"
    local host
    host="$(ls -1d "$prebuilt"/* | head -1)"
    local triple="$abi"
    local linker="$host/bin/${triple}${API}-clang"
    local cc_key="${triple//-/_}"
    local linker_key="$(echo "$abi" | tr '[:lower:]' '[:upper:]' | tr '-' '_')"
    export "CARGO_TARGET_${linker_key}_LINKER=$linker"
    export "CC_${cc_key}=$linker"
    export "CXX_${cc_key}=$linker"
    export "AR_${cc_key}=$host/bin/llvm-ar"
    export "RANLIB_${cc_key}=$host/bin/llvm-ranlib"
    flock /tmp/docapp-build.lock nice -n 10 cargo build -p docsuite-ffi --release --target "$abi" --features ocrs -j2
  fi

  local lib="$ROOT/target/$abi/release/libdocsuite_ffi.so"
  cp "$lib" "$jni_dir/libdocsuite_ffi.so"
}

for abi in "${ABIS[@]}"; do
  build_abi "$abi"
done

flock /tmp/docapp-build.lock nice -n 10 cargo build -p docsuite-ffi --features ocrs -j2
LIB_SRC="$ROOT/target/debug/libdocsuite_ffi.so"
flock /tmp/docapp-build.lock nice -n 10 cargo run -p docsuite-ffi --features bindgen,ocrs --bin uniffi-bindgen -- \
  generate --library --language kotlin --crate docsuite_ffi --out-dir "$JAVA_OUT" "$LIB_SRC"

echo "Android native libs and Kotlin bindings updated."