# CLI Reference

The `siegu` CLI provides headless access to all core features.

## Install

```bash
cargo install --path crates/siegu-cli
```

## Global Flags

| Flag | Description |
|------|-------------|
| `--config-dir <path>` | Override config directory (default: OS-appropriate path) |

---

## Commands

### `siegu scan`

Scan directories for media files and import into the library.

```bash
# Scan a specific folder (and add it to watched directories)
siegu scan /path/to/photos

# Scan all configured watched directories
siegu scan
```

### `siegu analyze`

Run ML analysis on photos.

```bash
# Analyze all unprocessed photos
siegu analyze all

# Analyze a single photo by ID
siegu analyze photo <photo-id>

# Run a specific model on all photos
siegu analyze model <model-id>
```

Model IDs: `clip`, `ultraface`, `ocr`, `nsfw`, `aesthetics`, `yolo`, `blip`, `arcface`, `midas`, `whisper`

### `siegu models`

Manage AI model files.

```bash
# List all models with download status
siegu models list

# Download all models
siegu models download

# Download specific models
siegu models download clip ultraface nsfw

# Show disk usage per model
siegu models usage
```

### `siegu config`

Manage configuration.

```bash
# View all config
siegu config get

# Get a specific key
siegu config get-key theme

# Set a config value
siegu config set theme dark

# List valid config keys
siegu config keys
```

### `siegu mesh`

Peer-to-peer mesh synchronization.

```bash
# Host a LAN sync session (starts signaling server + mDNS)
siegu mesh host

# Host on a specific port
siegu mesh host --port 9090

# Join a mesh room via signaling URL
siegu mesh join ws://192.168.1.100:8080/myroom

# Join via room ID (uses default local port)
siegu mesh join myroom

# Show session status
siegu mesh status

# Disconnect and clear saved session
siegu mesh disconnect

# Show storage quota usage
siegu mesh quota
```

### `siegu serve`

Start a standalone LAN signaling server.

```bash
siegu serve --port 8080
```

### `siegu status`

Show app overview.

```bash
siegu status
```

Output includes: config directory, database status, photo/video counts, watched folders, config values, model disk usage, available memory.
