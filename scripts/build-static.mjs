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
 * All vars are optional.
 */
import { cp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = join(__dirname, '..');
const SRC = join(ROOT, 'public');
const OUT = join(ROOT, 'dist');

const placeholders = {
  __GA_MEASUREMENT_ID__: process.env.GA_MEASUREMENT_ID || '',
  __STRIPE_PRO_PAYMENT_LINK_MONTHLY__: process.env.STRIPE_PRO_PAYMENT_LINK_MONTHLY || '',
  __STRIPE_PRO_PAYMENT_LINK_YEARLY__: process.env.STRIPE_PRO_PAYMENT_LINK_YEARLY || '',
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
