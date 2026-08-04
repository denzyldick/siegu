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

1. Deploy the Go signaling server (see below)
2. Host enters the signaling server URL
3. Joiner connects using the same room ID

---

## Signaling Server

LAN mode runs a built-in signaling server (embedded in the app). For remote connections, a standalone signaling server (`siegu-signal`, written in Rust) is available.

### Self-hosting

```bash
docker compose up -d
```

The server runs on port `8080` with these env vars:

| Variable | Default | Description |
|----------|---------|-------------|
| `PORT` | `8080` | Listen address |
| `SIEGU_SIGNAL_TOKEN` | *(unset)* | If set, every join/room request must include this token |

### Pre-built image

```
ghcr.io/denzyldick/siegu-signal:latest
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
