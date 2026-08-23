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

Model IDs: `clip`, `face`, `ocr`, `nsfw`, `aesthetics`, `yolo`, `blip`, `midas`, `whisper` (`ultraface`/`arcface` are accepted as aliases of `face`)

### `siegu models`

Manage AI model files.

```bash
# List all models with download status
siegu models list

# Download all models
siegu models download

# Download specific models
siegu models download clip face nsfw

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

### `siegu web`

Share this machine's library as a **view-only** gallery in any browser (#11).
Starts an embedded signaling server plus a small static web server and prints a
one-off link. Opening the link is the consent step — anyone holding it can
browse the library read-only until the command stops. Nothing is downloaded or
written on the viewing device; media streams over the WebRTC data channel on
demand.

```bash
siegu web
# Open in a browser on this machine:
#   http://127.0.0.1:8787/#<code>.<token>
# Or from another device on this network:
#   http://192.168.1.45:8787/#<code>.<token>
```

Flags: `--port` for the static client port (default 8787), `--config` as usual.
The web bundle lives in `webclient/`; build it once with
`cd webclient && npm install && npm run build`, or point `SIEGU_WEB_DIST` at a
built `dist/` directory.

### `siegu status`

Show app overview.

```bash
siegu status
```

Output includes: config directory, database status, photo/video counts, watched folders, config values, model disk usage, available memory.
