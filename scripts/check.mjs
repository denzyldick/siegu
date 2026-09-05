/**
 * Siegu landing — project-wide health check.
 *
 *   node scripts/check.mjs                # translations, routes, page integrity
 *   node scripts/check.mjs --require-payment
 *                                         # ALSO require the built dist/js/main.js
 *                                         # to have real Stripe Payment Links wired
 *                                         # (fails on inert/placeholder Pay button)
 *
 * Node-only, no deps, CI-safe.
 */
import { spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { readFile, readdir, stat } from 'node:fs/promises';
import { join, dirname, extname } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = join(__dirname, '..');
const PUBLIC = join(ROOT, 'public');
const DIST = join(ROOT, 'dist');

const REQUIRE_PAYMENT = process.argv.includes('--require-payment');
const STRIPE_URL_RE = /^https:\/\/(?:buy|checkout)\.stripe\.com\//i;
const PLACEHOLDER = /__STRIPE_PRO_PAYMENT_LINK_[A-Z_]+__/;

let failed = false;
const ok = (m) => console.log(`  ✓ ${m}`);
const fail = (m) => { console.error(`  ✗ ${m}`); failed = true; };

const MIME = {
  '.html': 'text/html',
  '.js': 'text/javascript',
  '.css': 'text/css',
  '.json': 'application/json',
  '.png': 'image/png',
  '.webp': 'image/webp',
  '.svg': 'image/svg+xml',
  '.xml': 'application/xml',
  '.txt': 'text/plain',
  '.ico': 'image/x-icon',
  '.webmanifest': 'application/manifest+json',
};

async function runNode(script) {
  return new Promise((resolve) => {
    const p = spawn(process.execPath, ['scripts/' + script], { cwd: ROOT, stdio: 'inherit' });
    p.on('exit', (code) => resolve(code === 0));
  });
}

async function collectHtmlFiles() {
  const out = [];
  const walk = async (dir, base) => {
    for (const e of await readdir(dir)) {
      const p = join(dir, e);
      const s = await stat(p);
      if (s.isDirectory()) await walk(p, base + e + '/');
      else if (e.endsWith('.html')) out.push(base + e);
    }
  };
  await walk(PUBLIC, '');
  return out;
}

async function main() {
  console.log('[check] translations');
  if (!(await runNode('check-translations.js'))) fail('translation check passed=false');
  console.log('[check] routes & assets (public/)');

  const server = createServer(async (req, res) => {
    let urlPath = decodeURIComponent(new URL(req.url, 'http://x').pathname);
    if (urlPath.endsWith('/')) urlPath += 'index.html';
    const file = join(PUBLIC, urlPath);
    try {
      const data = await readFile(file);
      res.writeHead(200, { 'Content-Type': MIME[extname(file)] || 'application/octet-stream' });
      res.end(data);
    } catch {
      res.writeHead(404);
      res.end('not found');
    }
  });
  const port = 4187;
  await new Promise((r) => server.listen(port, r));
  const base = `http://127.0.0.1:${port}/`;

  try {
    const pages = await collectHtmlFiles();
    const routes = [
      'index.html',
      ...pages,
      'css/styles.css',
      'js/main.js',
      'slides.json',
      'shots/library.webp',
      'shots/album.webp',
      'shots/locations.webp',
      'shots/viewer.webp',
      'shots/share.webp',
      'shots/banner.webp',
      'logo.png',
      'logo.svg',
      'banner.png',
      'og-image.png',
      'favicon.png',
      'favicon-white.png',
      'manifest.webmanifest',
      'robots.txt',
      'sitemap.xml',
      '404.html',
    ];

    for (const route of routes) {
      try {
        const res = await fetch(base + route);
        if (res.status !== 200) { fail(`${route} -> ${res.status}`); continue; }
        const body = await res.text();
        if (!body || !body.length) fail(`${route} -> empty body`);
        else ok(`${route} -> 200 (${body.length}b)`);
      } catch (e) {
        fail(`${route} -> fetch error ${e.message}`);
      }
    }

    // Per-page integrity
    console.log('[check] page integrity');
    const deadHref = /<a[^>]+href="#'[^>]*>/i;
    for (const page of pages) {
      const html = await readFile(join(PUBLIC, page), 'utf8');
      const path = page === 'index.html' ? '' : page.replace(/\.html$/, '');
      let pageOk = true;
      if (!/css\/styles\.css/.test(html)) { fail(`${page}: missing css/styles.css`); pageOk = false; }
      if (page !== '404.html') {
        if (!/js\/main\.js/.test(html)) { fail(`${page}: missing js/main.js`); pageOk = false; }
        if (!/data-i18n=/.test(html)) { fail(`${page}: no data-i18n hooks`); pageOk = false; }
      }
      const footer = html.slice(html.indexOf('<footer')).slice(0, 20000);
      if (/href="#"/.test(footer)) { fail(`${page}: dead href="#" in footer`); pageOk = false; }
      // Local link checker: ignore cross-page canonical <link> and external URLs.
      const localLinks = [...html.matchAll(/href="((?!https?:\/\/)[^"#]+\.html(?:#[^"']*)?)"/g)].map((m) => m[1].split('#')[0]);
      for (const link of new Set(localLinks)) {
        if (link.startsWith('/')) continue; // absolute-site path, not a local file
        try {
          await readFile(join(PUBLIC, link));
        } catch {
          fail(`${page}: internal link "${link}" has no target file`);
          pageOk = false;
        }
      }
      if (page === 'index.html') {
        for (const id of ['searchTrigger', 'themeBtn', 'pricingGrid', 'faqList', 'heroBg', 'proModal', 'proPayBtn']) {
          if (!html.includes(`id="${id}"`)) { fail(`index.html missing #${id}`); pageOk = false; }
        }
      }
      // Mobile nav + skip link on content pages (404 is a minimal standalone page)
      if (page !== '404.html') {
        if (!html.includes('id="menuBtn"') || !html.includes('id="navOverlay"')) {
          fail(`${page}: missing mobile hamburger (#menuBtn / #navOverlay)`);
          pageOk = false;
        }
        if (!html.includes('class="skip"')) { fail(`${page}: missing skip link`); pageOk = false; }
      }
      // aria-current: only top-level nav targets carry it
      const navTargets = ['pricing.html', 'faq.html', 'docs.html', 'download.html'];
      if (navTargets.includes(page)) {
        const pageHref = page.replace(/\.html$/, '') + '.html';
        const anchor = html.match(new RegExp(`<a[^>]*href="${pageHref}"[^>]*>`));
        if (!anchor || !/aria-current="page"/.test(anchor[0])) {
          fail(`${page}: current-page nav link not marked aria-current="page"`);
          pageOk = false;
        }
      }
      if (pageOk) ok(`${page}: markup + links OK (${path || '/'})`);
    }
  } finally {
    server.close();
  }

  // Build-wiring guard (the payment regression test)
  console.log('[check] build wiring (dist/js/main.js)');
  let distStatus = 'no dist — run `node scripts/build-static.mjs` first';
  try {
    const js = await readFile(join(DIST, 'js', 'main.js'), 'utf8');
    const month = js.match(/STRIPE_PRO_PAYMENT_LINK_MONTHLY = '([^']*)'/) || ['', ''];
    const year = js.match(/STRIPE_PRO_PAYMENT_LINK_YEARLY = '([^']*)'/) || ['', ''];
    const mVal = month[1] || '';
    const yVal = year[1] || '';
    const hasPlaceholder = PLACEHOLDER.test(js);
    const wired = STRIPE_URL_RE.test(mVal) && STRIPE_URL_RE.test(yVal);

    if (REQUIRE_PAYMENT && (hasPlaceholder || !wired)) {
      fail('Pay button would be INERT (#) — Stripe Payment Links are not wired into dist/js/main.js');
      fail(`  monthly=${mVal || '(empty/placeholder)'} yearly=${yVal || '(empty/placeholder)'}`);
      distStatus = 'INERT ⚠';
    } else if (!wired) {
      console.warn('  ⚠ dist Present but Stripe links not wired (ok in dev, never ship it):');
      console.warn(`    monthly=${mVal || '(empty)'} yearly=${yVal || '(empty)'}`);
      distStatus = 'dev-only (links not wired)';
    } else {
      ok(`dist payment wiring OK (${mVal} / ${yVal})`);
      distStatus = 'wired ✓';
    }
  } catch {
    if (REQUIRE_PAYMENT) fail('dist/js/main.js missing — nothing to verify for payment');
    else console.warn('  - dist missing — run `node scripts/build-static.mjs` before deploy verification');
  }

  console.log(`\n[check] done. payment: ${distStatus}`);
  if (failed) {
    console.error('\n[check] FAILED.');
    process.exit(1);
  }
  console.log('[check] OK.');
}

main().catch((e) => { console.error(e); process.exit(1); });