/**
 * Guest Mode B boot wiring (#19/#25, Phase 2).
 *
 * Builds a connected {@link GuestClient} over a real {@link PeerTransport} for
 * the `#code.token` session detected at runtime. The served SPA is typically
 * delivered by the Siegu host itself, so the transport connects to the host's
 * own `/ws` signalling bridge on the current origin (the CLI host is the WebRTC
 * initiator; this side answers the offer).
 *
 * Business value: the browser PAIRS by code + token and streams media over
 * WebRTC — the web.whatsapp.com model (Mode B). Everything here is injectable so
 * it is unit-testable with a fake transport; `createPeerTransport` is only used
 * when no transport is supplied.
 */
import { GuestClient } from './guest';
import { createPeerTransport } from './peer';
import type { PeerTransport } from './peer';
import type { GuestSession } from './protocol';

export interface GuestBootEvents {
  onOpen?: () => void;
  onClose?: () => void;
  onError?: (message: string) => void;
  onMedia?: (id: number | string, key: string, url: string) => void;
}

export interface GuestBoot {
  client: GuestClient;
  transport: PeerTransport;
}

/**
 * Build a connected guest session. Pass `transportOverride` in tests to avoid
 * constructing a real WebSocket/RTCPeerConnection.
 */
export function bootGuest(
  session: GuestSession,
  events: GuestBootEvents = {},
  transportOverride?: PeerTransport,
): GuestBoot {
  const transport = transportOverride ?? createPeerTransport(window.location.host, session);

  const client = new GuestClient(transport, {
    onOpen: () => events.onOpen?.(),
    onClose: () => events.onClose?.(),
    onError: (m) => events.onError?.(m),
    onMedia: (id, key, url) => events.onMedia?.(id, key, url),
  });

  return { client, transport };
}
