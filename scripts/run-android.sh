#!/bin/bash
# scripts/run-android.sh
#
# Builds and deploys the Siegu Android app on a connected device.
# Steps:
#   1. Build the frontend (yarn build)
#   2. Cross-compile the Rust library for aarch64-linux-android via cargo-ndk
#   3. Copy the .so into jniLibs for the Android Gradle project
#   4. Build a universal debug APK via Gradle
#   5. Install it on the connected device via adb
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

# Clean stale ffmpeg build artifacts so patched build.rs is used
rm -rf target/aarch64-linux-android/debug/build/ffmpeg-sys-next-*
rm -rf target/aarch64-linux-android/release/build/ffmpeg-sys-next-*

echo "=== Building frontend ==="
yarn build

echo "=== Building Rust lib for aarch64-linux-android ==="
cargo ndk -t aarch64-linux-android -P 24 \
  --manifest-path src-tauri/Cargo.toml \
  build --features "tauri/custom-protocol tauri/custom-protocol" --lib --release

echo "=== Copying .so to jniLibs ==="
mkdir -p src-tauri/gen/android/app/src/main/jniLibs/arm64-v8a
cp target/aarch64-linux-android/release/libsiegu_lib.so \
   src-tauri/gen/android/app/src/main/jniLibs/arm64-v8a/

echo "=== Building APK ==="
cd src-tauri/gen/android
ANDROID_HOME="$ANDROID_HOME" ./gradlew assembleUniversalDebug

echo "=== Installing on device ==="
APK="$(find app/build/outputs/apk/universal/debug -name '*.apk' | head -1)"
if [ -n "$APK" ]; then
  adb install -r "$APK"
  echo "Installed $APK on device"
else
  echo "ERROR: No APK found"
  exit 1
fi
