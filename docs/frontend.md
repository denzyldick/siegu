# Frontend Architecture

Tech: Vue 3 + TypeScript + Vuetify 3 + Vite + Pinia

## Directory Structure

```
src/
├── composables/     # 7 composables (useConnect, useSettings, etc.)
├── stores/          # 6 Pinia stores
├── components/      # 50+ Vue components in 8 subdirectories
│   ├── connect/     # Connection dialogs (ConnectModeToggle, ConnectJoinView, etc.)
│   ├── media/       # Media display (MediaGrid, MediaViewer, etc.)
│   ├── settings/    # Settings panels
│   ├── sync/        # Sync progress UI
│   ├── models/      # Model management UI
│   ├── people/      # Face/person management
│   ├── scan/        # Scan progress UI
│   └── common/      # Shared components (AppToolbar, NameDialog, etc.)
├── types/           # TypeScript type definitions
├── services/        # Tauri IPC wrappers
└── locales/         # 8 languages (en, nl, fr, de, es, it, pt, pap)
```

## State Management (Pinia Stores)

| Store | Purpose |
|-------|---------|
| `app` | Initialization state, onboarding completion |
| `ui` | Theme (light/dark/system), sidebar visibility, current view |
| `search` | Query, results, filters (favorites, videos, date range), pagination |
| `sync` | Sync progress, peers, connection status |
| `scan` | Scan progress, file counts |
| `models` | Model download status, progress |

## Composables

| Composable | Purpose |
|------------|---------|
| `useConnect` | Host/join state machine, mDNS discovery, WebRTC event listeners |
| `useSettings` | Config read/write via Tauri commands |
| `usePeople` | Face/person CRUD operations |
| `useLocale` | i18n locale switching |
| `useMediaUtils` | Media filtering (videos-only, favorites) |
| `useMediaUrl` | Resolves thumbnail/original URLs via media server |
| `useTauri` | Typed invoke wrapper for Tauri IPC |

## Event System

Tauri events bridge Rust backend → Vue frontend:

| Event | Payload | Source |
|-------|---------|--------|
| `scan-progress` | `ScanProgress` | File scanner |
| `indexing-progress` | `IndexingProgress` | ML worker |
| `indexing-eta` | string | ML worker |
| `sync-progress` | `SyncProgress` | Mesh transport |
| `webrtc-state` | string | TauriSyncEvent |
| `peer-connected` | `PeerDevice` | TauriSyncEvent |
| `peer-disconnected` | string (peer_id) | TauriSyncEvent |
| `room-code` | string | Pairing code |
| `photo-analysis-result` | Photo ID | ML worker |
| `model-progress` | `ModelProgress` | Downloader |
| `download-progress` | `DownloadProgress` | Model download |
| `sync-error` | string | Mesh transport |
| `photo-received` | `Photo` | TauriSyncEvent |

## Connection Flow

1. User selects **Host** or **Join** mode via `ConnectModeToggle`
2. **Host**: Generates pairing codes, starts LAN signaling server, registers mDNS service, shows QR + passphrase
3. **Join**: Discovers LAN hosts via mDNS polling, or scans QR / enters passphrase
4. After WebRTC handshake, `peer-connected` fires → peer list updates
5. Sync progress flows via `sync-progress` events

## Locales

8 language files in `src/locales/`:

- `en.json` — English
- `nl.json` — Dutch
- `fr.json` — French
- `de.json` — German
- `es.json` — Spanish
- `it.json` — Italian
- `pt.json` — Portuguese
- `pap.json` — Papiamento

Locale keys are validated against a canonical key list. Add new keys to all locales or the translation check script (`scripts/check-translations.js`) will fail CI.
