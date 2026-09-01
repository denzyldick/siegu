/**
 * WebRTC guest transport for the RPC web backend (#19, Phase 1).
 *
 * Mirrors the proven connectivity in `webclient/main.ts`: the CLI host
 * (`siegu web`) is the initiator, creates the room and sends the WebRTC
 * offer; this class answers it over the signalling relay and opens the data
 * channel. Messages are JSON-framed both ways.
 *
 * The rest of the app only depends on {@link PeerTransport}, so the RPC logic
 * in `guest.ts` can be unit-tested with a fake transport.
 */
import type { GuestInbound, GuestOutbound, GuestSession } from './protocol';

/** The abstract channel a guest uses to exchange JSON messages with the host. */
export interface PeerTransport {
  /** Send a JSON frame. No-op until open. */
  send(msg: GuestOutbound): void;
  /** Register the handler for inbound JSON frames. */
  onMessage(handler: (msg: GuestInbound) => void): void;
  /** Register lifecycle callbacks. */
  onOpen(handler: () => void): void;
  onClose(handler: () => void): void;
  isOpen(): boolean;
  close(): void;
}

type SignalMsg = Record<string, unknown> & { type: string };

function wsUrl(base: string): string {
  const proto = base.startsWith('https:') ? 'wss' : 'ws';
  return `${proto}://${base}`;
}

/**
 * Build a real {@link PeerTransport} over a signalling WebSocket + WebRTC
 * peer connection. `wsBase` is the `host[:port]` of the CLI static server
 * (the `/ws` bridge). `session` carries the room code + token from `#code.token`.
 */
export function createPeerTransport(
  wsBase: string,
  session: GuestSession,
  onUpsertPassword?: () => void,
): PeerTransport {
  const socket = new WebSocket(wsUrl(wsBase) + '/ws');

  const listeners = {
    message: [] as Array<(msg: GuestInbound) => void>,
    open: [] as Array<() => void>,
    close: [] as Array<() => void>,
  };

  let pc: RTCPeerConnection | null = null;
  let dc: RTCDataChannel | null = null;
  let closed = false;
  const decoder = new TextDecoder();

  const transport: PeerTransport = {
    send(msg) {
      if (!dc || dc.readyState !== 'open') return;
      // Respect SCTP backpressure: drop rather than queue unboundedly.
      if (dc.bufferedAmount > 1_000_000) return;
      dc.send(JSON.stringify(msg));
    },
    onMessage(handler) {
      listeners.message.push(handler);
    },
    onOpen(handler) {
      listeners.open.push(handler);
    },
    onClose(handler) {
      listeners.close.push(handler);
    },
    isOpen() {
      return !!dc && dc.readyState === 'open';
    },
    close() {
      closed = true;
      try {
        dc?.close();
      } catch {
        /* ignore */
      }
      try {
        pc?.close();
      } catch {
        /* ignore */
      }
      try {
        socket.close();
      } catch {
        /* ignore */
      }
    },
  };

  function emitOpen(): void {
    for (const h of listeners.open) h();
  }
  function emitClose(): void {
    for (const h of listeners.close) h();
  }
  function emitMessage(msg: GuestInbound): void {
    for (const h of listeners.message) h(msg);
  }

  function relay(payload: unknown): void {
    if (socket.readyState !== WebSocket.OPEN) return;
    socket.send(JSON.stringify({ type: 'relay', payload }));
  }

  socket.addEventListener('open', () => {
    socket.send(JSON.stringify({ type: 'join_room', code: session.code, token: session.token }));
  });

  socket.addEventListener('message', (event) => {
    let sig: SignalMsg;
    try {
      sig = JSON.parse(event.data as string);
    } catch {
      return;
    }
    switch (sig.type) {
      case 'relay': {
        const payload = sig.payload as SignalMsg | undefined;
        if (!payload?.type) return;
        if (payload.type === 'offer') {
          void answerOffer(String(payload.payload));
        } else if (payload.type === 'ice_candidate') {
          try {
            const init = JSON.parse(String(payload.payload));
            if (pc) void pc.addIceCandidate(init).catch(() => {});
          } catch {
            /* ignore malformed candidates */
          }
        }
        break;
      }
      case 'peer_disconnected':
      case 'room_closed':
        if (closed) break;
        emitClose();
        break;
      default:
        break;
    }
  });

  socket.addEventListener('close', () => {
    if (!closed) emitClose();
  });

  async function answerOffer(sdpJson: string): Promise<void> {
    try {
      pc = new RTCPeerConnection({ iceServers: [{ urls: 'stun:stun.l.google.com:19302' }] });

      pc.onicecandidate = (ev) => {
        if (!ev.candidate) return;
        relay({
          type: 'ice_candidate',
          payload: JSON.stringify(ev.candidate.toJSON()),
          target: 'peer',
        });
      };

      pc.ondatachannel = (ev) => {
        dc = ev.channel;
        dc.binaryType = 'arraybuffer';
        dc.onopen = () => {
          emitOpen();
          // Scope the guest to a single shared collection when the link carries
          // an album id; otherwise drop into view-only whole-library mode.
          if (session.albumId) {
            transport.send({ type: 'EnterAlbumShare', album_id: session.albumId });
          } else {
            transport.send({ type: 'EnterViewOnly' });
          }
        };
        dc.onmessage = (me) => {
          const text =
            typeof me.data === 'string' ? me.data : decoder.decode(me.data as ArrayBuffer);
          try {
            emitMessage(JSON.parse(text) as GuestInbound);
          } catch {
            /* ignore malformed frames */
          }
        };
        dc.onclose = () => emitClose();
      };

      await pc.setRemoteDescription(JSON.parse(sdpJson));
      const answer = await pc.createAnswer();
      await pc.setLocalDescription(answer);
      relay({
        type: 'answer',
        payload: JSON.stringify({ type: 'answer', sdp: answer.sdp }),
        target: 'peer',
      });
    } catch {
      emitClose();
    }
  }

  onUpsertPassword?.();
  return transport;
}
