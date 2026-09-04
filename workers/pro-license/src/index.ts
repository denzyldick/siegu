/**
 * Siegu Pro license fulfillment worker (Cloudflare Workers).
 *
 * Merchant-of-record + email-ownership verification for the paid (Pro) tier.
 *
 * Flow
 * ----
 *   1. Stripe `checkout.session.completed` -> `POST /stripe-webhook` -> records
 *      `paid:<email>` in KV (idempotent) and emails the buyer a download link +
 *      setup instructions (Resend). Stripe must capture the buyer's email.
 *
 *   2. In the desktop app (Settings > Pro) the user enters the email they
 *      bought with and taps "Send verification email". The app calls
 *      `POST /send-verify?email=...`. The worker only proceeds if that email is
 *      already in `paid:` KV (no spam to arbitrary addresses).
 *
 *   3. The worker creates a short-lived signed token (HMAC + expiry, no OTP)
 *      and emails a link via Resend: `<origin>/confirm?token=...`
 *
 *   4. The user clicks the link -> `GET /confirm?token=...` -> validates the
 *      signature & expiry -> records `verified:<email>` -> redirects to a
 *      success page. The token is single-use.
 *
 *   5. The app calls `GET /verify?email=...` and only unlocks Pro when the
 *      email is BOTH `paid` and `verified`.
 *
 * Storage: Cloudflare KV namespace (`PAID_EMAILS`).
 *   - `paid:<email>`      -> payment record (JSON)
 *   - `verified:<email>`  -> timestamp when ownership was confirmed
 *   - `token:<sha256>`    -> single-use confirmation tokens (keyed by digest)
 *
 * Secrets (all set via `wrangler secret put`):
 *   - STRIPE_WEBHOOK_SECRET   : Stripe webhook signing secret (whsec_...)
 *   - SIEGU_VERIFY_TOKEN      : shared secret the desktop app sends (app auth)
 *   - CONFIRM_SIGNING_SECRET  : HMAC key for confirm links (NEVER in the app)
 *   - RESEND_API_KEY          : Resend API key (for sending email)
 *   - MAIL_FROM               : verified sender, e.g. "Siegu <pro@example.com>"
 *
 * Deploy:
 *   cd workers/pro-license
 *   npm install
 *   npx wrangler kv namespace create PAID_EMAILS        # paste id/preview_id below
 *   npx wrangler secret put STRIPE_WEBHOOK_SECRET
 *   npx wrangler secret put SIEGU_VERIFY_TOKEN
 *   npx wrangler secret put CONFIRM_SIGNING_SECRET
 *   npx wrangler secret put RESEND_API_KEY
 *   npx wrangler secret put MAIL_FROM
 *   npx wrangler deploy
 */
export interface Env {
  PAID_EMAILS: KVNamespace;
  STRIPE_WEBHOOK_SECRET: string;
  SIEGU_VERIFY_TOKEN: string;
  CONFIRM_SIGNING_SECRET: string;
  RESEND_API_KEY: string;
  MAIL_FROM: string;
}

/** Landing page shown after a successful verification. */
const SUCCESS_URL = 'https://denzyldick.github.io/siegu/#pricing';

/** Direct download for the Linux (AppImage) build, sent to buyers. */
const DOWNLOAD_URL =
  'https://github.com/denzyldick/siegu/releases/download/v0.1.14/Siegu_0.1.14_amd64.AppImage';

/** README section listing install from source / all supported platforms. */
const RUN_OTHER_WAYS_URL = 'https://github.com/denzyldick/siegu#how-do-i-install-it';

/** Setup / troubleshooting guide (landing page). */
const SETUP_URL = 'https://denzyldick.github.io/siegu/';

const JSON_HEADERS = {
  'content-type': 'application/json; charset=utf-8',
};

function json(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), { status, headers: JSON_HEADERS });
}

/** Normalise an email for use as a KV key/partition: lowercase, trimmed. */
function normalizeEmail(raw: string): string {
  return raw.trim().toLowerCase();
}

/** Simple percent-encoding for a query param value. */
function encodeParam(input: string): string {
  return input.replace(/[^A-Za-z0-9._~-]/g, (c) => {
    return '%' + c.charCodeAt(0).toString(16).toUpperCase();
  });
}

// ---- Signed verification tokens (HMAC-SHA256, no random code) ----
// Token payload: `<email>|<expiresEpochSeconds>`. Signature is the HMAC of the
// payload. format: base64url(payload).base64url(sig). Expiry ~30 min.
//
// SECURITY: the HMAC key is a DEDICATED secret (CONFIRM_SIGNING_SECRET), NOT
// the app's SIEGU_VERIFY_TOKEN. The app token is embedded in the shipped
// desktop app, so it must never double as the key used to forge confirm links.
// Keep these two secrets distinct.

async function hmacHex(env: Env, data: string): Promise<string> {
  const key = await crypto.subtle.importKey(
    'raw',
    new TextEncoder().encode(env.CONFIRM_SIGNING_SECRET),
    { name: 'HMAC', hash: 'SHA-256' },
    false,
    ['sign'],
  );
  const mac = await crypto.subtle.sign('HMAC', key, new TextEncoder().encode(data));
  return [...new Uint8Array(mac)].map((b) => b.toString(16).padStart(2, '0')).join('');
}

function b64url(data: string): string {
  return btoa(data).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
}
function unb64url(data: string): string {
  const pad = data.length % 4 === 0 ? '' : '='.repeat(4 - (data.length % 4));
  return atob(data.replace(/-/g, '+').replace(/_/g, '/') + pad);
}

async function issueToken(env: Env, email: string): Promise<string> {
  const expires = Math.floor(Date.now() / 1000) + 30 * 60;
  const payload = `${email}|${expires}`;
  const sig = await hmacHex(env, payload);
  return `${b64url(payload)}.${b64url(sig)}`;
}

interface TokenPayload {
  email: string;
  expires: number;
}

async function parseToken(env: Env, token: string): Promise<TokenPayload | null> {
  const [encPayload, encSig] = String(token || '').split('.');
  if (!encPayload || !encSig) return null;
  let payload: string;
  try {
    payload = unb64url(encPayload);
  } catch {
    return null;
  }
  const expected = await hmacHex(env, payload);
  try {
    const given = unb64url(encSig);
    const expBytes = new TextEncoder().encode(expected);
    const givenBytes = new TextEncoder().encode(given);
    if (expBytes.length !== givenBytes.length) return null;
    let ok = 0;
    for (let i = 0; i < expBytes.length; i++) ok |= expBytes[i] ^ givenBytes[i];
    if (ok !== 0) return null;
  } catch {
    return null;
  }
  const sep = payload.lastIndexOf('|');
  const email = normalizeEmail(payload.slice(0, sep));
  const expires = Number(payload.slice(sep + 1));
  if (!email || !Number.isFinite(expires)) return null;
  return { email, expires };
}

// ---- Stripe webhook signature ----
async function verifyStripeSignature(
  env: Env,
  rawBody: ArrayBuffer,
  header: string | null,
): Promise<boolean> {
  if (!header || !env.STRIPE_WEBHOOK_SECRET) return false;
  const parts = header.split(',').map((p) => p.trim());
  const ts = parts.find((p) => p.startsWith('t='))?.slice(2);
  const sig = parts.find((p) => p.startsWith('v1='))?.slice(3);
  if (!ts || !sig) return false;

  const timestamp = Number(ts);
  if (!Number.isFinite(timestamp)) return false;
  const diff = Math.abs(Date.now() / 1000 - timestamp);
  if (diff > 5 * 60) return false;

  const signedPayload = `${ts}.${new TextDecoder().decode(rawBody)}`;
  const key = await crypto.subtle.importKey(
    'raw',
    new TextEncoder().encode(env.STRIPE_WEBHOOK_SECRET),
    { name: 'HMAC', hash: 'SHA-256' },
    false,
    ['sign'],
  );
  const mac = await crypto.subtle.sign('HMAC', key, new TextEncoder().encode(signedPayload));
  const hex = [...new Uint8Array(mac)].map((b) => b.toString(16).padStart(2, '0')).join('');
  if (hex.length !== sig.length) return false;
  let ok = 0;
  for (let i = 0; i < hex.length; i++) ok |= hex.charCodeAt(i) ^ sig.charCodeAt(i);
  return ok === 0;
}

// ---- Resend email ----
async function sendVerifyEmail(env: Env, to: string, link: string): Promise<void> {
  const resp = await fetch('https://api.resend.com/emails', {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${env.RESEND_API_KEY}`,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      from: env.MAIL_FROM || 'Siegu <pro@example.com>',
      to,
      subject: 'Confirm your Siegu Pro email',
      html: `
        <div style="font-family:Outfit,Arial,sans-serif;max-width:480px;margin:0 auto;color:#18181b">
          <h2 style="margin-bottom:8px">Almost there — confirm your email</h2>
          <p style="color:#52525b">Tap the button below to verify this email and unlock Siegu Pro. The link expires in 30 minutes.</p>
          <p style="text-align:center;margin:24px 0">
            <a href="${link}" style="background:#000;color:#fff;text-decoration:none;padding:12px 24px;border-radius:12px;font-weight:700;display:inline-block">Confirm & unlock Pro</a>
          </p>
          <p style="color:#8f8f98;font-size:13px">If you didn't request this, you can safely ignore this email.</p>
        </div>
      `,
    }),
  });
  if (!resp.ok) {
    const body = await resp.text();
    throw new Error(`resend_error_${resp.status}: ${body.slice(0, 200)}`);
  }
}

async function readRawBody(request: Request): Promise<ArrayBuffer> {
  return await request.arrayBuffer();
}

// ---- Purchase fulfillment email (download + setup instructions) ----
async function sendPurchaseEmail(env: Env, to: string, amount: number | null, currency: string | null): Promise<void> {
  const price = amount != null ? (amount / 100).toFixed(2) : null;
  const priceLine =
    price != null
      ? `<p style="color:#52525b">Order total: <strong>${(currency ?? '').toUpperCase()} ${price}</strong></p>`
      : '';
  const resp = await fetch('https://api.resend.com/emails', {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${env.RESEND_API_KEY}`,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      from: env.MAIL_FROM || 'Siegu <pro@example.com>',
      to,
      subject: 'Welcome to Siegu Pro — your download & next steps',
      html: `
        <div style="font-family:Outfit,Arial,sans-serif;max-width:480px;margin:0 auto;color:#18181b">
          <h2 style="margin-bottom:8px">You're all set for Siegu Pro 🎉</h2>
          <p style="color:#52525b">Thanks for your purchase. Below is your download and the two quick steps to get Pro activated.</p>
          ${priceLine}
          <p style="text-align:center;margin:24px 0">
            <a href="${DOWNLOAD_URL}" style="background:#000;color:#fff;text-decoration:none;padding:14px 28px;border-radius:12px;font-weight:700;display:inline-block">Download Siegu for Linux</a>
          </p>
          <ol style="color:#52525b;line-height:1.7;padding-left:20px">
            <li>Run the downloaded file (right-click → <em>Allow launching</em>, or use a package manager).</li>
            <li>Open <strong>Settings → Pro</strong>, enter the email you used at checkout (<strong>${to}</strong>), tap <em>Send verification email</em>, then click the link you receive to unlock Pro.</li>
          </ol>
          <p style="color:#8f8f98;font-size:13px;line-height:1.5">Prefer another way to run Siegu? See <a href="${RUN_OTHER_WAYS_URL}" style="color:#000">all the ways to install and run the app</a> (Linux, macOS, Windows, Android, iOS, or from source). Need help? Reply to this email or see <a href="${SETUP_URL}" style="color:#000">siegu's site</a>.</p>
        </div>
      `,
    }),
  });
  if (!resp.ok) {
    const body = await resp.text();
    throw new Error(`purchase_resend_error_${resp.status}: ${body.slice(0, 200)}`);
  }
}

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const url = new URL(request.url);

    const corsHeaders = {
      'access-control-allow-origin': '*',
      'access-control-allow-methods': 'GET,POST,OPTIONS',
      'access-control-allow-headers': 'content-type, stripe-signature, x-siegu-token',
    };

    if (request.method === 'OPTIONS') {
      return new Response(null, { status: 204, headers: corsHeaders });
    }

    const withCors = (r: Response): Response => {
      for (const [k, v] of Object.entries(corsHeaders)) r.headers.set(k, v);
      return r;
    };

    try {
      // ---- Send verification email (requires the email to have paid)
      if (url.pathname === '/send-verify' && request.method === 'POST') {
        const email = normalizeEmail(url.searchParams.get('email') ?? '');
        const token = request.headers.get('x-siegu-token') ?? '';

        if (!email.includes('@')) {
          return withCors(json({ ok: false, error: 'email required' }, 400));
        }
        if (!token || token !== env.SIEGU_VERIFY_TOKEN) {
          return withCors(json({ ok: false, error: 'unauthorized' }, 401));
        }
        if (!env.RESEND_API_KEY) {
          return withCors(json({ ok: false, error: 'email_not_configured' }, 500));
        }

        const paid = await env.PAID_EMAILS.get(`paid:${email}`);
        if (!paid) {
          // Don't leak whether an email exists; degrade gracefully to the
          // "not paid" path the app already handles.
          return withCors(json({ ok: false, paid: false, error: 'not_paid' }, 200));
        }

        const confirmationToken = await issueToken(env, email);
        // Store a single-use marker so the same token can't be replayed.
        const digest = await hmacHex(env, `confirm:${confirmationToken}`);
        await env.PAID_EMAILS.put(`token:${digest}`, JSON.stringify({ email, used: false }), {
          expirationTtl: 30 * 60,
        });

        const link = `${url.origin}/confirm?token=${encodeParam(confirmationToken)}`;
        await sendVerifyEmail(env, email, link);

        return withCors(json({ ok: true, sent: true, email }));
      }

      // ---- Confirm link (public — no secret header, just the signed token)
      if (url.pathname === '/confirm' && request.method === 'GET') {
        const rawToken = url.searchParams.get('token') ?? '';
        const parsed = await parseToken(env, rawToken);

        if (!parsed) {
          return new Response('Invalid or expired link.', {
            status: 400,
            headers: { 'content-type': 'text/plain; charset=utf-8' },
          });
        }
        if (parsed.expires < Math.floor(Date.now() / 1000)) {
          return new Response('This link has expired. Open the app and send a new one.', {
            status: 410,
            headers: { 'content-type': 'text/plain; charset=utf-8' },
          });
        }

        // Single-use: mark and check atomically-ish (best-effort).
        const digest = await hmacHex(env, `confirm:${rawToken}`);
        const existing = await env.PAID_EMAILS.get(`token:${digest}`);
        if (!existing) {
          return new Response('This link has already been used or is invalid.', {
            status: 410,
            headers: { 'content-type': 'text/plain; charset=utf-8' },
          });
        }
        const rec = JSON.parse(existing) as { used?: boolean };
        if (rec.used) {
          return new Response('This link has already been used.', {
            status: 410,
            headers: { 'content-type': 'text/plain; charset=utf-8' },
          });
        }

        await env.PAID_EMAILS.put(`token:${digest}`, JSON.stringify({ ...rec, used: true }));
        await env.PAID_EMAILS.put(`verified:${parsed.email}`, new Date().toISOString());

        return new Response(
          '<!doctype html><html><head><meta name="viewport" content="width=device-width,initial-scale=1">' +
            '<style>body{font-family:Outfit,Arial,sans-serif;background:#fff;color:#18181b;display:flex;align-items:center;justify-content:center;height:100vh;margin:0;text-align:center}div{max-width:400px;padding:24px}h1{font-size:24px;margin:0 0 8px}.ok{width:56px;height:56px;border-radius:50%;background:#22c55e;color:#fff;font-size:30px;display:flex;align-items:center;justify-content:center;margin:0 auto 16px}p{color:#52525b;line-height:1.5}a{color:#000;font-weight:700}</style></head>' +
            '<body><div><div class="ok">✓</div><h1>Email verified!</h1>' +
            `<p>${esc(parsed.email)} is confirmed. Siegu Pro is now unlocked — go back to the app and it will activate automatically.</p>` +
            `<p><a href="${SUCCESS_URL}">Back to Siegu</a></p></div></body></html>`,
          { headers: { 'content-type': 'text/html; charset=utf-8' } },
        );
      }

      // ---- Verify entitlement (app polls this; unlocks only when paid + verified)
      if (url.pathname === '/verify' && request.method === 'GET') {
        const email = normalizeEmail(url.searchParams.get('email') ?? '');
        const token = request.headers.get('x-siegu-token') ?? '';

        if (!email) return withCors(json({ ok: false, paid: false, verified: false, error: 'email required' }, 400));
        if (!token || token !== env.SIEGU_VERIFY_TOKEN) {
          return withCors(json({ ok: false, paid: false, verified: false, error: 'unauthorized' }, 401));
        }

        const [paid, verified] = await Promise.all([
          env.PAID_EMAILS.get(`paid:${email}`),
          env.PAID_EMAILS.get(`verified:${email}`),
        ]);
        return withCors(
          json({
            ok: true,
            email,
            paid: paid !== null,
            verified: verified !== null,
            plan: paid !== null && verified !== null ? 'pro' : 'free',
          }),
        );
      }

      // ---- Stripe webhook
      if (url.pathname === '/stripe-webhook' && request.method === 'POST') {
        const raw = await readRawBody(request);
        const header = request.headers.get('stripe-signature');
        const valid = await verifyStripeSignature(env, raw, header);
        if (!valid) return withCors(json({ ok: false, error: 'invalid signature' }, 400));

        const event = JSON.parse(new TextDecoder().decode(raw));
        if (event.type !== 'checkout.session.completed') {
          return withCors(json({ ok: true, received: event.type }));
        }

        const session = event.data?.object ?? {};
        // Payment Links store the customer's entered email under
        // customer_details.email (customer_email is only for pre-filling).
        const email = normalizeEmail(
          session.customer_details?.email ?? session.customer_email ?? '',
        );

        if (email) {
          const record = {
            email,
            customer: session.customer ?? null,
            sessionId: session.id ?? null,
            amountTotal: session.amount_total ?? null,
            currency: session.currency ?? null,
            paidAt: new Date().toISOString(),
          };
          await env.PAID_EMAILS.put(`paid:${email}`, JSON.stringify(record));

          // Idempotency: Stripe retries a webhook until it gets a 2xx. Without a
          // per-event marker every retry would re-send the purchase email. Only
          // mark it sent AFTER a successful send, so a transient failure still
          // lets the retry deliver the email.
          const eventId = String(event.id ?? '');
          const sentMarker = `webhook_sent:${eventId}`;
          if (!eventId || !(await env.PAID_EMAILS.get(sentMarker))) {
            try {
              await sendPurchaseEmail(
                env,
                email,
                session.amount_total ?? null,
                session.currency ?? null,
              );
              if (eventId) {
                await env.PAID_EMAILS.put(sentMarker, new Date().toISOString(), {
                  expirationTtl: 7 * 24 * 60 * 60,
                });
              }
            } catch (err) {
              // Best-effort download + setup email. Swallow failures so a mail
              // error can't cause the handler to throw (Stripe would then
              // retry, which re-records + re-sends — the marker above prevents
              // duplicate sends on those retries once one succeeds).
              console.error(
                'purchase email failed',
                err instanceof Error ? err.message : err,
              );
            }
          }
        }

        return withCors(json({ ok: true }));
      }

      // ---- Health / root
      return withCors(json({ ok: true, service: 'siegu-pro-license' }));
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      return withCors(json({ ok: false, error: message }, 500));
    }
  },
} satisfies ExportedHandler<Env>;

function esc(s: string): string {
  return s.replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c]!);
}
