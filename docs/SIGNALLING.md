# Signalling Server

Siegu uses a WebSocket signalling server to coordinate WebRTC connections between devices. The default server is `wss://siegu.io/ws`, but you can self-host your own.

For connectivity when the devices can't reach each other directly (symmetric NAT, cellular/carrier-grade NAT), Siegu also supports its built-in TURN relay — see [NAT Traversal & TURN](turn.md).

## How It Works

1. Both devices connect to the signalling server via WebSocket
2. The server relays WebRTC offer/answer/ICE candidates between peers
3. Once the peer-to-peer connection is established, the signalling server is no longer needed for data transfer
4. If no direct peer-to-peer path exists (e.g. a guest on mobile data behind carrier NAT), the media is relayed through Siegu's built-in TURN relay — encrypted in transit, never stored

```
Device A ──WebSocket──┐
                      ├── Signalling Server ──┤
Device B ──WebSocket──┘
                      │
                      └── WebRTC (direct peer-to-peer after handshake)
```

## Quick Start with Docker Compose

The repository compose file runs a full `siegu web` host behind Caddy, including
the embedded signalling server and its `/ws` bridge:

```bash
docker compose up --build
```

Local uses plain HTTP on `http://localhost/`. On a VPS, Caddy terminates TLS:

```bash
SIEGU_DOMAIN=photos.example.com docker compose up --build
# signalling is then reachable at wss://photos.example.com/ws
```

### How It Works

```
Device / Browser → Caddy (:80 or :443)
                       │
                       └── siegu-host:8787 (`siegu web`)
                                  │
                                  └── embedded signalling server + /ws bridge
```

### Services

| Service      | Port        | Description                                              |
| ------------ | ----------- | -------------------------------------------------------- |
| `caddy`      | 80 / 443    | TLS/HTTP edge, reverse proxy to the host                 |
| `siegu-host` | internal    | `siegu web` with embedded signalling and room-code API   |

## Self-Hosted Setup

### Requirements

- Docker and Docker Compose, or the Rust `siegu-signal` binary
- HTTPS/TLS in front of the server (WebRTC requires secure origins in most browsers)
- A public IP or domain name reachable by both devices

### Using the Reference Server

Siegu includes a Rust-based signalling server in `crates/siegu-signal`. It uses
the same `siegu_core::lan_server` implementation the apps embed, so no separate
Go application is needed:

```bash
# From the workspace root
cargo run --release -p siegu-signal
# Or a minimal server without token/web-dir options:
cargo run --release -p siegu-cli -- serve --port 8080
```

The server listens on port 8080 by default. Set `RUST_LOG` for logging:

```bash
RUST_LOG=info cargo run --release -p siegu-signal
```

For WAN clients, put it behind a TLS reverse proxy that forwards `/ws`.

### Environment Variables

| Variable             | Default | Description                                         |
| -------------------- | ------- | --------------------------------------------------- |
| `PORT`               | `8080`  | Listening port                                      |
| `RUST_LOG`           | `warn`  | Log level (`info`, `debug`, `trace`)                |
| `SIEGU_SIGNAL_TOKEN` | None    | Token required for all connections (authentication) |
| `SIEGU_WEB_DIST_DIR` | None    | Path to webclient dist for serving the viewer SPA   |

### Configuration in Siegu

1. Open **Settings → Signalling**
2. Enter your server URL (e.g., `wss://your-server.example.com/ws`)
3. If your server requires authentication, enter a token
4. Click **Test connection** to verify
5. Click **Save**

### Docker (Standalone)

Build from the repository root with the crate's Dockerfile:

```bash
docker build -f crates/siegu-signal/Dockerfile -t siegu-signal .
docker run -p 8080:8080 siegu-signal
```

Or use the published image:

```bash
docker run -p 8080:8080 ghcr.io/denzyldick/siegu-signal:latest
```

## Token Authentication

If your signalling server enforces token-based auth:

1. Set the token in **Settings → Signalling → Signalling token**
2. The token is appended as a query parameter: `wss://your-server/ws?token=YOUR_TOKEN`
3. Your server must validate the token during the WebSocket handshake

## Testing the Web Client

### From Docker Compose

```bash
# Start everything on port 8080
docker compose up

# Open in browser
open http://localhost:8080
```

### With a Real Host

1. Open Siegu on your computer
2. Go to **Settings → Signalling** and configure your server
3. Open a collection → tap "Share" → copy the link
4. Open the link in a mobile browser or another device

The viewer will connect via the signalling server and stream the collection.

## Troubleshooting

- **"Test connection" fails**: Check that the URL is reachable and the server is running
- **Devices can't connect**: Ensure both devices can reach the signalling server URL
- **Sync works on Wi-Fi but not on mobile data**: The guest is likely behind carrier-grade NAT and needs a TURN relay — see [NAT Traversal & TURN](turn.md)
- **Connection drops**: Check server logs for disconnection reasons; verify TLS certificates are valid
- **Web client shows "Missing session link"**: The URL hash must contain CODE.TOKEN — make sure you're using the full share URL
- **Web client shows "Session ended" immediately**: The signalling server may not be running or the room expired

## Use Cases

### Device-to-Device Sync

Two Tauri instances syncing libraries over the internet:

```
Phone (Tauri) ──WebSocket── Signalling ──WebSocket── Desktop (Tauri)
                    │
                    └── WebRTC (direct sync after handshake)
```

### Collection Sharing

Share a collection with someone who doesn't have Siegu:

```
You (Tauri) ──WebSocket── Signalling ──WebSocket── Friend (Browser)
                    │
                    └── WebRTC (view-only streaming)
```

## Related Documentation

- [NAT Traversal & TURN](turn.md) — keeping connections working over cellular / restrictive networks
- [Web Client](webclient.md) — View-only browser viewer
- [Collection Sharing](sharing.md) — How sharing works
- [Security](security.md) — Privacy and encryption details
