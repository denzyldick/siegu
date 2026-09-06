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

## Network / TURN

Siegu ships with a built-in TURN relay that runs on the host when enabled. It
hands guests ICE credentials automatically; guests behind restrictive networks
(mobile data, carrier-grade NAT) can still connect through it. See
[NAT Traversal & TURN](turn.md) for the full setup.

### Config keys

Saved via `save_config` (the app's Settings or the config file):

| Key | Default | Meaning |
|-----|---------|---------|
| `turn_enabled` | `false` | `true`/`false` — start the built-in relay at launch |
| `turn_port` | `0` | Relay UDP port; `0` picks a free port automatically |
| `turn_public_host` | empty | Public IP of this device; empty = auto-detect the LAN IP |
| `turn_username` | auto | Credentials the app generated (left alone once set) |
| `turn_password` | auto | Credentials the app generated (left alone once set) |

`turn_public_host` must be an IP address (or empty — auto-detected). Port must
be a `u16` (0–65535).

### Host environment variables (external relay override)

The host reads these from its environment when creating WebRTC connections. The
built-in relay sets them itself; setting `SIEGU_TURN_URLS` before launch **skips
the built-in relay** and uses your own instead.

| Variable | Default | Description |
|----------|---------|-------------|
| `SIEGU_TURN_URLS` | set by app | Comma-separated TURN URLs, e.g. `turn:home.example.com:3478` |
| `SIEGU_TURN_USERNAME` | set by app | TURN username (only needed if the relay has auth) |
| `SIEGU_TURN_CREDENTIAL` | set by app | TURN password/credential |
