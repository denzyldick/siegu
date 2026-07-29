# Mesh Protocol

## Discovery

### mDNS

- Hosts register `_siegu._tcp.local.` services with TXT records: `protocol_version`, `room_id`
- Joiners discover via `discover_hosts()` (timeout scan) or `watch_hosts()` (continuous channel)
- Implemented in `crates/siegu-core/src/mdns.rs`

### Mnemonic / QR

- BIP39 generates a 6-word passphrase + UUID
- SHA-256(passphrase) → `room_id`
- Host displays QR code; joiner scans or types phrase
- Implemented in `crates/siegu-core/src/server.rs`

### Manual

Direct IP + port entry for LAN hosts.

## Signaling Protocol

17 message types over WebSocket JSON in `crates/siegu-core/src/signal.rs`:

| Message | Direction | Purpose |
|---------|-----------|---------|
| `Join` | Client → Server | Join a room with device_id + metadata |
| `Joined` | Server → Client | Confirmation + peer_count |
| `PeerJoined` | Server → Client | Another device joined |
| `PeerDisconnected` | Server → Client | Peer left the room |
| `Offer` | Peer → Peer | WebRTC SDP offer |
| `Answer` | Peer → Peer | WebRTC SDP answer |
| `IceCandidate` | Peer → Peer | ICE candidate |
| `Relay` | Peer → Peer | Relay message through intermediate peer |
| `RoomClosed` | Server → Client | Room was destroyed |
| `DeviceInfo` | Bidirectional | Device metadata exchange |
| `Ping` | Bidirectional | Keepalive |
| `Pong` | Bidirectional | Keepalive response |
| `Error` | Server → Client | Error with description |
| `JoinError` | Server → Client | Join rejected |
| `Kick` | Server → Client | Force disconnect |
| `RoomList` | Server → Client | Available rooms |
| `DirectMessage` | Peer → Peer | Arbitrary direct message |

### LAN Signaling

Built-in warp WebSocket server (`lan_server.rs`):

- Manages rooms as `HashMap<String, Room>` 
- Max 5 clients per room (`MAX_MESH_DEVICES`)
- Relays Offer/Answer/ICE between peers
- No external server needed

### Remote Signaling

Connects to a Go signaling server.

- Default: `wss://siegu.io/ws`
- Self-hosted via `docker-compose.yml`

## WebRTC Transport

File: `crates/siegu-core/src/mesh_transport.rs`

`MeshTransport` wraps WebSocket + WebRTC:

1. Connect to signaling WebSocket
2. Send `Join` with room_id + device metadata
3. Receive `Joined` confirmation (peer_count indicates if others are present)
4. When another peer arrives, initiate WebRTC handshake
5. Create RTCDataChannel for sync messages
6. Incoming messages muxed to `MeshManager::handle_sync_message()`
7. Supports relay mode through intermediate peers (mesh topology)

## Sync Protocol

File: `crates/siegu-core/src/mesh.rs`

Once a WebRTC data channel is established, `SyncMessage` enum drives the protocol:

| Message | Purpose |
|---------|---------|
| `ManifestRequest` / `ManifestResponse` | Exchange photo inventory (IDs + sync_needed flags) |
| `FileRequest` | Request specific photo by ID |
| `FileHeader` | Metadata before transfer (size, name, mime) |
| `FileChunk` | 64KB chunk of file data |
| `FileEnd` | Signal transfer complete (with SHA-256 hash) |
| `StartSync` | Trigger sync session |
| `MetadataUpdate` | Propagate AI results (caption, score, indexed state) |
| `VersionNegotiate` / `VersionReject` | Protocol version handshake |
| `CatchUp` / `CatchUpDone` | Incremental sync after reconnection |

### File Transfer

- **Chunk size**: 64KB
- **Retry**: Automatic on failure
- **Landing**: Files received to `sync_temp/` directory
- **Verification**: SHA-256 hash checked against `FileEnd` message
- **Import**: Verified files moved to library folder, imported via `import_photo()`

### Storage Quota

- Configurable via `max_storage_mb` config key
- Enforced pre-receive in `FileHeader` handler
- Quota checked by walking sync directory size

### Delta Sync

- Only photos with `sync_needed = 1` are transferred
- After full sync, subsequent connections only transfer new/changed files
- `MetadataUpdate` messages propagate AI results without re-transferring files
