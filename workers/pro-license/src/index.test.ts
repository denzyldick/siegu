import { describe, it, expect, beforeEach, vi } from 'vitest';
import { createHmac } from 'node:crypto';
import worker from './index';
import type { Env } from './index';

const STRIPE_SECRET = 'whsec_test_secret';

/** Minimal in-memory KVNamespace stand-in for tests. */
class FakeKV {
  private store = new Map<string, string>();
  private expiries = new Map<string, number>();

  async get(key: string): Promise<string | null> {
    const exp = this.expiries.get(key);
    if (exp !== undefined && exp < Date.now()) return null;
    return this.store.get(key) ?? null;
  }
  async put(
    key: string,
    value: string,
    opts: { expirationTtl?: number } = {},
  ): Promise<void> {
    this.store.set(key, value);
    if (opts.expirationTtl) this.expiries.set(key, Date.now() + opts.expirationTtl * 1000);
  }
  async delete(key: string): Promise<void> {
    this.store.delete(key);
  }
}

function makeEnv(): Env {
  return {
    PAID_EMAILS: new FakeKV() as unknown as KVNamespace,
    STRIPE_WEBHOOK_SECRET: STRIPE_SECRET,
    SIEGU_VERIFY_TOKEN: 'app-token',
    CONFIRM_SIGNING_SECRET: 'confirm-secret',
    RESEND_API_KEY: 're_test',
    MAIL_FROM: 'Siegu <pro@example.com>',
  };
}

/** Build a valid `stripe-signature` header using Stripe's HMAC scheme. */
function stripeHeader(body: string, secret = STRIPE_SECRET, timestamp = Math.floor(Date.now() / 1000)): string {
  const signed = `${timestamp}.${body}`;
  const mac = createHmac('sha256', secret).update(signed).digest('hex');
  return `t=${timestamp},v1=${mac}`;
}

function completedEvent(eventId: string): string {
  return JSON.stringify({
    id: eventId,
    type: 'checkout.session.completed',
    data: {
      object: {
        id: 'cs_test_1',
        customer: 'cus_1',
        amount_total: 1200,
        currency: 'usd',
        customer_details: { email: 'Buyer@Example.com' },
      },
    },
  });
}

describe('pro-license worker — Stripe webhook idempotency', () => {
  let env: Env;
  let resend: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    vi.restoreAllMocks();
    env = makeEnv();
    resend = vi.fn(async () => new Response('', { status: 200 }));
    vi.stubGlobal('fetch', async (...args: Parameters<typeof fetch>) => resend(...args) as Promise<Response>);
  });

  it('records payment and sends one purchase email on the first delivery', async () => {
    const body = completedEvent('evt_1');
    const request = new Request('https://worker.test/stripe-webhook', {
      method: 'POST',
      headers: { 'stripe-signature': stripeHeader(body) },
      body,
    });
    const resp = await worker.fetch(request, env);
    expect(resp.status).toBe(200);
    // Email normalized (lowercased/trimmed) and stored as paid.
    expect(await env.PAID_EMAILS.get('paid:buyer@example.com')).toBeTruthy();
    // Exactly one purchase email sent, to the normalized address.
    expect(resend).toHaveBeenCalledTimes(1);
    const sentBody = JSON.parse(String(resend.mock.calls[0][1].body));
    expect(sentBody.to).toBe('buyer@example.com');
  });

  it('does not resend the purchase email when Stripe retries the same event', async () => {
    const body = completedEvent('evt_dup');
    const header = stripeHeader(body);
    for (let i = 0; i < 3; i++) {
      const request = new Request('https://worker.test/stripe-webhook', {
        method: 'POST',
        headers: { 'stripe-signature': header },
        body,
      });
      const resp = await worker.fetch(request, env);
      expect(resp.status).toBe(200);
    }
    // Idempotent: only ONE purchase email across the 3 identical retries.
    expect(resend).toHaveBeenCalledTimes(1);
    expect(await env.PAID_EMAILS.get('webhook_sent:evt_dup')).toBeTruthy();
  });

  it('rejects a webhook with an invalid signature', async () => {
    const request = new Request('https://worker.test/stripe-webhook', {
      method: 'POST',
      headers: { 'stripe-signature': `t=${Math.floor(Date.now() / 1000)},v1=bogus` },
      body: completedEvent('evt_x'),
    });
    const resp = await worker.fetch(request, env);
    expect(resp.status).toBe(400);
    expect(resend).not.toHaveBeenCalled();
  });
});
