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
import { resolveSignalingBase } from '@/services/signalling';
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
 * constructing a real WebSocket/RTCPeerConnection. `signalingBase` is the
 * `host[:port]` of the signaler the guest connects to: pass a remote one (e.g.
 * `siegu.io`) to pair via a hosted `wss://` relay (Phase 4); when omitted in a
 * browser the default resolves to the serving origin's `/ws` bridge.
 */
export function bootGuest(
  session: GuestSession,
  events: GuestBootEvents = {},
  transportOverride?: PeerTransport,
  signalingBase?: string,
): GuestBoot {
  const base = signalingBase ?? resolveSignalingBase();
  const transport = transportOverride ?? createPeerTransport(base, session);

  const client = new GuestClient(transport, {
    onOpen: () => events.onOpen?.(),
    onClose: () => events.onClose?.(),
    onError: (m) => events.onError?.(m),
    onMedia: (id, key, url) => events.onMedia?.(id, key, url),
  });

  return { client, transport };
}
