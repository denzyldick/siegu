# Android

## Structure

```
src-tauri/gen/android/
├── app/
│   ├── build.gradle.kts
│   └── src/main/
│       ├── AndroidManifest.xml
│       ├── java/io/denzyl/siegu/
│       │   ├── MainActivity.kt
│       │   ├── WallpaperPlugin.kt
│       │   └── generated/          # Tauri auto-generated bindings
│       └── res/                    # Icons, themes, strings
├── buildSrc/                       # Custom Gradle Rust build plugin
├── build.gradle.kts
├── settings.gradle
└── gradle.properties
```

## Permissions

From `AndroidManifest.xml`:

| Permission | Purpose |
|------------|---------|
| `INTERNET` | Networking |
| `ACCESS_NETWORK_STATE` | Connectivity checks |
| `ACCESS_WIFI_STATE` | WiFi status |
| `CAMERA` | QR scanning (optional, not required) |
| `READ_EXTERNAL_STORAGE` | Legacy storage access |
| `WRITE_EXTERNAL_STORAGE` | Legacy storage write |
| `READ_MEDIA_IMAGES` | Scoped storage (API 33+) |
| `READ_MEDIA_VIDEO` | Scoped storage (API 33+) |
| `READ_MEDIA_AUDIO` | Scoped storage (API 33+) |
| `MANAGE_EXTERNAL_STORAGE` | Full file access |
| `ACCESS_FINE_LOCATION` | GPS for photo geotagging |
| `ACCESS_COARSE_LOCATION` | Approximate location |
| `ACCESS_MEDIA_LOCATION` | EXIF GPS reading |
| `NEARBY_WIFI_DEVICES` | mDNS LAN discovery |
| `SET_WALLPAPER` | Set photo as wallpaper |

## Build Pipeline

`scripts/run-android.sh` automates:

1. Clean stale ffmpeg build artifacts
2. `bun run build` — Build Vue frontend
3. `cargo ndk -t aarch64-linux-android build --release` — Cross-compile Rust
4. Copy `libsiegu_lib.so` → `jniLibs/arm64-v8a/`
5. `./gradlew assembleUniversalDebug` — Build universal APK
6. `adb install -r` — Install on connected device

### Prerequisites

- Android SDK + NDK r27 (`~/Android/Sdk`)
- `cargo-ndk` installed
- Rust target: `rustup target add aarch64-linux-android`
- Bun/Node deps installed
- Connected device or emulator with `adb`

## Custom Plugins

### WallpaperPlugin.kt

Registers a Tauri plugin for the `set_wallpaper` command. Uses Android's `WallpaperManager` to set a photo as the device wallpaper.

### MainActivity.kt

Standard Tauri Android entry point with WebView configuration.
