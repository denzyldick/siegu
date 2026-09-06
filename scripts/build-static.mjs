/**
 * Siegu landing — bake runtime config into the static site.
 *
 * Reads runtime config from the environment and writes a fully-baked copy of
 * ./public into ./dist (used for GitHub Pages and any other static host).
 * While a value is unset, its placeholder stays in place and the matching
 * buttons render inert (#). The Stripe Payment Link backs all Pro/upgrade
 * CTAs and opens Stripe's hosted, PCI-compliant checkout.
 *
 * Usage:
 *   GA_MEASUREMENT_ID=G-XXXXXXX \
 *   STRIPE_PRO_PAYMENT_LINK=https://buy.stripe.com/xxx \
 *     node scripts/build-static.mjs
 *
 * Stripe link vars are optional in dev (placeholder stays → button inert).
 * In production (STRICT=1, or NODE_ENV=production) they are REQUIRED and must
 * resolve to a real Stripe Payment Link — the build fails loudly instead of
 * silently shipping a dead "Pay with card" button.
 */
import { cp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = join(__dirname, '..');
const SRC = join(ROOT, 'public');
const OUT = join(ROOT, 'dist');

// Stripe Payment Links are hosted checkout URLs (buy.checkout.stripe.com).
// Anything else means the config never reached this build.
const STRIPE_URL_RE = /^https:\/\/(?:buy|checkout)\.stripe\.com\//i;
const STRICT = process.env.STRICT === '1' || process.env.NODE_ENV === 'production';

function bail(message) {
  console.error(`[build-static] ERROR: ${message}`);
  console.error('           Set STRIPE_PRO_PAYMENT_LINK_MONTHLY and');
  console.error('           STRIPE_PRO_PAYMENT_LINK_YEARLY to real Stripe');
  console.error('           Payment Link URLs (https://buy.stripe.com/...) and');
  console.error('           rebuild. This guard exists so a broken Pay button');
  console.error('           can never ship silently again.');
  process.exit(1);
}

const monthly = process.env.STRIPE_PRO_PAYMENT_LINK_MONTHLY || '';
const yearly = process.env.STRIPE_PRO_PAYMENT_LINK_YEARLY || '';

if (STRICT) {
  if (!STRIPE_URL_RE.test(monthly)) bail('STRIPE_PRO_PAYMENT_LINK_MONTHLY is missing or not a buy.stripe.com URL.');
  if (!STRIPE_URL_RE.test(yearly)) bail('STRIPE_PRO_PAYMENT_LINK_YEARLY is missing or not a buy.stripe.com URL.');
} else if (monthly || yearly) {
  // Even in dev, warn about a half-wired setup so it can't surprise you.
  if (!STRIPE_URL_RE.test(monthly) || !STRIPE_URL_RE.test(yearly)) {
    console.warn('[build-static] WARNING: Stripe links are not fully configured — Pay button will be inert.');
  }
}

const placeholders = {
  __GA_MEASUREMENT_ID__: process.env.GA_MEASUREMENT_ID || '',
  __CLARITY_PROJECT_ID__: process.env.CLARITY_PROJECT_ID || '',
  __STRIPE_PRO_PAYMENT_LINK_MONTHLY__: monthly,
  __STRIPE_PRO_PAYMENT_LINK_YEARLY__: yearly,
};

async function main() {
  await rm(OUT, { recursive: true, force: true });
  await cp(SRC, OUT, { recursive: true });

  // Build-version cache-buster: every rebuild gets a fresh `?v=...` on the
  // asset URLs so returning visitors (served with Cache-Control: max-age) don't
  // keep a stale cached main.js/CSS after a redeploy.
  const version = Date.now().toString(36);

  const htmlPath = join(OUT, 'index.html');
  let html = await readFile(htmlPath, 'utf8');
  html = html.replace(
    /css\/styles\.css/g,
    `css/styles.css?v=${version}`,
  );
  html = html.replace(
    /js\/main\.js/g,
    `js/main.js?v=${version}`,
  );
  await writeFile(htmlPath, html);

  const jsPath = join(OUT, 'js', 'main.js');
  let js = await readFile(jsPath, 'utf8');

  for (const [placeholder, value] of Object.entries(placeholders)) {
    js = js.split(placeholder).join(value);
  }

  await writeFile(jsPath, js);

  const set = Object.entries(placeholders)
    .filter(([, v]) => v)
    .map(([k]) => k);
  console.log(`[build-static] built ./dist (wired: ${set.join(', ') || 'none'}, cache v=${version})`);
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
