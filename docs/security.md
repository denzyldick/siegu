# Security & Privacy

## Core Principles

- **Local-First AI**: All 9 AI models run on-device via ONNX Runtime. No data leaves your computer.
- **Zero Telemetry**: No analytics, crash reporting, or usage tracking.
- **No Accounts**: No user accounts, no cloud sync, no sign-up.

## Data Protection

### Model Downloads

- Model files are downloaded from HuggingFace and other sources over HTTPS
- SHA-256 hash verification ensures file integrity
- Files with mismatched hashes are deleted automatically

### Peer-to-Peer Sync

- **DTLS Encryption**: WebRTC data channels are encrypted with DTLS in transit
- **Zero-Knowledge Signaling**: The signaling server relays only encrypted SDP/ICE data — it never sees files, metadata, or manifests
- **No Persistent Server**: LAN mode uses a local signaling server; remote mode uses a minimal relay that has no access to content

### File Handling

- **Filename Sanitization**: Synced filenames are sanitized to prevent path traversal and control character injection
- **Temp Landing**: Received files land in `sync_temp/` first, are verified, then moved to the library
- **Storage Quota**: Configurable limit prevents disk exhaustion

## Application Security

### Graceful Shutdown

`ShutdownCoordinator` signals all background tasks (scan, sync, ML) before exit, preventing data corruption.

### Transaction Safety

- All ML batch writes use `BEGIN` / `COMMIT` / `ROLLBACK`
- Periodically runs `VACUUM` for database maintenance

### Config Validation

- All 22 config keys are whitelisted (unknown keys rejected)
- Values are type-checked with range constraints (e.g., `scan_threads`: 1–32)
- Invalid values are rejected with a descriptive error

### Scan Deduplication

- `ScanGuard` prevents concurrent scan operations
- Each scan skips paths already in the database

## Vulnerability Reporting

Report security issues to the project maintainer via GitHub Issues or direct contact. Do not file public issues for critical vulnerabilities.
