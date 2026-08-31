# Collection Sharing

Siegu allows you to share individual collections (albums) with others in a read-only, view-only mode.

## How It Works

```
You (Host)                           Friend (Guest)
     │                                    │
     │  1. Open a collection              │
     │     → tap "Share" in menu          │
     │     → see sharing info             │
     │                                    │
     │  2. Share via signalling server    │
     │     → connection coordinated       │
     │     → WebRTC data channel opens    │
     │                                    │
     │  3. Friend views in browser        │
     │     → opens share link             │
     │     → sees ONLY your collection    │
     │     → photos stream, not downloaded│
     │                                    │
     │  4. Session ends                   │
     │     → all data cleared             │
```

## Sharing Requirements

Collection sharing requires a **signalling server** to coordinate the secure WebRTC connection between your device and the viewer's browser.

### Hosted Service

The easiest option: [siegu.io/connect](https://siegu.io/connect)

- No setup required
- Works across networks (not just LAN)
- Secure, encrypted connections

### Self-Hosted

Run your own signalling server. See [SIGNALLING.md](SIGNALLING.md) for setup instructions.

## Desktop App (local, for now)

The desktop app's **Share Collection** action (`… → Share Collection`) starts a
local signalling + web server and generates a browser share link
(`http://127.0.0.1:PORT/#CODE.TOKEN.ALBUM`). Opening it loads the view-only web
client and shows only that collection; granting/copying/stopping happens right
in the dialog. For now the link works on the same computer — cross-network
sharing via the hosted `siegu.io` relay is the upcoming default (see
`src/services/appConfig.ts` for the single base-domain switch).

## What the Viewer Can See

- **Only the shared collection** — the viewer cannot access other photos or collections
- **Read-only** — no editing, deleting, or modifying capabilities
- **Streaming only** — photos are not downloaded to the viewer's device

## What the Viewer Cannot See

- Other collections or the full library
- File system paths or device information
- AI analysis results (unless included in the stream)
- Any metadata beyond what's in the shared photos

## Privacy & Security

### Data Flow

1. Photos stream directly from your device to the viewer via WebRTC
2. No photos pass through any server
3. The signalling server only coordinates the connection (relays encrypted SDP/ICE data)

### Session Security

- Sessions auto-expire after 30 minutes
- All data is cleared from the viewer's browser when the session ends
- Blob URLs are revoked to prevent cached access
- The URL hash is cleared to prevent re-loading

### Anti-Crawling

- `robots.txt` blocks all search engines
- Meta tags prevent indexing of shared content
- No server-side caching of shared photos

## Mobile Photo Viewer

When viewing shared photos on mobile:

- **Swipe left/right** — navigate between photos
- **Swipe up** — jump to previous time period
- **Swipe down** — jump to next time period
- **Double-tap** — toggle favorite (if connected to your library)

## Troubleshooting

### "Session ended" immediately

- The signalling server may not be running
- Check that the URL is correct (includes CODE.TOKEN in the hash)

### Photos not loading

- Ensure both devices can reach the signalling server
- Check firewall settings for WebRTC (STUN/TURN ports)
- Try a different network if on restrictive WiFi

### Viewer sees "Access denied"

- The collection may not exist or the link may be expired
- Ask the host to generate a new share link

## Architecture

```
Host Device (Tauri)
  │
  ├── Embedded web server (serves webclient/dist/)
  ├── Embedded signalling server (LAN mode)
  │
  └── WebRTC data channel
        │
        └── Guest Browser
              ├── webclient/index.html
              ├── WebRTC peer connection
              └── Gallery + Preview UI
```
