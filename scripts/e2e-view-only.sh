#!/usr/bin/env bash
set -euo pipefail

# View-only + RPC end-to-end validation (#9, #10, #19).
#
# `mesh host --share-mode <mode>` hosts a real WebRTC session with the RPC
# surface enabled. Second CLI instances then verify:
#   1. view-only entry: chunked manifest + thumbnail via the view-only cache
#      (`mesh browse`, greppable VIEWONLY markers),
#   2. sync guard: the sharer IGNORES StartSync from a view-only peer,
#   3. restore pull: one original re-materializes byte-for-byte (SHA-256),
#   4. RPC round-trips over CommandRequest/CommandResponse: list_files works,
#      toggle_favorite is REJECTED under ro and SUCCEEDS under rw.
#
# Usage:
#   scripts/e2e-view-only.sh
#
# Env:
#   SIEGU_BIN          path to the siegu CLI binary (default: target/release/siegu)
#   SIEGU_E2E_PHOTOS   directory of media to host (default: tests/fixtures/faces)
#
# Unlike e2e-sync.sh this always uses the host's in-process signaling server;
# the drivers join over plain LAN signaling exactly like `mesh join`.
#
# Used by the `mesh-e2e` jobs in .github/workflows/ubuntu.yml, macos.yml and
# windows.yml.

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
BROWSE_CFG="$WORK/browse"
RPC_CFG="$WORK/rpc"
mkdir -p "$HOST_CFG" "$BROWSE_CFG" "$RPC_CFG"
HOST_PID=""

cleanup() {
  [ -n "$HOST_PID" ] && kill "$HOST_PID" 2>/dev/null || true
  # The peers can still be holding their SQLite/WAL files open for a moment
  # after kill (especially on Windows/MSYS), which makes `rm -rf` fail with
  # "Device or resource busy" and, under `set -e`, flips the CI exit code.
  # Give them a moment to exit, then clean up best-effort.
  for _ in $(seq 1 50); do
    if [ -z "$HOST_PID" ] || ! kill -0 "$HOST_PID" 2>/dev/null; then
      break
    fi
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
echo "hosting $BASENAME (sha256=$EXPECTED_SHA)"

start_host() {
  local mode="$1"
  local log="$2"
  "$BIN" --config-dir "$HOST_CFG" mesh host --port 0 --share-mode "$mode" >"$log" 2>&1 &
  HOST_PID=$!
}

wait_for_session() {
  local log="$1"
  for _ in $(seq 1 60); do
    if grep -q "Room ID:" "$log" && grep -q "Signaling server on port" "$log"; then
      return 0
    fi
    if ! kill -0 "$HOST_PID" 2>/dev/null; then
      echo "host exited unexpectedly" >&2
      tail -40 "$log" >&2
      return 1
    fi
    sleep 0.5
  done
  echo "FAIL: host did not print a room id / port within 30s" >&2
  tail -40 "$log" >&2
  return 1
}

session_args_from_log() {
  local log="$1"
  ROOM_ID="$(grep -oE 'Room ID: [0-9a-f-]+' "$log" | head -1 | sed -E 's/Room ID: //' | tr -d '\r')"
  SIGNAL_PORT="$(grep -oE 'Signaling server on port [0-9]+' "$log" | sed -E 's/Signaling server on port //' | head -1 | tr -d '\r')"
  SERVER_URL="ws://127.0.0.1:${SIGNAL_PORT}"
}

echo "== starting read-only mesh host =="
start_host ro "$WORK/host-ro.log"
wait_for_session "$WORK/host-ro.log"
session_args_from_log "$WORK/host-ro.log"
[ -n "$ROOM_ID" ] || { echo "FAIL: no room id in host log" >&2; tail -20 "$WORK/host-ro.log" >&2; exit 1; }
echo "OK: ro room $ROOM_ID on $SERVER_URL"

echo "== running view-only browser driver =="
"$BIN" --config-dir "$BROWSE_CFG" mesh browse "$ROOM_ID" \
  --server "$SERVER_URL" >"$WORK/browse.log" 2>&1
grep -q "VIEWONLY MANIFEST OK" "$WORK/browse.log" || {
  echo "FAIL: VIEWONLY MANIFEST OK marker missing" >&2; tail -40 "$WORK/browse.log" >&2; exit 1;
}
grep -q "VIEWONLY THUMB OK" "$WORK/browse.log" || {
  echo "FAIL: VIEWONLY THUMB OK marker missing" >&2; tail -40 "$WORK/browse.log" >&2; exit 1;
}
grep -q "VIEWONLY RESTORE OK" "$WORK/browse.log" || {
  echo "FAIL: VIEWONLY RESTORE OK marker missing" >&2; tail -40 "$WORK/browse.log" >&2; exit 1;
}

echo "== verifying sharer ignored the StartSync probe =="
grep -q "ignoring StartSync from view-only peer" "$WORK/host-ro.log" || {
  echo "FAIL: sharer accepted StartSync from a view-only peer" >&2
  tail -40 "$WORK/host-ro.log" >&2
  exit 1
}
echo "OK: sync guard held"

FIRST_ID="$(grep -oE 'VIEWONLY RESTORE REQUESTED id=[^ ]+' "$WORK/browse.log" | sed -E 's/VIEWONLY RESTORE REQUESTED id=//' | head -1)"
RESTORED_FILE="$(grep -oE 'VIEWONLY RESTORE OK path=.*' "$WORK/browse.log" | sed -E 's/VIEWONLY RESTORE OK path=//' | head -1)"

echo "== verifying restored original is byte-identical =="
RECV_SHA="$(sha256_of "$RESTORED_FILE")"
if [ "$RECV_SHA" != "$EXPECTED_SHA" ]; then
  echo "FAIL: SHA-256 mismatch: expected $EXPECTED_SHA, got $RECV_SHA" >&2
  exit 1
fi
LIB_COUNT="$(find "$(dirname "$RESTORED_FILE")" -maxdepth 1 -type f | wc -l | tr -d ' ')"
if [ "$LIB_COUNT" != "1" ]; then
  echo "FAIL: browse client library should hold exactly 1 restored file, found $LIB_COUNT" >&2
  find "$(dirname "$RESTORED_FILE")" -maxdepth 1 -type f >&2
  exit 1
fi
echo "OK: restore pull matched ($RECV_SHA), library holds exactly 1 file"

restart_host() {
  # A host session goes stale once its guest leaves (the LAN room keeps the
  # old peer state), so every phase dials into a FRESH host process.
  kill "$HOST_PID" 2>/dev/null || true
  for _ in $(seq 1 50); do
    kill -0 "$HOST_PID" 2>/dev/null || break
    sleep 0.1
  done
  start_host "$1" "$2"
  wait_for_session "$2"
  session_args_from_log "$2"
}

echo "== fresh ro host for RPC checks =="
restart_host ro "$WORK/host-ro2.log"

echo "== RPC under ro: list_files must work =="
"$BIN" --config-dir "$RPC_CFG" mesh rpc "$ROOM_ID" list_files '{"query":"","offset":0,"limit":5}' \
  --server "$SERVER_URL" >"$WORK/rpc-list.log" 2>&1
grep -q "RPC RESULT ok=true" "$WORK/rpc-list.log" || {
  echo "FAIL: list_files RPC failed under ro" >&2; tail -20 "$WORK/rpc-list.log" >&2; exit 1;
}
echo "OK: list_files answered over WebRTC"

echo "== another fresh ro host for the rejection check =="
restart_host ro "$WORK/host-ro3.log"

echo "== RPC under ro: toggle_favorite must be rejected =="
if "$BIN" --config-dir "$RPC_CFG" mesh rpc "$ROOM_ID" toggle_favorite "{\"id\":\"$FIRST_ID\"}" \
     --server "$SERVER_URL" >"$WORK/rpc-fav-ro.log" 2>&1; then
  echo "FAIL: toggle_favorite succeeded although share mode is ro" >&2
  cat "$WORK/rpc-fav-ro.log" >&2
  exit 1
fi
grep -qF -- "--share-mode rw" "$WORK/rpc-fav-ro.log" || {
  echo "FAIL: rejection did not explain how to enable writes" >&2
  tail -20 "$WORK/rpc-fav-ro.log" >&2
  exit 1
}
echo "OK: write rejected with guidance (--share-mode rw)"

echo "== restarting host in read-write mode =="
restart_host rw "$WORK/host-rw.log"

echo "== RPC under rw: toggle_favorite must succeed =="
if ! "$BIN" --config-dir "$RPC_CFG" mesh rpc "$ROOM_ID" toggle_favorite "{\"id\":\"$FIRST_ID\"}" \
       --server "$SERVER_URL" >"$WORK/rpc-fav-rw.log" 2>&1; then
  echo "FAIL: toggle_favorite failed although share mode is rw" >&2
  tail -20 "$WORK/rpc-fav-rw.log" >&2
  exit 1
fi
grep -q "RPC RESULT ok=true" "$WORK/rpc-fav-rw.log" || {
  echo "FAIL: expected favorite=true in reply" >&2
  tail -20 "$WORK/rpc-fav-rw.log" >&2
  exit 1
}
echo "OK: write applied under rw"

echo "PASS: view-only browsing, sync guard, restore pull and RPC share modes all verified"
