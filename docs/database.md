# Database Schema

File: `crates/siegu-core/src/database.rs`

12 tables for photos, AI results, devices, and configuration.

## `photo` — Main media table

| Column | Type | Notes |
|--------|------|-------|
| `id` | TEXT PK | SHA-256 of path |
| `location` | TEXT | UNIQUE index |
| `encoded` | TEXT | Base64 320px thumbnail |
| `created` | TEXT (ISO8601) | Indexed |
| `latitude` | REAL | Spatial index with longitude |
| `longitude` | REAL | Spatial index with latitude |
| `indexed` | INTEGER | 0=new, 1=metadata, 2=fully processed |
| `caption` | TEXT | BLIP-generated caption |
| `aesthetics_score` | REAL | Aesthetics model score |
| `favorite` | INTEGER | Default 0 |
| `sync_needed` | INTEGER | Default 0, set to 1 for new/changed files |

## `ai_status` — Per-photo model processing flags

One column per model, all `INTEGER DEFAULT 0`:

`clip`, `face`, `ocr`, `nsfw`, `aesthetics`, `yolo`, `blip`, `arcface`, `midas`, `whisper`, `sam`, `superres`

FK `photo_id`, UNIQUE constraint on `photo_id`.

## `faces` — Detected faces

| Column | Type | Notes |
|--------|------|-------|
| `face_id` | TEXT PK | UUID |
| `photo_id` | TEXT | FK → photo(id) ON DELETE CASCADE |
| `crop_path` | TEXT | Filesystem path to cropped face image |
| `encoded` | TEXT | Base64 thumbnail |
| `embedding` | BLOB | 512xf32 ArcFace embedding |
| `person_id` | TEXT | FK → people(id) ON DELETE SET NULL |

## `people` — Person identities

| Column | Type | Notes |
|--------|------|-------|
| `id` | TEXT PK | UUID |
| `name` | TEXT | Nullable (null = unnamed cluster) |
| `embedding` | BLOB | 512xf32 centroid embedding |

## `peer_device` — Mesh-connected devices

| Column | Type | Notes |
|--------|------|-------|
| `device_id` | TEXT PK | UUID |
| `name` | TEXT | Device hostname |
| `ip` | TEXT | IP address |
| `port` | INTEGER | Signaling port |
| `device_type` | TEXT | "desktop", "mobile", etc. |
| `os` | TEXT | OS name |
| `models_enabled` | TEXT | JSON array of model names |
| `protocol_version` | INTEGER | Sync protocol version |
| `storage_used` | INTEGER | Storage consumed for sync |
| `storage_capacity` | INTEGER | Max storage limit |
| `last_seen` | TEXT | ISO8601 timestamp |

## `ocr` — OCR results

| Column | Type | Notes |
|--------|------|-------|
| `photo_id` | TEXT | FK → photo(id) ON DELETE CASCADE |
| `text` | TEXT | Recognized text |

## `directory` — Monitored folders

| Column | Type | Notes |
|--------|------|-------|
| `path` | TEXT PK | Canonical directory path |
| `added` | TEXT | Timestamp |

## `object` — YOLO object detections

| Column | Type | Notes |
|--------|------|-------|
| `photo_id` | TEXT | FK → photo(id) ON DELETE CASCADE |
| `class_name` | TEXT | COCO class label |
| `probability` | REAL | Confidence score (≥0.5) |

## `properties` — Photo metadata key/value store

| Column | Type | Notes |
|--------|------|-------|
| `photo_id` | TEXT | FK → photo(id) ON DELETE CASCADE |
| `key` | TEXT | Property name |
| `value` | TEXT | Property value |

Stores EXIF metadata (camera make/model, lens, ISO, etc.), GPS reverse-geocode results, transcriptions, and sync status.

## `device` — Legacy paired devices

| Column | Type | Notes |
|--------|------|-------|
| `id` | TEXT | |
| `name` | TEXT | |

## `config` — Key/value app configuration

| Column | Type | Notes |
|--------|------|-------|
| `key` | TEXT PK | Config key name |
| `value` | TEXT | Config value |

## `logs` — App log entries

| Column | Type | Notes |
|--------|------|-------|
| `timestamp` | TEXT | ISO8601 |
| `level` | TEXT | "info", "warn", "error" |
| `message` | TEXT | Log message |

## `album` — User-created photo collections (local, free tier)

| Column | Type | Notes |
|--------|------|-------|
| `id` | TEXT PK | Album id (UUID) |
| `name` | TEXT | Album display name |
| `created_at` | TEXT | ISO8601 creation time |
| `cover_photo_id` | TEXT | Id of the last-added photo used as the cover |
| `sort_order` | INTEGER | Manual ordering of the albums list |

## `album_item` — Membership of a photo in an album

| Column | Type | Notes |
|--------|------|-------|
| `album_id` | TEXT PK | Owning album id |
| `photo_id` | TEXT PK | Photo id (duplicates are rejected) |
| `added_at` | TEXT | ISO8601 time the photo was added |
| `position` | INTEGER | Manual order within the album |

Album contents are served ordered by `position ASC, added_at DESC`. Deleting an album cascades to `album_item`. Sharing albums is out of scope for the free tier (see the paid-tier issue).

## Indexes

- `photo.location` UNIQUE
- `photo.created` ASC
- `photo.latitude` + `photo.longitude` spatial index
- `album_item(album_id, position)` ASC
- `album_item(photo_id)`

## Migrations

The schema is created on first run via `CREATE TABLE IF NOT EXISTS` statements. Column additions between versions use `ALTER TABLE ADD COLUMN IF NOT EXISTS` (with a pragma-based fallback for older SQLite versions).
