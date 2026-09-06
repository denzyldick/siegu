# NAT Traversal & TURN

WebRTC usually connects devices directly (peer-to-peer), but not every network
allows that. This page explains when that happens and how Siegu keeps syncing
and sharing working anyway — including over **mobile data**.

## When Direct Connection Fails

Two devices that are on the same Wi-Fi (LAN) find each other with no setup.
Over the internet, a device normally first asks a STUN server what its public
IP looks like and tries to punch a hole through its router ("hole punching").
That works for most home networks, but fails on:

- **Symmetric NAT** — some routers assign a *different* public port per
  destination, which hole punching can't predict.
- **Carrier-grade NAT (CGNAT)** — many mobile data networks put every phone
  behind a shared public IP, so the phone has no public address at all.

When no direct path can be established, WebRTC needs a **TURN server**: a
relay both devices can reach that forwards the encrypted media between them.

```
Device A ──┐
           ├── TURN relay (forwards, never stores)
Device B ──┘
```

TURN is used only as a fallback. ICE always prefers the direct STUN path; the
relay only engages when a peer has no other way.

## How Siegu Uses TURN

Siegu ships with its own **built-in TURN relay** — no separate server to
install. When enabled, the host starts a small TURN server on the machine
running Siegu the moment it launches, generates credentials, and hands them to
every peer (desktop guests and the web sharing page) so both sides can offer
the relay as a candidate. The browser default stays a public STUN server —
nothing special is needed for LAN or most internet cases.

The host reads its ICE configuration from three environment variables when it
creates WebRTC connections (`mesh_transport.rs`). With the built-in relay these
are **set automatically** by the app:

| Variable | Description |
|----------|-------------|
| `SIEGU_TURN_URLS` | Comma-separated TURN URLs, e.g. `turn:home.example.com:3478` |
| `SIEGU_TURN_USERNAME` | TURN username (required when the relay has auth) |
| `SIEGU_TURN_CREDENTIAL` | TURN password/credential |

You can still override these variables yourself to point at a relay you manage
separately — set `SIEGU_TURN_URLS` before starting Siegu and the built-in relay
skips its own start.

## Option A — Built-in relay (free, runs on the host)

The relay runs inside Siegu on the same machine that hosts your library. No
server rental, no credit card — just the app you already run.

### Enable it

Desktop app: **Settings → Pro → Advanced → Built-in relay**, toggle *Run a
relay on this device* and Save. Or set the config keys directly (see
[Configuration](configuration.md)):

| Key | Meaning |
|-----|---------|
| `turn_enabled` | `true` / `false` — start the built-in relay at launch |
| `turn_port` | Relay UDP port; `0` (default) picks a free port automatically |
| `turn_public_host` | Public IP of this device; empty = auto-detected LAN IP (browser guests still need your public address — see below) |

The app generates a username/password pair the first time the relay is enabled
and reuses it, so guests get stable credentials. Changes to the relay settings
apply the next time the app launches.

### Open the port

The relay listens on a UDP port on the host. For guests **on the same
network**, nothing else is needed. For guests **on the internet**, the router
must forward that UDP port to the host's LAN IP:

1. Note the relay port (auto-picked unless you set `turn_port`).
2. In your router, forward UDP `<port>` to the host's LAN IP.
3. Keep the host on a static/reserved DHCP address so the forward stays valid.

The port forward is the one manual step; everything else is in the app.

### Test

From a device **not on your home Wi-Fi** (cellular data is ideal), open a
share link or sync from your phone. Use a small test first — a full library
relay consumes your **home upload bandwidth**:

- A photo is relayed through your router's WAN, so `N` GB of media costs `N`
  GB of your monthly upload quota.
- Home upload speeds (typically 10–30 Mbps) cap the throughput: fine for
  photos, slow for large videos.

### Caveats

- **If your home ISP uses CGNAT**, router forwards are unreachable from the
  internet and a built-in relay at home cannot work. Test from a separate
  network before relying on it.
- The relay **forwards but never stores** traffic; media is still end-to-end
  encrypted in transit.
- Guests always get credentials alongside the relay URL (long-lived static
  auth). The relay only listens on the host, so it can't be discovered and
  abused for bandwidth the way an open public relay can.

## Option B — Hosted Relay (Pro)

Siegu Pro includes a hosted relay (`wss://siegu.io/ws` signalling **plus** a
TURN relay) so your devices can connect from **any** network — cellular,
hotel Wi-Fi, countries where the network blocks direct peer connections —
with **zero router configuration**. This is the paid tier's "it just works"
story: you never think about NAT again.

## Summary

| Setup | Cost | Works on | Router config | Best for |
|-------|------|----------|---------------|----------|
| LAN only | Free | Same network | No | Default |
| Built-in relay | Free | Internet (if home NAT allows) | UDP forward once | DIY, mobile data at home |
| Hosted relay (Pro) | Pro plan | Anything, incl. CGNAT | None | Everyone else |

## Related

- [Signalling Server](SIGNALLING.md) — the WebSocket coordination layer
- [Configuration](configuration.md) — app config keys
- [Mesh Protocol](mesh-protocol.md) — the sync data-channel protocol
- [Collection Sharing](sharing.md) — how sharing works