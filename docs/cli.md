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

All three forms accept `--headless`, which prints progress lines and an E2E
summary instead of showing the interactive TUI (used by CI scripts).

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
# Host a LAN sync session (starts a signaling server + mDNS)
siegu mesh host

# Host on a specific port (default 0 = pick a free port)
siegu mesh host --port 9090

# Host against an existing signaling server, joining/creating a room
siegu mesh host --server ws://192.168.1.100:8080 --room myroom

# Join a mesh room (room ID is positional; URL goes in --server)
siegu mesh join myroom
siegu mesh join myroom --server ws://192.168.1.100:8080

# When joining a --server host with a pre-agreed room, create the WebRTC offer
siegu mesh join myroom --server ws://192.168.1.100:8080 --initiator

# Show session status / disconnect / storage quota usage
siegu mesh status
siegu mesh disconnect
siegu mesh quota

# Browse a peer's library view-only and verify the manifest (#9, e2e helper)
siegu mesh browse myroom --server ws://192.168.1.100:8080

# Browse in album-share mode for a specific album (#16)
siegu mesh browse myroom --album <album-id>

# Send a single RPC command to the peer and print its reply (#19, e2e helper)
siegu mesh rpc myroom list_files --server ws://192.168.1.100:8080
siegu mesh rpc myroom toggle_favorite '{"id": "<photo-id>"}'

# Seed a manual album with the first N photos (prints ALBUM ID, e2e helper)
siegu mesh seed-album --name "Shared" --take-first 5
```

Availability and defaults:
- `host` — `--server` connects to an existing signaling server instead of
  starting a local one; `--room` names the room (required with `--server`).
  `--share-mode <ro|rw>` sets the permission level for connected peers
  (default `ro`): `ro` allows browsing only, `rw` also allows
  favorites/trash mutations.
- `join` — `--initiator` makes this peer create the WebRTC offer (needed when
  joining a `--server` host).
- `browse`, `rpc`, and `seed-album` are e2e test helpers; they print greppable
  `VIEWONLY`, `RPC RESULT`, and `ALBUM ID` markers used by
  `scripts/e2e-view-only.sh`.
- All mesh subcommands accept `-c/--config` to point at a config directory.

### Remote sync via mesh

There is no separate `siegu sync` command. Remote (non-LAN) sync is done with
the mesh commands above — host on one machine, join on the other — using a
signaling server URL in `--server`.

### `siegu serve`

Start a standalone LAN signaling server.

```bash
siegu serve --port 8080
```

### `siegu web`

Share this machine's library as a browser gallery (#11, #19). Starts an
embedded signaling server plus a small static web server and prints a one-off
link. Opening the link is the consent step — anyone holding it can browse the
library until the command stops. Nothing is downloaded or written on the
viewing device; media streams over the WebRTC data channel on demand.

```bash
siegu web
# Open in a browser on this machine:
#   http://127.0.0.1:8787/#<code>.<token>
# Or from another device on this network:
#   http://192.168.1.45:8787/#<code>.<token>
```

Flags: `--port` for the static client port (default 8787), `--config` as usual,
and `--share-mode <ro|rw>` (default `ro`). The default `ro` is view-only — guests
can browse but not change anything. `rw` additionally lets guests toggle
favorites and trash photos. The browser webclient itself is read-only; the
`rw` mode is consumed by the mesh `browse`/`rpc` test helpers.

The web bundle lives in `webclient/`; build it once with
`cd webclient && bun install && bun run build` (npm also works), or point
`SIEGU_WEB_DIST` at a built `dist/` directory.

### `siegu status`

Show app overview.

```bash
siegu status
```

Output includes: config directory, database status, photo/video counts, watched folders, config values, model disk usage, available memory.
