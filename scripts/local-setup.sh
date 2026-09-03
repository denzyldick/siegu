#!/usr/bin/env bash
# One-time local host setup for https://siegu.io  (run with sudo).
#
#   1. adds `siegu.io -> 127.0.0.1` to /etc/hosts
#   2. starts the landing container and trusts Caddy's internal CA so the
#      browser shows a green padlock on https://siegu.io
#
# Usage (from the repo root):
#   sudo bash scripts/local-setup.sh
#
# After the browser is restarted, open https://siegu.io.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

if [ "$(id -u)" -ne 0 ]; then
  echo "Run me with sudo:  sudo bash scripts/local-setup.sh" >&2
  exit 1
fi

if [ -f /etc/hosts ]; then
  if ! grep -qE '^127\.0\.0\.1[[:space:]]+siegu\.io([[:space:]]|$)' /etc/hosts; then
    echo "  + /etc/hosts: adding 127.0.0.1 siegu.io"
    printf '127.0.0.1 siegu.io\n' >> /etc/hosts
  else
    echo "  - /etc/hosts already maps siegu.io to 127.0.0.1"
  fi
fi

echo "  + building + starting landing container (ports 80/443)..."
docker compose up -d --build

CA_SRC="/data/caddy/pki/authorities/local/root.crt"
ANCHOR="/etc/ca-certificates/trust-source/anchors/siegu-caddy-root.crt"
if docker compose exec caddy sh -c "test -f $CA_SRC" >/dev/null 2>&1; then
  docker compose cp caddy:"$CA_SRC" /tmp/siegu-caddy-root.crt
  if [ -f "$ANCHOR" ] && cmp -s /tmp/siegu-caddy-root.crt "$ANCHOR"; then
    echo "  - Caddy internal CA already trusted"
  else
    echo "  + installing Caddy internal CA into the system trust store"
    cp /tmp/siegu-caddy-root.crt "$ANCHOR"
    update-ca-trust
  fi
  echo
  echo "  Done. Restart the browser, then open  https://siegu.io"
else
  echo "  ! Caddy internal CA not found yet — it is generated on first TLS request."
  echo "    Open https://siegu.io once (accept the warning), then re-run this script."
fi