# Web Client (View-Only Viewer)

The web client is a standalone, zero-dependency browser app that lets someone view photos shared from a Siegu host — without installing anything.

## How It Works

```
Host (Tauri app)                    Guest (Browser)
     │                                    │
     │  1. Host runs `siegu web`          │
     │     → starts signalling server     │
     │     → starts embedded web server   │
     │     → prints share URL             │
     │                                    │
     │  2. Guest opens URL ───────────────│
     │     → WebSocket to /ws             │
     │     → WebRTC offer/answer          │
     │     → Data channel established     │
     │                                    │
     │  3. Guest browses gallery          │
     │     ← thumbnails stream via DC     │
     │     ← full-res on demand           │
     │                                    │
     │  4. Session ends                   │
     │     → all blob URLs revoked        │
     │     → all data cleared from memory │
     │     → URL hash wiped               │
```

## Running Locally

### Prerequisites

- Node.js 18+
- A running Siegu host (Tauri app with `siegu web` active)

### Development Server

```bash
cd webclient
npm install
npm run dev
# → http://localhost:5173
```

The dev server proxies WebSocket connections to the host's signalling server. Open the share URL printed by `siegu web` in your browser.

### Build for Production

```bash
cd webclient
npm run build
# → dist/
```

The `dist/` folder is served by the host's embedded web server when running `siegu web`.

## Docker (Quick Test)

Test everything with one command:

```bash
docker compose up
# → http://localhost:8080
```

This starts both the signalling server and web client on **port 8080**. Nginx serves the web client and proxies WebSocket connections to the internal signalling server.

## URL Format

The share URL contains all connection info in the hash fragment:

```
http://HOST:PORT/#CODE.TOKEN
http://HOST:PORT/#CODE.TOKEN.ALBUM_ID
```

- **CODE**: Room code for the signalling server
- **TOKEN**: Authentication token
- **ALBUM_ID** (optional): If present, the viewer enters album-scoped mode (can only see photos in that album)

## Security Features

### No Persistence

- Photos stream via WebRTC data channel — nothing is downloaded to disk
- Blob URLs are revoked when the preview dialog closes
- All blob URLs are revoked when the session ends

### Session Timeout

- Sessions auto-expire after **30 minutes**
- A countdown timer is displayed in the header
- When expired, all data is wiped and the page shows "Session ended"

### Tab Visibility

- If the browser tab is hidden for **5 minutes**, the session is destroyed
- Prevents abandoned sessions from staying active

### Anti-Crawling

- `robots.txt` blocks all crawlers: `User-agent: * Disallow: /`
- Meta tags: `noindex, nofollow, noarchive, nosnippet, noimageindex`
- `Referrer-Policy: no-referrer`

### Destructor

On session end (disconnect, timeout, page close), the web client:

1. Revokes all blob URLs (`URL.revokeObjectURL`)
2. Clears all in-memory caches
3. Closes WebRTC peer connection and data channel
4. Wipes the gallery DOM
5. Replaces the URL hash (prevents re-loading the session link)

## Mobile Gestures

The web client is designed for mobile browsers:

- **Tap** a thumbnail to open the full-res preview
- **Swipe left/right** in the preview to navigate between photos
- Close the preview with the × button

## Keyboard Shortcuts

None — the web client is intentionally minimal.

## Architecture

| File | Purpose |
|------|---------|
| `index.html` | Entry point, minimal HTML shell |
| `src/main.ts` | All logic: signalling, WebRTC, gallery rendering, preview |
| `src/lib.ts` | Pure utilities: `parseHash`, `inferMime`, `assembleChunks` |
| `src/style.css` | Dark theme, grid layout, responsive design |
| `public/robots.txt` | Blocks search engine crawlers |

## Dependencies

Zero runtime dependencies. The web client is vanilla TypeScript bundled with Vite.
