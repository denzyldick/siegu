#!/bin/sh
set -e

# Build the site address: HTTPS w/ auto-TLS when a domain is set, else plain
# HTTP on :80 (works on localhost / LAN without any TLS setup).
if [ -n "$SIEGU_DOMAIN" ]; then
  SITE_ADDR="https://${SIEGU_DOMAIN}"
else
  SITE_ADDR=":80"
fi

echo "Using site address: ${SITE_ADDR}"
sed "s|{SITE_ADDR}|${SITE_ADDR}|g" /etc/caddy/Caddyfile.template > /etc/caddy/Caddyfile

exec caddy run --config /etc/caddy/Caddyfile --adapter caddyfile "$@"
