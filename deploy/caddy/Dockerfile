###############################################################################
# Caddy edge for the Siegu web client.
# Serves as the HTTP/HTTPS front door in front of the `siegu web` host:
#   - plain HTTP on :80 when SIEGU_DOMAIN is empty (localhost / LAN),
#   - automatic HTTPS when SIEGU_DOMAIN is set (e.g. app.example.com).
###############################################################################
FROM caddy:2.8-alpine
COPY Caddyfile.template /etc/caddy/Caddyfile.template
COPY entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh
ENTRYPOINT ["/entrypoint.sh"]
