# Configuration

Config is stored in the app's SQLite database (`siegu.db`) as key-value pairs.

## Config file location

| Platform | Path |
|----------|------|
| Linux | `~/.config/io.denzyl.siegu/siegu.db` |
| macOS | `~/Library/Application Support/io.denzyl.siegu/siegu.db` |
| Windows | `%APPDATA%\io.denzyl.siegu\siegu.db` |
| Android | `/data/data/io.denzyl.siegu/files/siegu.db` |
| iOS | `~/Library/Application Support/io.denzyl.siegu/siegu.db` |

Override with `--config-dir` flag on CLI commands.

## All Config Keys

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `theme` | string | `system` | `light`, `dark`, or `system` |
| `language` | string | `en` | UI language code |
| `scan_threads` | integer | `4` | Parallel scan threads (1–32) |
| `indexing_mode` | string | `immediate` | `immediate`, `idle`, or `manual` |
| `tier` | string | `free` | Feature tier: `free` or `paid` |
| `sync_path` | string | — | Custom sync download directory |
| `auto_scan` | string | — | Enable automatic scanning (`true`/`false`) |
| `sync_enabled` | string | — | Enable mesh sync (`true`/`false`) |
| `max_storage_mb` | integer | `0` (unlimited) | Max storage for synced files (1–1,000,000) |
| `model_enabled_clip` | string | — | Enable CLIP model (`true`/`false`) |
| `model_enabled_face` | string | — | Enable face detection, recognition and grouping |
| `model_enabled_ocr` | string | — | Enable OCR |
| `model_enabled_nsfw` | string | — | Enable NSFW detection |
| `model_enabled_aesthetics` | string | — | Enable aesthetics scoring |
| `model_enabled_yolo` | string | — | Enable object detection |
| `model_enabled_blip` | string | — | Enable image captioning |
| `model_enabled_arcface` | string | — | Legacy alias of `model_enabled_face` (kept in sync by the app) |
| `model_enabled_midas` | string | — | Enable depth estimation |
| `model_enabled_whisper` | string | — | Enable audio transcription |
| `model_enabled_sam` | string | — | Enable SAM segmentation |
| `model_enabled_superres` | string | — | Enable super-resolution |
| `last_scan_completed` | string | — | Timestamp of last scan (read-only) |

## CLI Usage

```bash
# View all config
siegu-cli config get

# Get a specific key
siegu-cli config get-key theme

# Set a value
siegu-cli config set theme dark

# List all valid keys
siegu-cli config keys
```

Config key validation: keys are whitelisted, values have type/range checking. Invalid values are rejected with a descriptive error.
