# Signalling Server

Siegu uses a WebSocket signalling server to coordinate WebRTC connections between devices. The default server is `wss://siegu.io/ws`, but you can self-host your own.

## How It Works

1. Both devices connect to the signalling server via WebSocket
2. The server relays WebRTC offer/answer/ICE candidates between peers
3. Once the peer-to-peer connection is established, the signalling server is no longer needed for data transfer

```
Device A ──WebSocket──┐
                      ├── Signalling Server ──┤
Device B ──WebSocket──┘
                      │
                      └── WebRTC (direct peer-to-peer after handshake)
```

## Quick Start with Docker Compose

The fastest way to run both the signalling server and web client:

```bash
docker compose up
```

This gives you everything on **one port** (`http://localhost:8080`):
- Web client (static files)
- WebSocket signalling (proxied to the internal signalling server)

### How It Works

```
Browser → http://localhost:8080
              │
              ├── /          → nginx serves webclient
              └── /ws        → nginx proxies to signalling:8080
                                    │
                                    └── signalling server (internal)
```

### Services

| Service | Port | Description |
|---------|------|-------------|
| `webclient` | 8080 | nginx serves webclient + proxies WebSocket |
| `signalling` | internal | WebSocket signalling server (not exposed directly) |

## Self-Hosted Setup

### Requirements

- Docker and Docker Compose (or a WebSocket server)
- HTTPS/TLS (WebRTC requires secure origins in most browsers)
- A public IP or domain name reachable by both devices

### Using the Reference Server

Siegu includes a Rust-based signalling server in the `siegu-signal` crate:

```bash
# Build and run the reference server
cd siegu-signal
cargo run --release
```

The server listens on port 8080 by default. Set the `RUST_LOG` environment variable for logging:

```bash
RUST_LOG=info cargo run --release
```

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `PORT` | `8080` | Listening port |
| `RUST_LOG` | `warn` | Log level (`info`, `debug`, `trace`) |
| `SIEGU_SIGNAL_TOKEN` | None | Token required for all connections (authentication) |
| `SIEGU_WEB_DIST_DIR` | None | Path to webclient dist for serving the viewer SPA |

### Configuration in Siegu

1. Open **Settings → Signalling**
2. Enter your server URL (e.g., `wss://your-server.example.com/ws`)
3. If your server requires authentication, enter a token
4. Click **Test connection** to verify
5. Click **Save**

### Docker (Standalone)

```bash
docker build -t siegu-signal .
docker run -p 8080:8080 siegu-signal
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

- [Web Client](webclient.md) — View-only browser viewer
- [Collection Sharing](sharing.md) — How sharing works
- [Security](security.md) — Privacy and encryption details
