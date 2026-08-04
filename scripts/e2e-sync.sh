#!/usr/bin/env bash
set -euo pipefail

# Transient end-to-end mesh sync validation.
#
# Exercises the REAL sync stack the app ships: `mesh host` brings up a LAN
# signaling server + WebRTC transport (initiator), `mesh join --server`
# connects a second CLI instance as the peer, the data channel opens, protocol
# messages are exchanged, and a photo scanned on the host actually transfers
# to the joiner with a byte-for-byte SHA-256 match.
#
# Usage:
#   scripts/e2e-sync.sh
#
# Env:
#   SIEGU_BIN          path to the siegu CLI binary (default: target/release/siegu)
#   SIEGU_E2E_PHOTOS   directory of media to sync (default: tests/fixtures/faces)
#   SIEGU_SIGNAL_URL   ws(s):// URL of an EXTERNAL signaling server to use
#                      instead of the host's in-process one (Docker job).
#
# Used by the `mesh-e2e` jobs in .github/workflows/ubuntu.yml, macos.yml and
# windows.yml, and by the post-publish job in signal-docker.yml.

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN="${SIEGU_BIN:-$REPO_ROOT/target/release/siegu}"
PHOTOS="${SIEGU_E2E_PHOTOS:-$REPO_ROOT/tests/fixtures/faces}"

if [ ! -x "$BIN" ] && [ ! -x "${BIN}.exe" ]; then
  echo "siegu binary not found at $BIN. Run: cargo build --release -p siegu-cli" >&2
  exit 1
fi
if [ -x "${BIN}.exe" ]; then
  BIN="${BIN}.exe"
fi
if [ ! -d "$PHOTOS" ]; then
  echo "photo directory not found: $PHOTOS" >&2
  exit 1
fi

sha256_of() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}

WORK="$(mktemp -d)"
HOST_CFG="$WORK/host"
JOIN_CFG="$WORK/join"
mkdir -p "$HOST_CFG" "$JOIN_CFG"
HOST_PID=""
JOIN_PID=""

cleanup() {
  [ -n "$HOST_PID" ] && kill "$HOST_PID" 2>/dev/null || true
  [ -n "$JOIN_PID" ] && kill "$JOIN_PID" 2>/dev/null || true
  # The peers can still be holding their SQLite/WAL files open for a moment
  # after kill (especially on Windows/MSYS), which makes `rm -rf` fail with
  # "Device or resource busy" and, under `set -e`, flips the CI exit code.
  # Give them a moment to exit, then clean up best-effort.
  for _ in $(seq 1 50); do
    ALIVE=""
    [ -n "$HOST_PID" ] && kill -0 "$HOST_PID" 2>/dev/null && ALIVE=1
    [ -n "$JOIN_PID" ] && kill -0 "$JOIN_PID" 2>/dev/null && ALIVE=1
    [ -z "$ALIVE" ] && break
    sleep 0.1
  done
  rm -rf "$WORK" 2>/dev/null || true
}
trap cleanup EXIT

echo "== scanning host library =="
"$BIN" --config-dir "$HOST_CFG" scan "$PHOTOS" 2>&1 | tail -1

SOURCE_PHOTO="$(
  find "$PHOTOS" -maxdepth 1 -type f \( -iname '*.jpg' -o -iname '*.jpeg' -o -iname '*.png' -o -iname '*.heic' -o -iname '*.webp' \) | head -1
)"
[ -n "$SOURCE_PHOTO" ] || { echo "no media file found in $PHOTOS" >&2; exit 1; }
EXPECTED_SHA="$(sha256_of "$SOURCE_PHOTO")"
BASENAME="$(basename "$SOURCE_PHOTO")"
echo "syncing $BASENAME (sha256=$EXPECTED_SHA)"

if [ -n "${SIEGU_SIGNAL_URL:-}" ]; then
  echo "== host connecting to external signaling server =="
  ROOM_ID="e2e-$(date +%s)"
  "$BIN" --config-dir "$HOST_CFG" mesh host --server "$SIEGU_SIGNAL_URL" --room "$ROOM_ID" >"$WORK/host.log" 2>&1 &
  HOST_PID=$!
  JOIN_ARGS=(mesh join "$ROOM_ID" --server "$SIEGU_SIGNAL_URL")
else
  echo "== starting in-process LAN signaling server =="
  "$BIN" --config-dir "$HOST_CFG" mesh host --port 0 >"$WORK/host.log" 2>&1 &
  HOST_PID=$!
  ROOM_ID=""
  PORT=""
  for _ in $(seq 1 60); do
    ROOM_ID="$(grep -oE 'Room ID: [0-9a-f-]+' "$WORK/host.log" | sed -E 's/Room ID: //' | head -1 || true)"
    PORT="$(grep -oE 'Signaling server on port [0-9]+' "$WORK/host.log" | sed -E 's/Signaling server on port //' | head -1 || true)"
    [ -n "$ROOM_ID" ] && [ -n "$PORT" ] && break
    sleep 0.5
  done
  [ -n "$ROOM_ID" ] || { echo "host did not print a room id" >&2; tail -20 "$WORK/host.log" >&2; exit 1; }
  [ -n "$PORT" ] || { echo "host did not print a signaling port" >&2; tail -20 "$WORK/host.log" >&2; exit 1; }
  JOIN_ARGS=(mesh join "$ROOM_ID" --server "ws://127.0.0.1:$PORT")
fi

echo "== starting joiner =="
"$BIN" --config-dir "$JOIN_CFG" "${JOIN_ARGS[@]}" >"$WORK/join.log" 2>&1 &
JOIN_PID=$!

echo "== waiting for WebRTC data channel + peer handshake =="
CONNECTED=""
for _ in $(seq 1 90); do
  if grep -q "Secure Data Channel Ready" "$WORK/host.log" && grep -q "\[sync\] Connected" "$WORK/join.log"; then
    CONNECTED=1
    break
  fi
  if ! kill -0 "$HOST_PID" 2>/dev/null; then
    echo "host exited unexpectedly" >&2
    tail -40 "$WORK/host.log" >&2
    exit 1
  fi
  if ! kill -0 "$JOIN_PID" 2>/dev/null; then
    echo "joiner exited unexpectedly" >&2
    tail -40 "$WORK/join.log" >&2
    exit 1
  fi
  sleep 1
done
[ -n "$CONNECTED" ] || {
  echo "FAIL: data channel / handshake did not complete within 90s" >&2
  echo "--- host.log ---" >&2
  tail -40 "$WORK/host.log" >&2
  echo "--- join.log ---" >&2
  tail -40 "$WORK/join.log" >&2
  exit 1
}
echo "OK: data channel open, peers exchanged VersionNegotiate"

echo "== waiting for file transfer =="
RECV_FILE="$JOIN_CFG/Siegu/siegu/$BASENAME"
TRANSFERRED=""
for _ in $(seq 1 60); do
  if [ -s "$RECV_FILE" ]; then
    TRANSFERRED=1
    break
  fi
  sleep 1
done
[ -n "$TRANSFERRED" ] || {
  echo "FAIL: received file not found at $RECV_FILE" >&2
  echo "--- join.log ---" >&2
  tail -40 "$WORK/join.log" >&2
  echo "--- host.log ---" >&2
  tail -40 "$WORK/host.log" >&2
  exit 1
}

RECV_SHA="$(sha256_of "$RECV_FILE")"
if [ "$RECV_SHA" != "$EXPECTED_SHA" ]; then
  echo "FAIL: SHA-256 mismatch: expected $EXPECTED_SHA, got $RECV_SHA" >&2
  exit 1
fi

echo "PASS: $BASENAME transferred ($RECV_SHA) with matching SHA-256"
