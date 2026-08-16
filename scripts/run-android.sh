#!/bin/bash
# scripts/run-android.sh
#
# Builds and deploys the Siegu Android app on a connected device.
#
# Usage:
#   ./scripts/run-android.sh   # full build (frontend + Rust + APK)
#
# Notes:
#   - The frontend (dist/) is embedded in the Rust .so at compile time,
#     so every build always compiles Rust. Cargo is incremental — only
#     changed crates are rebuilt (~1-2 min for frontend-only changes).
#   - The .so is copied into jniLibs, then Gradle builds the final APK.
#
# Prerequisites:
#   - Android SDK + NDK r27 installed (default: ~/Android/Sdk)
#   - cargo-ndk installed (cargo install cargo-ndk)
#   - Rust targets: rustup target add aarch64-linux-android
#   - yarn deps installed (yarn install)
#   - Connected device or emulator with adb
set -euo pipefail

NDK_VERSION="27.0.12077973"
ANDROID_HOME="${ANDROID_HOME:-$HOME/Android/Sdk}"
NDK="$ANDROID_HOME/ndk/$NDK_VERSION"
TOOLCHAIN="$NDK/toolchains/llvm/prebuilt/linux-x86_64"
PROJECT_DIR="$(cd "$(dirname "$0")/.." && pwd)"

cd "$PROJECT_DIR"

echo "=== Building frontend ==="
yarn build

# ffmpeg-sys-next resolves llvm-nm / llvm-strip via `clang/../llvm-nm`, which
# fails on the clang FILE path. Patch its build.rs (idempotent) so the tools
# resolve next to clang, falling back to PATH. See patch-ffmpeg-android.py.
echo "=== Patching ffmpeg-sys-next build.rs (Android llvm-nm fix) ==="
cargo fetch --manifest-path src-tauri/Cargo.toml
FFMPEG_BUILD_RS="$(find "$HOME/.cargo/registry/src" -path "*/ffmpeg-sys-next-*/build.rs" 2>/dev/null | xargs -r ls -t | head -1)"
if [ -n "$FFMPEG_BUILD_RS" ]; then
  python3 scripts/patch-ffmpeg-android.py "$FFMPEG_BUILD_RS"
else
  echo "WARNING: ffmpeg-sys-next build.rs not found in cargo registry"
fi

echo "=== Building Rust lib for aarch64-linux-android ==="
cargo ndk -t aarch64-linux-android -P 24 \
  --manifest-path src-tauri/Cargo.toml \
  build --features "tauri/custom-protocol" --lib --release

echo "=== Copying .so to jniLibs ==="
mkdir -p src-tauri/gen/android/app/src/main/jniLibs/arm64-v8a
rm -f src-tauri/gen/android/app/src/main/jniLibs/arm64-v8a/libsiegu_lib.so
cp target/aarch64-linux-android/release/libsiegu_lib.so \
   src-tauri/gen/android/app/src/main/jniLibs/arm64-v8a/

echo "=== Building APK ==="
cd src-tauri/gen/android
ANDROID_HOME="$ANDROID_HOME" ./gradlew assembleUniversalRelease -PskipRustBuild=true

echo "=== Installing on device ==="
APK="$(find app/build/outputs/apk/universal/release -name '*.apk' 2>/dev/null | head -1)"
if [ -z "$APK" ]; then
  APK="$(find app/build/outputs/apk/universal/debug -name '*.apk' | head -1)"
fi
if [ -n "$APK" ]; then
  adb install -r "$APK"
  echo "Installed $APK on device"
else
  echo "ERROR: No APK found"
  exit 1
fi
