/**
 * siegu view-only web client (#11).
 *
 * Speaks the same SignalMessage (WebSocket) + SyncMessage (WebRTC data
 * channel) protocols as native peers. The CLI side (`siegu web`) is the
 * initiator: it creates the room, sends the WebRTC offer and serves the
 * read-only manifest plus media. This page answers the offer, renders the
 * gallery and pulls media on demand. Nothing is persisted anywhere.
 */

interface ViewPhoto {
  id: string;
  location: string;
  created: string;
  caption: string | null;
}

type SignalMsg = Record<string, unknown> & { type: string };

type SyncMsg =
  | { type: 'ViewOnlyManifest'; photos: ViewPhoto[]; more: boolean }
  | { type: 'ViewMedia'; id: string; mime: string; data: string }
  | {
      type: 'FileHeader';
      id: string;
      filename: string;
      size: number;
      checksum: string;
    }
  | { type: 'FileChunk'; id: string; index: number; data: number[] }
  | { type: 'FileEnd'; id: string; checksum: string }
  | { type: string; [k: string]: unknown };

const statusEl = document.getElementById('status') as HTMLElement;
const gateEl = document.getElementById('gate') as HTMLElement;
const gateMsg = document.getElementById('gate-msg') as HTMLElement;
const galleryEl = document.getElementById('gallery') as HTMLElement;
const previewEl = document.getElementById('preview') as HTMLDialogElement;
const previewTitle = document.getElementById('preview-title') as HTMLElement;
const previewBody = document.getElementById('preview-body') as HTMLElement;

function setStatus(text: string): void {
  statusEl.textContent = text;
}

/** Parse "#CODE.TOKEN" from the URL fragment. */
function parseHash(): { code: string; token: string } | null {
  const raw = decodeURIComponent(window.location.hash.replace(/^#/, ''));
  const [code, token] = raw.split('.');
  if (!code || !token || code.includes('/') || token.includes('/')) return null;
  return { code, token };
}

// ---------------------------------------------------------------------------
// Media cache
// ---------------------------------------------------------------------------

const objectUrls = new Map<string, string>();
const inflightThumbs = new Set<string>();
// Only one original streams at a time (single preview dialog), but it must be
// keyed by photo id: reusing a shared cache slot would serve the previous
// photo's bytes when opening a different item.
let pendingOriginal: {
  id: string;
  filename: string;
  chunks: Map<number, number[]>;
} | null = null;

function cachedUrl(key: string): string | undefined {
  return objectUrls.get(key);
}

function storeBlobUrl(key: string, bytes: Uint8Array, mime: string): string {
  const existing = objectUrls.get(key);
  if (existing) return existing;
  const copy = new Uint8Array(bytes);
  const url = URL.createObjectURL(new Blob([copy], { type: mime }));
  objectUrls.set(key, url);
  return url;
}

// ---------------------------------------------------------------------------
// Data channel
// ---------------------------------------------------------------------------

let dc: RTCDataChannel | null = null;
let manifestPhotos: ViewPhoto[] = [];

function sendSync(msg: SyncMsg): void {
  if (!dc || dc.readyState !== 'open') return;
  // Respect SCTP backpressure: drop rather than queue unboundedly.
  if (dc.bufferedAmount > 1_000_000) return;
  dc.send(JSON.stringify(msg));
}

function requestThumb(id: string): void {
  if (cachedUrl(`thumb:${id}`) || inflightThumbs.has(id)) return;
  inflightThumbs.add(id);
  sendSync({ type: 'FetchMediaRequest', id, thumbnail: true });
}

function requestOriginal(id: string): void {
  pendingOriginal = { id, filename: '', chunks: new Map() };
  sendSync({ type: 'FetchMediaRequest', id, thumbnail: false });
}

function handleSync(msg: SyncMsg): void {
  switch (msg.type) {
    case 'ViewOnlyManifest': {
      const m = msg as Extract<SyncMsg, { type: 'ViewOnlyManifest' }>;
      manifestPhotos = manifestPhotos.concat(m.photos);
      setStatus(
        m.more
          ? `Loaded ${manifestPhotos.length} items…`
          : `Loaded ${manifestPhotos.length} photos`,
      );
      renderGallery();
      break;
    }
    case 'ViewMedia': {
      const m = msg as Extract<SyncMsg, { type: 'ViewMedia' }>;
      const raw = atob(m.data);
      const bytes = new Uint8Array(raw.length);
      for (let i = 0; i < raw.length; i++) bytes[i] = raw.charCodeAt(i);
      const url = storeBlobUrl(`thumb:${m.id}`, bytes, m.mime);
      inflightThumbs.delete(m.id);
      applyThumb(m.id, url);
      break;
    }
    case 'FileHeader': {
      const m = msg as Extract<SyncMsg, { type: 'FileHeader' }>;
      if (pendingOriginal && pendingOriginal.id === m.id) {
        pendingOriginal.filename = m.filename;
      }
      break;
    }
    case 'FileChunk': {
      const m = msg as Extract<SyncMsg, { type: 'FileChunk' }>;
      if (pendingOriginal && pendingOriginal.id === m.id) {
        pendingOriginal.chunks.set(m.index, m.data);
      }
      break;
    }
    case 'FileEnd': {
      const m = msg as { type: 'FileEnd'; id: string };
      if (pendingOriginal && pendingOriginal.id === m.id) {
        finishOriginal(m.id);
      }
      break;
    }
    default:
      break;
  }
}

function finishOriginal(id: string): void {
  const orig = pendingOriginal;
  pendingOriginal = null;
  if (!orig || orig.id !== id) return;
  const indexes = [...orig.chunks.keys()].sort((a, b) => a - b);
  let len = 0;
  for (const i of indexes) len += orig.chunks.get(i)!.length;
  const bytes = new Uint8Array(len);
  let offset = 0;
  for (const i of indexes) {
    bytes.set(orig.chunks.get(i)!, offset);
    offset += orig.chunks.get(i)!.length;
  }
  const name = orig.filename.toLowerCase();
  const mime = /\.(mp4|mov|m4v)$/.test(name)
    ? 'video/mp4'
    : /\.webm$/.test(name)
      ? 'video/webm'
      : 'image/jpeg';
  const url = storeBlobUrl(`original:${id}`, bytes, mime);
  showPreview(url, mime.startsWith('video/'));
}

// ---------------------------------------------------------------------------
// Gallery
// ---------------------------------------------------------------------------

// One observer for the lifetime of the page: tiles ask it for thumbnails when
// they approach the viewport. Recreating it per render leaked every instance.
const galleryObserver = new IntersectionObserver(
  (entries) => {
    for (const entry of entries) {
      if (!entry.isIntersecting) continue;
      const tile = entry.target as HTMLElement;
      galleryObserver.unobserve(tile);
      const id = tile.dataset.id!;
      const cached = cachedUrl(`thumb:${id}`);
      if (cached) applyThumb(id, cached);
      else requestThumb(id);
    }
  },
  { rootMargin: '300px' },
);

function buildTile(photo: ViewPhoto): HTMLElement {
  const tile = document.createElement('button');
  tile.type = 'button';
  tile.className = 'tile';
  tile.dataset.id = photo.id;
  tile.title = photo.caption ?? '';
  tile.setAttribute('aria-label', photo.caption ?? photo.location ?? photo.id);
  tile.addEventListener('click', () => openFull(photo));

  const img = document.createElement('img');
  img.alt = photo.caption ?? '';
  img.loading = 'lazy';
  img.addEventListener('load', () => img.classList.add('loaded'));
  tile.appendChild(img);

  if (/\.(mp4|mov|avi|mkv|webm)$/i.test(photo.location)) {
    const badge = document.createElement('span');
    badge.className = 'badge';
    badge.textContent = '▶';
    tile.appendChild(badge);
  }

  galleryObserver.observe(tile);
  return tile;
}

// Large libraries stream in as many manifest chunks; building the whole DOM
// on every chunk (with a per-photo querySelector) crashed the tab around 9k
// items. Grow instead: append one batch per call and let a sentinel element
// pull in the next batch when the user scrolls near the end.
const GALLERY_BATCH = 500;
let renderedTiles = 0;
const gallerySentinel = document.createElement('div');
gallerySentinel.className = 'sentinel';

function renderGallery(): void {
  if (renderedTiles >= manifestPhotos.length) return;

  const end = Math.min(renderedTiles + GALLERY_BATCH, manifestPhotos.length);
  const frag = document.createDocumentFragment();
  for (let i = renderedTiles; i < end; i++) {
    frag.appendChild(buildTile(manifestPhotos[i]));
  }
  renderedTiles = end;

  if (renderedTiles < manifestPhotos.length) {
    galleryEl.appendChild(gallerySentinel);
    galleryEl.appendChild(frag);
  } else {
    galleryEl.appendChild(frag);
    gallerySentinel.remove();
  }
}

const sentinelObserver = new IntersectionObserver((entries) => {
  for (const entry of entries) {
    if (entry.isIntersecting) renderGallery();
  }
});
sentinelObserver.observe(gallerySentinel);

function applyThumb(id: string, url: string): void {
  const img = galleryEl.querySelector<HTMLImageElement>(`[data-id="${CSS.escape(id)}"] img`);
  if (img) {
    img.src = url;
    img.classList.add('loaded');
  }
}

function openFull(photo: ViewPhoto): void {
  previewTitle.textContent = photo.caption ?? photo.location;
  previewBody.replaceChildren();
  previewEl.showModal();
  const video = /\.(mp4|webm|mov|m4v)$/i.test(photo.location);
  const cached = cachedUrl(`original:${photo.id}`);
  if (cached) {
    showPreview(cached, video);
    return;
  }
  const placeholder = document.createElement('p');
  placeholder.textContent = 'Streaming from device…';
  placeholder.style.color = 'var(--muted)';
  previewBody.appendChild(placeholder);
  requestOriginal(photo.id);
}

function showPreview(url: string, video: boolean): void {
  previewBody.replaceChildren();
  let el: HTMLImageElement | HTMLVideoElement;
  if (video) {
    el = document.createElement('video');
    (el as HTMLVideoElement).controls = true;
    (el as HTMLVideoElement).playsInline = true;
  } else {
    el = document.createElement('img');
  }
  el.src = url;
  previewBody.appendChild(el);
}

document.getElementById('preview-close')?.addEventListener('click', () => {
  previewEl.close();
  previewBody.replaceChildren();
});

// ---------------------------------------------------------------------------
// Signalling + WebRTC
// ---------------------------------------------------------------------------

function wsUrl(): string {
  const proto = window.location.protocol === 'https:' ? 'wss' : 'ws';
  return `${proto}://${window.location.host}/ws`;
}

async function start(): Promise<void> {
  const session = parseHash();
  if (!session) {
    setStatus('Missing session link');
    gateMsg.textContent =
      'This page needs a session link. Run `siegu web` on the device that holds your library and open the URL it prints.';
    return;
  }

  setStatus('Connecting…');
  const ws = new WebSocket(wsUrl());

  ws.addEventListener('open', () => {
    ws.send(
      JSON.stringify({
        type: 'join_room',
        code: session.code,
        token: session.token,
      }),
    );
    setStatus('Joining session…');
  });

  ws.addEventListener('close', () => {
    setStatus('Session ended');
  });

  ws.addEventListener('error', () => {
    setStatus('Connection failed — check the link and try again');
  });

  ws.addEventListener('message', (event) => {
    let sig: SignalMsg;
    try {
      sig = JSON.parse(event.data as string);
    } catch {
      return;
    }

    switch (sig.type) {
      case 'room_joined':
        setStatus('Waiting for the device to share…');
        break;
      case 'error':
        setStatus(String(sig.message ?? 'Session error'));
        break;
      case 'relay':
        handleRelay(ws, sig);
        break;
      case 'peer_disconnected':
      case 'room_closed':
        setStatus('Peer disconnected — session over');
        if (dc) dc.close();
        break;
      default:
        break;
    }
  });
}

function handleRelay(ws: WebSocket, sig: SignalMsg): void {
  const payload = sig.payload as SignalMsg | undefined;
  if (!payload?.type) return;

  switch (payload.type) {
    case 'offer':
      answerOffer(ws, String(payload.payload));
      break;
    case 'ice_candidate':
      try {
        const init = JSON.parse(String(payload.payload));
        if (pc) pc.addIceCandidate(init).catch(() => {});
      } catch {
        /* ignore malformed candidates */
      }
      break;
    default:
      break;
  }
}

let pc: RTCPeerConnection | null = null;

async function answerOffer(ws: WebSocket, sdpJson: string): Promise<void> {
  try {
    setStatus('Connecting media channel…');
    pc = new RTCPeerConnection({
      iceServers: [{ urls: 'stun:stun.l.google.com:19302' }],
    });

    pc.onicecandidate = (ev) => {
      if (!ev.candidate) return;
      ws.send(
        JSON.stringify({
          type: 'relay',
          payload: {
            type: 'ice_candidate',
            payload: JSON.stringify(ev.candidate.toJSON()),
            target: 'peer',
          },
        }),
      );
    };

    pc.ondatachannel = (ev) => {
      dc = ev.channel;
      dc.binaryType = 'arraybuffer';
      console.log('[siegu-dc] channel', dc.label, dc.readyState);
      dc.onopen = () => {
        console.log('[siegu-dc] open');
        setStatus('Connected — loading library…');
        gateEl.hidden = true;
        galleryEl.hidden = false;
        manifestPhotos = [];
        sendSync({ type: 'EnterViewOnly' });
      };
      // webrtc-rs tags its outgoing frames as binary, so Chrome hands them
      // to us as ArrayBuffers even though the payload is UTF-8 JSON.
      const decoder = new TextDecoder();
      dc.onmessage = (me) => {
        const text = typeof me.data === 'string' ? me.data : decoder.decode(me.data as ArrayBuffer);
        try {
          handleSync(JSON.parse(text));
        } catch (e) {
          console.error('[siegu-dc] handle failed', e, text.slice(0, 120));
        }
      };
      dc.onclose = () => setStatus('Media channel closed');
    };

    await pc.setRemoteDescription(JSON.parse(sdpJson));
    const answer = await pc.createAnswer();
    await pc.setLocalDescription(answer);
    ws.send(
      JSON.stringify({
        type: 'relay',
        payload: {
          type: 'answer',
          payload: JSON.stringify({ type: 'answer', sdp: answer.sdp }),
          target: 'peer',
        },
      }),
    );
  } catch (e) {
    setStatus(`Could not connect: ${String(e)}`);
  }
}

void start();
