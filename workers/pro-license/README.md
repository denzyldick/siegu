# Siegu Pro License worker

A Cloudflare Worker that is the **merchant-of-record hook** for Siegu Pro: it
records which email has paid (via a Stripe `checkout.session.completed`
webhook) and answers the desktop app's "is this email paid?" question so the
app can unlock Pro.

## Flow

1. User clicks **Upgrade to Pro** on the landing page → Stripe Payment Link
   checkout.
2. On successful payment, Stripe POSTs `checkout.session.completed` to
   `POST /webhook`. The worker verifies the Stripe signature and stores
   `paid:<email>` in KV (idempotent).
3. In the desktop app, the user enters the email they paid with and clicks
   **Verify & unlock Pro**. The app calls `GET /verify?email=...` with the
   shared token; if the email is in KV, the app sets `tier=paid` locally.

## Endpoints

| Method | Path        | Purpose                                             |
| ------ | ----------- | --------------------------------------------------- |
| POST   | `/webhook`  | Stripe webhook (verifies signature, records email)  |
| GET    | `/verify`   | Checks `?email=` against KV; needs `x-siegu-token`  |
| GET    | `/`         | Health                                            |

`/verify` requires the `x-siegu-token` header equal to the `SIEGU_VERIFY_TOKEN`
secret. `/webhook` requires a valid `stripe-signature` header.

## Deploy

```bash
cd workers/pro-license
npm install

# 1. Create the KV namespace and paste its id/preview_id into wrangler.toml
npx wrangler kv namespace create PAID_EMAILS

# 2. Set secrets (never commit these)
npx wrangler secret put STRIPE_WEBHOOK_SECRET    # whsec_... from the Stripe dashboard
npx wrangler secret put SIEGU_VERIFY_TOKEN       # shared secret the app will send

# 3. Deploy
npx wrangler deploy
```

After deploying, in the **Stripe Dashboard** (Test or Live mode matching the
landing page's Payment Links) add a webhook endpoint URL
`https://siegu-pro-license.<your-subdomain>.workers.dev/webhook` and subscribe
to the **`checkout.session.completed`** event. Copy the `whsec_...` signing
secret into `STRIPE_WEBHOOK_SECRET`.

> The Stripe Payment Links and the webhook must be in the **same mode** (test
> vs live). The landing page currently uses test Payment Links; run this worker
> webhook in test mode too until you go live.

## Local dev

```bash
npx wrangler dev
curl "http://localhost:8787/verify?email=you@example.com" \
  -H "x-siegu-token: $SIEGU_VERIFY_TOKEN"
```

## Caveat / roadmap

There is **no paid/Pro feature gating in the Siegu desktop app yet** — this
worker records entitlement, and the app stores `tier=paid`, but nothing is
locked in the app. Gating real features behind `tier` is future work.
