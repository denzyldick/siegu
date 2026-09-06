# Siegu — live checkout launch checklist

Everything below is **user action** — no code changes are needed once the
secrets are set. After each secret update, push to `master`; CI rebuilds
the strict bundle and deploys it automatically.

---

## 1. Create live Stripe products & payment links

1. Open **[Stripe Dashboard → Products](https://dashboard.stripe.com/test/products)**
   (switch to **Live mode** in the sidebar).
2. Create a new **Recurring** product:

| Field | Value |
|-------|-------|
| Name | Siegu Pro |
| Description | Unlimited photo library · Encrypted sync & sharing · Early access |
| Pricing model | Standard pricing |
| Price 1 | **$7.99 / month**, billing period **Yearly**, trial **None** |
| Price 2 | **$9.99 / month**, billing period **Monthly**, trial **None** |

3. On the product page, click **Add payment link** for the Yearly price →
   copy the link (starts with `https://buy.stripe.com/...`).
4. Repeat for the Monthly price.

---

## 2. Set the secrets

```bash
gh secret set STRIPE_PRO_PAYMENT_LINK_MONTHLY  --body "https://buy.stripe.com/..."
gh secret set STRIPE_PRO_PAYMENT_LINK_YEARLY   --body "https://buy.stripe.com/..."
```

That's it — push to `master` (or wait for the next push) and CI will bake
the live links into the deployed bundle.

### Verify

```bash
# Wait ~30 s for deploy to finish, then:
curl -s "https://denzyldick.github.io/siegu/js/main.js?v=$(date +%s)" | grep -E 'STRIPE_PRO_PAYMENT_LINK'
```

Both lines should show real `https://buy.stripe.com/...` URLs, not the
test links.

Open `pricing.html` or `connect.html` in an incognito window → **Upgrade
to Pro** → **Pay with card** should open the live Stripe Checkout.

---

## 3. Enable the Founding (lifetime) offer

When you're ready to sell a one-time **$99 Lifetime Pro** offer:

1. In Stripe → Products → **Add one-time product**:

| Field | Value |
|-------|-------|
| Name | Siegu Pro — Lifetime |
| Description | One-time purchase · Lifetime Pro access · All future updates included |
| Price | **$99.00** |

2. Create a **Payment Link** (not a checkout session) from that product.
3. Set the secret:

```bash
gh secret set FOUNDING_PRO_PAYMENT_LINK --body "https://buy.stripe.com/..."
```

CI will bake it in; the "Lifetime Pro for $99" line appears automatically
on `pricing.html` and inside the Pro modal. Remove or clear the secret
later to hide the offer again.

---

## 4. Sell the Family plan (optional)

Family currently shows a waitlist modal. To sell it:

1. Repeat step 1 for a **Family** product (6 users, encrypted sync,
   sharing). Suggested pricing: $29.99 / month or $23.99 / month billed
   yearly.
2. Set:

```bash
gh secret set STRIPE_FAMILY_PAYMENT_LINK_MONTHLY --body "https://buy.stripe.com/..."
gh secret set STRIPE_FAMILY_PAYMENT_LINK_YEARLY  --body "https://buy.stripe.com/..."
```

3. The waitlist buttons will need a small code change (see
   `renderPricing` in `main.js`) to open the Stripe link instead of the
   waitlist modal. That can be done when you're ready.

---

## 5. Post-launch smoke tests

| What | How |
|------|-----|
| Checkout opens | Incognito → pricing → Pro → Pay with card |
| Subscription created | Stripe → Subscriptions shows a new entry |
| App unlock works | After paying, open Siegu → app shows Pro features |
| Founding link works | pricing → Lifetime → opens correct checkout |
| Mobile | iOS Safari + Android Chrome → same flow |
| Consents | Incognito → declining consent → no GA/Clarity fires |

---

## Current secrets summary

| Secret | Purpose |
|--------|---------|
| `STRIPE_PRO_PAYMENT_LINK_MONTHLY` | Pro monthly checkout link (live) |
| `STRIPE_PRO_PAYMENT_LINK_YEARLY` | Pro yearly checkout link (live) |
| `GA_MEASUREMENT_ID` | Google Analytics 4 (`G-Z1QJYVPR46`) |
| `CLARITY_PROJECT_ID` | Microsoft Clarity heatmaps (`ye3gmjqs0g`) |
| `FOUNDING_PRO_PAYMENT_LINK` | Lifetime Pro $99 (optional — set to enable, clear to hide) |
