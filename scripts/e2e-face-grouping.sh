#!/usr/bin/env bash
set -euo pipefail

# Transient end-to-end face-grouping validation.
#
# Scans the committed portrait fixtures into a throwaway config directory,
# runs the ML pipeline headlessly, and asserts that every detected face is
# grouped into a SINGLE person (the >0.5 similarity threshold from the A1
# face-grouping fix). Also asserts NSFW + aesthetics scores were produced.
#
# Used by .github/workflows/e2e.yml. Models are downloaded to a temp dir
# (or a persistent cache dir via SIEGU_MODELS_DIR); nothing touches a real
# library.

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PHOTOS="$REPO_ROOT/tests/fixtures/faces"

BIN="${SIEGU_BIN:-$REPO_ROOT/target/release/siegu}"

if [ ! -x "$BIN" ]; then
  echo "siegu binary not found at $BIN. Run: cargo build --release -p siegu-cli" >&2
  exit 1
fi

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
CFG="$WORK/config"

REQUIRED_FILES="face_detection_yunet_2023mar.onnx arcface.onnx nsfw.onnx aesthetics.onnx"

models_ok() {
  for f in $REQUIRED_FILES; do
    [ -s "$1/$f" ] || return 1
  done
}

enable_models() {
  for m in face nsfw aesthetics; do
    "$BIN" --config-dir "$CFG" config set "model_enabled_$m" true >/dev/null
  done
}

download_models() {
  enable_models
  "$BIN" --config-dir "$CFG" models download face nsfw aesthetics >/dev/null
}

if [ -n "${SIEGU_MODELS_DIR:-}" ] && models_ok "$SIEGU_MODELS_DIR"; then
  echo "Using cached models from $SIEGU_MODELS_DIR"
  mkdir -p "$CFG"
  ln -s "$SIEGU_MODELS_DIR" "$CFG/models"
  enable_models
elif [ -n "${SIEGU_MODELS_DIR:-}" ]; then
  echo "Downloading models into cache dir $SIEGU_MODELS_DIR"
  mkdir -p "$SIEGU_MODELS_DIR"
  download_models
  cp "$CFG"/models/* "$SIEGU_MODELS_DIR"/
  # Keep the freshly downloaded models available for the scan/analyze below:
  # swap the throwaway models dir for a symlink into the persistent cache.
  rm -rf "$CFG/models"
  ln -s "$SIEGU_MODELS_DIR" "$CFG/models"
else
  download_models
fi

echo "== scanning fixtures =="
"$BIN" --config-dir "$CFG" scan "$PHOTOS" 2>&1 | tail -1

echo "== analyzing =="
OUT="$("$BIN" --config-dir "$CFG" analyze all --headless 2>&1)"
echo "$OUT"

echo "== assertions =="
PEOPLE_TOTAL="$(echo "$OUT" | grep -oP 'people_total=\K[0-9]+' | head -1)"
DISTINCT_PEOPLE="$(echo "$OUT" \
  | grep -oP 'people=\[[^]]*\]' \
  | grep -oP '[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}' \
  | sort -u | wc -l)"
NSFW_CNT="$(echo "$OUT" | grep -oP 'nsfw=\K[^ ]+' | grep -v '^-$' | wc -l)"
AES_CNT="$(echo "$OUT" | grep -oP 'aesthetics=\K[^ ]+' | grep -v '^-$' | wc -l)"

if [ -z "$PEOPLE_TOTAL" ] || [ "$PEOPLE_TOTAL" != "1" ]; then
  echo "FAIL: expected exactly 1 person group, got '${PEOPLE_TOTAL:-none}'" >&2
  exit 1
fi
if [ "$DISTINCT_PEOPLE" != "1" ]; then
  echo "FAIL: faces were assigned to $DISTINCT_PEOPLE distinct person(s), expected 1" >&2
  exit 1
fi
if [ "$NSFW_CNT" -lt 1 ]; then
  echo "FAIL: expected at least one NSFW score, got none" >&2
  exit 1
fi
if [ "$AES_CNT" -lt 1 ]; then
  echo "FAIL: expected at least one aesthetics score, got none" >&2
  exit 1
fi

echo "PASS: all faces grouped into a single person; NSFW=$NSFW_CNT aesthetics=$AES_CNT"
