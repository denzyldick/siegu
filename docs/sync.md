# Mesh Sync

Siegu syncs your media library between devices over encrypted peer-to-peer connections — no cloud, no accounts.

## How it works

```
┌─────────────┐     WebRTC DTLS      ┌─────────────┐
│   Device A   │ ◄──────────────────► │   Device B   │
│  (Host)      │    encrypted P2P     │  (Joiner)    │
└──────┬───────┘                      └──────┬───────┘
       │                                      │
       └────────── Signaling ────────────────┘
                  (WebSocket)
```

1. **Host** starts a signaling server + mDNS broadcast
2. **Joiner** discovers the host (mDNS, QR code, or mnemonic phrase)
3. Devices exchange WebRTC SDP/ICE via the signaling channel
4. A direct DTLS-encrypted data channel is established
5. Devices exchange manifests (lists of photo IDs) and transfer only missing files

---

## Connectivity Methods

### LAN (Local Network)

Both devices on the same local network:

1. **Host**: Open the Connect panel → tap **Host**. A QR code and 4-word mnemonic are displayed.
2. **Joiner**: Open the Connect panel → tap **Join**. Devices appear automatically via mDNS. Alternatively, scan the QR code or type the mnemonic.

### Remote (WAN)

For devices on different networks:

1. Run Siegu's Rust signalling server on a public host (`siegu-signal`, or
   `siegu-cli serve`; see below). No separate Go service is needed.
2. Put it behind HTTPS (for example Caddy or nginx) and enter the
   `wss://your-host.example.com/ws` URL in the host.
3. Joiner connects to the same URL and room ID.

---

## Signaling Server

The same Rust server implementation (`siegu_core::lan_server`) powers LAN mode,
the `siegu web` host, and the standalone `siegu-signal` binary. WAN peers simply
point at a publicly reachable instance; there is no Go signalling service to
deploy.

### Self-hosting

Run the standalone server from the workspace root:

```bash
cargo run --release -p siegu-signal
# or, for a minimal server without token/web-dist options:
cargo run --release -p siegu-cli -- serve --port 8080
```

For a hosted web share, `docker compose up -d` starts `siegu web` behind Caddy;
Caddy terminates TLS and proxies `/ws` to the embedded signalling server.

The standalone server listens on `PORT` (default `8080`) with these env vars:

| Variable              | Default   | Description                                              |
| --------------------- | --------- | -------------------------------------------------------- |
| `PORT`                | `8080`    | Listening port                                           |
| `SIEGU_SIGNAL_TOKEN`  | _(unset)_ | If set, every join/room request must include this token  |
| `SIEGU_WEB_DIST_DIR`  | _(unset)_ | Optional webclient dist served alongside `/ws`           |

### Pre-built image

The Rust server is published as:

```bash
ghcr.io/denzyldick/siegu-signal:latest
```

Run it behind a TLS reverse proxy for WAN clients:

```bash
docker run -p 8080:8080 ghcr.io/denzyldick/siegu-signal:latest
```

---

## Sync Protocol

Once connected, the sync protocol transfers only what's missing:

1. **Manifest exchange**: Both sides share lists of photo IDs and their `sync_needed` flags
2. **File transfer**: Missing files are chunked into 64KB blocks over the WebRTC data channel
3. **Metadata sync**: AI results (captions, scores) are propagated as lightweight metadata updates
4. **Storage quota**: Configurable via `max_storage_mb` config key (default: unlimited)

### Delta sync

Only photos with `sync_needed = 1` are transferred. After a full sync, subsequent connections only transfer new/changed files.

---

## Tips

- Both devices must have the AI models downloaded to sync AI metadata
- Storage quota is enforced per-device — configure on each device separately
- For best performance, keep devices on the same LAN
- The signaling server never sees your files — only encrypted handshake data
