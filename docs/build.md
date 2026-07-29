# Building from Source

## Desktop (Linux/macOS/Windows)

### Development mode

```bash
npm run tauri dev
```

### Production build

```bash
npm run tauri build
```

This produces:
- **Linux**: `.AppImage` + `.deb`
- **macOS**: `.dmg`
- **Windows**: `.msi` installer

### Build flags

| Flag | Effect |
|------|--------|
| `--bundles appimage` | Linux AppImage only |
| `--bundles deb` | Debian package only |
| `--no-bundle` | Build binary without packaging |

---

## Android

### Prerequisites

- Android SDK + NDK r27 (`~/Android/Sdk`)
- Rust target: `rustup target add aarch64-linux-android`
- `cargo-ndk`: `cargo install cargo-ndk`
- Connected device or emulator with ADB

### Build & Deploy (automated)

```bash
bash scripts/run-android.sh
```

The script:
1. Builds the frontend (`yarn build`)
2. Cross-compiles Rust for `aarch64-linux-android` via `cargo-ndk`
3. Copies `libsiegu_lib.so` into `jniLibs/arm64-v8a`
4. Builds a universal debug APK via Gradle
5. Installs on the connected device via `adb install -r`

### Manual steps

```bash
# 1. Build frontend
yarn build

# 2. Cross-compile Rust
cargo ndk -t aarch64-linux-android -P 24 \
  --manifest-path src-tauri/Cargo.toml \
  build --features "tauri/custom-protocol" --lib --release

# 3. Copy .so
mkdir -p src-tauri/gen/android/app/src/main/jniLibs/arm64-v8a
cp target/aarch64-linux-android/release/libsiegu_lib.so \
   src-tauri/gen/android/app/src/main/jniLibs/arm64-v8a/

# 4. Build APK
cd src-tauri/gen/android
./gradlew assembleUniversalDebug

# 5. Install
adb install -r app/build/outputs/apk/universal/debug/*.apk
```

### Android Studio (for native code editing)

```bash
npx tauri android init
npx tauri android open
```

---

## iOS

### Prerequisites

- macOS with Xcode 15+
- Rust targets: `rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios`

### Build

```bash
npm run tauri ios build
```

This produces an `.xcarchive` and `.ipa`. Code signing must be configured in Xcode for release builds.

### Development

```bash
npm run tauri ios dev
```

---

## CI Pipelines

The project uses GitHub Actions (`.github/workflows/ci.yml`) with three test jobs:

| Job | Platform | Key steps |
|-----|----------|-----------|
| `test` | macOS, Ubuntu, Windows | `cargo fmt --check`, `cargo check`, `cargo test`, `cargo clippy`, `tauri build` |
| `test-android` | Ubuntu | `cargo fmt --check`, `cargo ndk check` (aarch64 + x86_64) |
| `test-ios` | macOS | `cargo fmt --check`, `cargo check --target aarch64-apple-ios` |

Release jobs (`release`, `release-android`, `release-arch`, `release-ios`) publish binaries on GitHub Releases.

### CI environment variables

| Variable | Purpose |
|----------|---------|
| `ORT_STRATEGY=download` | Downloads pre-built ONNX Runtime binaries instead of building them |
