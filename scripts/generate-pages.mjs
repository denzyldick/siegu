/**
 * Siegu landing — generate the SEO subpages (pricing, faq, docs, download).
 *
 * Reads ./public/index.html and extracts the shared shell (head wiring,
 * header, footer, modals, command palette) so every page stays in sync.
 * Pass in its own <main> content and page-specific SEO meta.
 *
 * Usage:
 *   node scripts/generate-pages.mjs
 */
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = join(__dirname, '..');
const SRC = join(ROOT, 'public');

const BASE = 'https://denzyldick.github.io/siegu';

const SITE_SCHEMA_ORG = `{
    "@type": "Organization",
    "@id": "${BASE}/#organization",
    "name": "Siegu",
    "url": "${BASE}/",
    "logo": "${BASE}/logo.png",
    "sameAs": ["https://github.com/denzyldick/siegu"]
  }`;

function headFor({ title, description, keywords, url, ogTitle, ogDesc, schema }) {
  return `<!doctype html>
<html lang="en" data-theme="system">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <title>${title}</title>
  <meta name="description" content="${description}" />
  <script>
    (function () {
      try {
        var v = localStorage.getItem('siegu_theme');
        var mode = ['light', 'dark', 'system'].indexOf(v) !== -1 ? v : 'system';
        var dark = mode === 'dark' ||
          (mode === 'system' && window.matchMedia('(prefers-color-scheme: dark)').matches);
        document.documentElement.setAttribute('data-theme', dark ? 'dark' : 'light');
      } catch (e) { /* fallback to CSS media query */ }
    })();
  </script>
  <!-- Open Graph -->
  <meta property="og:type" content="website" />
  <meta property="og:site_name" content="Siegu" />
  <meta property="og:title" content="${ogTitle || title}" />
  <meta property="og:description" content="${ogDesc || description}" />
  <meta property="og:url" content="${url}" />
  <meta property="og:image" content="${BASE}/og-image.png" />
  <meta property="og:image:width" content="1200" />
  <meta property="og:image:height" content="630" />
  <meta name="twitter:card" content="summary_large_image" />
  <meta name="twitter:title" content="${ogTitle || title}" />
  <meta name="twitter:description" content="${ogDesc || description}" />
  <meta name="twitter:image" content="${BASE}/og-image.png" />

  <!-- SEO -->
  <link rel="canonical" href="${url}" />
  <meta name="robots" content="index, follow" />
  <meta name="author" content="Denzyl Dick" />
  <meta name="keywords" content="${keywords}" />
  <meta name="theme-color" content="#0b0b0b" />
  <meta property="og:locale" content="en_US" />

  <script type="application/ld+json">
  {
    "@context": "https://schema.org",
    "@graph": [
      ${schema}
    ]
  }
  </script>

  <link rel="icon" href="favicon.png" />
  <link rel="icon" href="favicon-white.png" media="(prefers-color-scheme: dark)" />
  <link rel="apple-touch-icon" href="logo.png" />
  <link rel="manifest" href="manifest.webmanifest" />

  <link rel="preconnect" href="https://fonts.googleapis.com" />
  <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin />
  <link href="https://fonts.googleapis.com/css2?family=Outfit:wght@300;400;500;600;700;800;900&display=swap" rel="stylesheet" />

  <link rel="stylesheet" href="css/styles.css" />
</head>
<body>`;
}

const NAV_ITEMS = [
  { href: 'index.html#features', key: 'nav.features', label: 'Features' },
  { href: 'pricing.html', key: 'nav.pricing', label: 'Pricing' },
  { href: 'faq.html', key: 'nav.faq', label: 'FAQ' },
  { href: 'docs.html', key: 'nav.docs', label: 'Docs' },
  { href: 'download.html', key: 'nav.download', label: 'Download' },
  { href: 'https://siegu.onrender.com', key: 'nav.demo', label: 'Live demo', external: true },
];

function bodyTop(activeHref) {
  const nav = NAV_ITEMS.map((n) => {
    const active = n.href === activeHref ? ' class="is-active"' : '';
    const ext = n.external ? ' target="_blank" rel="noopener" data-track="demo_clicked"' : '';
    return `        <a href="${n.href}"${active} data-i18n="${n.key}"${ext}>${n.label}</a>`;
  }).join('\n');
  return `<body>
  <a class="skip" href="#main" style="position:absolute;left:-9999px;top:-9999px">Skip to content</a>
  <noscript>
    <p style="background:#1f1f1f;color:#e8e8e8;text-align:center;padding:10px 16px;margin:0;font-size:14px">
      Siegu needs JavaScript to show the download buttons and live demo. It's free, private, and works offline.
    </p>
  </noscript>

  <header class="site-header">
    <div class="container header-inner">
      <a href="index.html#top" class="brand"><img src="logo.png" alt="siegu" /><span>siegu</span></a>
      <nav class="nav" aria-label="Primary">
${nav}
      </nav>
      <div class="header-actions">
        <button class="search-trigger" id="searchTrigger" type="button" aria-label="Search (Ctrl+K)" data-i18n-aria="search.trigger_label">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>
          <span class="search-shortcut">⌘K</span>
        </button>
        <button class="theme-btn" id="themeBtn" type="button" aria-label="Toggle theme">☾</button>
        <div class="lang-switch">
          <button class="lang-btn" id="langBtn" type="button">
            <span id="langCode">EN</span>
            <span>▾</span>
          </button>
          <div class="lang-menu" id="langMenu"></div>
        </div>
        <a class="btn btn-ink btn-sm" href="#" data-track="cta_get_started" data-i18n="nav.get_free">Get siegu free</a>
      </div>
    </div>
  </header>

  <main id="main">`;
}

const TAIL = `  </main>

  <footer class="site-footer">
    <div class="container">
      <div class="footer-grid">
        <div class="footer-brand">
          <a href="index.html#top" class="brand"><img src="logo.png" alt="siegu" /><span>siegu</span></a>
          <p data-i18n="footer.tagline">A private, local-first photo library.</p>
        </div>
        <div class="footer-col">
          <h5 data-i18n="footer.product">Product</h5>
          <a href="index.html#features" data-i18n="footer.product_links.features">Features</a>
          <a href="pricing.html" data-i18n="footer.product_links.pricing">Pricing</a>
          <a href="#" data-i18n="footer.product_links.changelog">Changelog</a>
          <a href="#" data-i18n="footer.product_links.roadmap">Roadmap</a>
        </div>
        <div class="footer-col">
          <h5 data-i18n="footer.resources">Resources</h5>
          <a href="https://github.com/denzyldick/siegu/tree/main/docs" target="_blank" rel="noopener" data-i18n="footer.resources_links.docs">Documentation</a>
          <a href="download.html" data-track="cta_get_started" data-i18n="footer.resources_links.download">Download</a>
          <a href="compare.html" data-i18n="footer.resources_links.compare">vs Google Photos</a>
          <a href="https://siegu.onrender.com" target="_blank" rel="noopener" data-track="demo_clicked" data-i18n="footer.resources_links.demo">Live demo</a>
        </div>
        <div class="footer-col">
          <h5 data-i18n="footer.company">Company</h5>
          <a href="#" data-i18n="footer.company_links.about">About</a>
          <a href="https://github.com/denzyldick/siegu" target="_blank" rel="noopener" data-i18n="footer.company_links.github">GitHub</a>
          <a href="#" data-i18n="footer.company_links.blog">Blog</a>
        </div>
      </div>
      <div class="footer-bottom">
        <span>© <span id="year"></span> siegu. <span data-i18n="footer.rights">All rights reserved.</span></span>
        <a href="#" data-i18n="footer.legal">Privacy & Terms</a>
      </div>
    </div>
  </footer>

  <!-- Download modal: pick your OS -->
  <div class="dl-overlay" id="downloadModal" aria-hidden="true" role="dialog" aria-label="Download Siegu">
    <div class="dl-dialog">
      <button class="dl-close" data-dl-close type="button" aria-label="Close">&times;</button>
      <h3 data-i18n="download.title">Get Siegu free</h3>
      <p class="dl-sub" data-i18n="download.desc">Free forever, private, and yours. Pick your platform to download the latest build.</p>
      <div class="dl-grid" id="dlGrid"><!-- filled by main.js --></div>
      <p class="dl-note" data-i18n="download.note">All downloads check in at zero cost — no account, no cloud.</p>
    </div>
  </div>

  <!-- Pro modal: explain + pay -->
  <div class="dl-overlay" id="proModal" aria-hidden="true" role="dialog" aria-label="Upgrade to Pro">
    <div class="dl-dialog">
      <button class="dl-close" data-pro-close type="button" aria-label="Close">&times;</button>
      <h3 data-i18n="pro.title">Upgrade to Pro</h3>
      <p class="dl-sub" data-i18n="pro.desc">Unlock everything. One price, every device, no subscriptions quirks.</p>
      <ul class="pro-benefits" id="proBenefits"><!-- filled by main.js --></ul>
      <div class="pro-pay-row">
        <span class="pro-price" id="proPrice">$9.99<span data-i18n="pro.per_month"> /month</span></span>
        <a class="btn btn-ink btn-lg" id="proPayBtn" data-track="upgrade_clicked" target="_blank" rel="noopener" data-i18n="pro.pay">Pay with card</a>
      </div>
      <p class="dl-note" data-i18n="pro.note">Secure checkout by Stripe. Cancel anytime.</p>
    </div>
  </div>

  <!-- Waitlist modal (Family plan) -->
  <div class="waitlist-overlay" id="waitlistModal" aria-hidden="true" role="dialog" aria-label="Join waitlist">
    <div class="waitlist-dialog">
      <button class="waitlist-close" id="waitlistClose" type="button" aria-label="Close">&times;</button>
      <h3 data-i18n="waitlist.title">You're early — and that's a great thing.</h3>
      <p data-i18n="waitlist.desc">Family sharing is launching soon. Drop your email and we'll let you know the moment it's ready — plus an early-bird discount.</p>
      <form class="waitlist-modal-form" action="https://formspree.io/f/mrpgkbyj" method="POST">
        <input type="email" name="email" placeholder="your@email.com" required aria-label="Email address" />
        <input type="hidden" name="source" id="waitlistSource" value="family" />
        <button type="submit" class="btn btn-ink btn-lg" data-track="waitlist_submit" data-i18n="waitlist.cta">Join waitlist</button>
      </form>
      <p class="waitlist-note" data-i18n="waitlist.note">No spam. Just a heads-up when it's ready.</p>
    </div>
  </div>

  <!-- Command palette / quick search (Ctrl+K / ⌘K) -->
  <div class="cmd-overlay" id="cmdPalette" aria-hidden="true" role="dialog" aria-modal="true" aria-label="Quick search" data-i18n-aria="search.aria">
    <div class="cmd-dialog">
      <div class="cmd-input-wrap">
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>
        <input type="text" id="cmdInput" autocomplete="off" spellcheck="false" placeholder="Search…" data-i18n-placeholder="search.placeholder" />
        <kbd class="cmd-kbd">ESC</kbd>
      </div>
      <div class="cmd-groups" id="cmdGroups"><!-- filled by main.js --></div>
      <div class="cmd-empty" id="cmdEmpty" data-i18n="search.no_results" hidden>No results</div>
      <div class="cmd-footer">
        <span><kbd>↑</kbd><kbd>↓</kbd> <span data-i18n="search.nav">to navigate</span></span>
        <span><kbd>↵</kbd> <span data-i18n="search.select">to select</span></span>
        <span><span data-i18n="search.theme">Theme</span>: <button id="cmdTheme" type="button"><span data-i18n="search.toggle">Toggle</span></button></span>
      </div>
    </div>
  </div>

  <script src="js/main.js"></script>
</body>
</html>`;

/* ---------- Subpage content ---------- */

const PRICING_MAIN = `
    <section class="page-hero">
      <div class="container">
        <p class="eyebrow" data-i18n="pricing.eyebrow">Pricing</p>
        <h1 data-i18n="pricing.title">Simple, honest pricing</h1>
        <p class="sub" data-i18n="pricing.subtitle">Everything is local and private. Pay for convenience, not for your data.</p>
      </div>
    </section>

    <section class="section">
      <div class="container center pricing-head">
        <div class="trust-row">
          <span data-i18n="trust.open_source">Open source</span><span class="tdot">·</span>
          <span data-i18n="trust.no_uploads">No uploads</span><span class="tdot">·</span>
          <span data-i18n="trust.local">Your photos never leave your device</span>
        </div>
        <div class="toggle-row" id="billingToggle">
          <button type="button" data-period="monthly" data-i18n="pricing.toggle.monthly">Monthly</button>
          <button type="button" class="active" data-period="yearly" data-i18n="pricing.toggle.yearly">Yearly</button>
          <span class="save-tag" data-i18n="pricing.toggle.save">Save 20%</span>
        </div>
      </div>
      <div class="container">
        <div class="pricing-grid" id="pricingGrid">
          <!-- Plan cards injected by js/main.js -->
        </div>
        <p class="pricing-note center" data-i18n="pricing.note">Prices in USD. Cancel anytime. No extra fees for more photos.</p>
      </div>
    </section>`;

const FAQ_MAIN = `
    <section class="page-hero">
      <div class="container">
        <p class="eyebrow" data-i18n="faq.eyebrow">FAQ</p>
        <h1 data-i18n="faq.title">Questions, answered</h1>
        <p class="sub">Everything about Siegu, your privacy, and how it works.</p>
      </div>
    </section>

    <section class="section">
      <div class="container">
        <div class="faq-list faq-page-list" id="faqList"><!-- injected --></div>
      </div>
    </section>`;

const DOWNLOAD_MAIN = `
    <section class="page-hero">
      <div class="container">
        <p class="eyebrow" data-i18n="download.title">Get Siegu free</p>
        <h1>Download Siegu</h1>
        <p class="sub" data-i18n="download.desc">Free forever, private, and yours. Pick your platform to download the latest build. Available for Windows, macOS, Linux, and Android.</p>
      </div>
    </section>

    <section class="section">
      <div class="container">
        <div class="dl-grid dl-page-grid" id="dlPageGrid"><!-- filled by main.js --></div>
        <p class="dl-page-note" data-i18n="download.note">All downloads are free — no account, no cloud.</p>
      </div>
    </section>`;

const DOCS_MAIN = `
    <section class="page-hero">
      <div class="container">
        <p class="eyebrow">Documentation</p>
        <h1>Siegu docs</h1>
        <p class="sub">Guides, troubleshooting, and everything you need to get the most out of Siegu.</p>
      </div>
    </section>

    <section class="section page-body">
      <div class="container">
        <div class="narrow">
          <p>Siegu is a private, local-first photo library. Your photos and metadata stay on your device, organized and searchable with on-device AI. You own your library — no cloud, no uploads, no lock-in.</p>
          <p>The full documentation lives on GitHub, alongside the open-source code.</p>
          <p>Popular topics:</p>
          <ul>
            <li><a href="https://github.com/denzyldick/siegu/tree/main/docs" target="_blank" rel="noopener">Documentation home</a></li>
            <li><a href="https://github.com/denzyldick/siegu/blob/main/README.md" target="_blank" rel="noopener">Readme &amp; getting started</a></li>
            <li><a href="https://github.com/denzyldick/siegu" target="_blank" rel="noopener">Source code on GitHub</a></li>
          </ul>
        </div>
      </div>
    </section>`;

const COMPARE_MAIN = `
    <section class="page-hero">
      <div class="container">
        <p class="eyebrow">Private alternative to Google Photos</p>
        <h1>Siegu vs Google Photos</h1>
        <p class="sub">Both organize your life in photos. Siegu keeps everything on your device — private, searchable, and yours. No cloud, no ads, no training on your memories.</p>
      </div>
    </section>

    <section class="section page-body">
      <div class="container">
        <div class="narrow">
          <h2>Why people leave Google Photos</h2>
          <p>Google Photos scans your library, shows you ads, and trains its AI on what you share. Your photos live on Google&rsquo;s servers, and leaving the service means exporting thousands of files you never fully controlled. Siegu flips that: your photos and the AI that organizes them both run on your device.</p>

          <h2>At a glance</h2>
          <div class="compare-wrap">
            <table class="compare-table">
              <thead>
                <tr><th></th><th>Google Photos</th><th>Siegu</th></tr>
              </thead>
              <tbody>
                <tr><td>Where your photos live</td><td>Google servers</td><td>Only on your device</td></tr>
                <tr><td>Offline search</td><td>Limited</td><td>Full on-device AI search</td></tr>
                <tr><td>Ads / data mining</td><td>Yes</td><td>Never</td></tr>
                <tr><td>AI that knows your library</td><td>Trains on your uploads</td><td>Runs locally, privately</td></tr>
                <tr><td>Free storage limit</td><td>15&nbsp;GB (shared)</td><td>Unlimited, your disk</td></tr>
                <tr><td>Export / lock-in</td><td>Takeout required</td><td>Files stay local</td></tr>
                <tr><td>Open source</td><td>No</td><td>Yes</td></tr>
              </tbody>
            </table>
          </div>

          <h2>What you get instead</h2>
          <ul>
            <li><strong>Private by design.</strong> Nothing is uploaded, ever. Not photos, not faces, not your search history.</li>
            <li><strong>Search that understands you.</strong> Ask in natural language and get answers from your own library &mdash; computed on your hardware.</li>
            <li><strong>Yours to keep.</strong> The app is open source and your library never leaves your drive. There is no service to cancel.</li>
          </ul>

          <p>Curious? <a href="pricing.html">See the plans</a> or <a href="download.html">download Siegu for free</a>.</p>
        </div>
      </div>
    </section>`;

const FAQPAGE_SCHEMA = [
  { q: 'Is my data really private?', a: 'Yes. Your photos and metadata stay on your device. Siegu uses on-device AI, so nothing is uploaded for processing.' },
  { q: 'Does Siegu work offline?', a: 'Absolutely. Your library is stored locally, so you can browse and search your photos anywhere — even without a connection.' },
  { q: 'How does sharing work?', a: 'You can share a collection over a live, end-to-end-encrypted link. Guests only see what you choose to share, and nothing is stored on our side.' },
  { q: 'What platforms are supported?', a: 'Siegu runs on Windows, macOS and Linux, with a full web experience for guests.' },
  { q: 'Can I import from Google Photos or iCloud?', a: 'Yes — Siegu imports your existing library, then processes it all locally and privately.' },
  { q: 'Is there a demo I can try?', a: 'Yes! Open the live demo to see Siegu in action right in your browser — no install or account needed.' },
];

function faqSchemaFor() {
  const mainEntity = FAQPAGE_SCHEMA.map((it) => `{
          "@type": "Question",
          "name": ${JSON.stringify(it.q)},
          "acceptedAnswer": {
            "@type": "Answer",
            "text": ${JSON.stringify(it.a)}
          }
        }`).join(',\n');
  return `{
        "@type": "WebPage",
        "@id": "${BASE}/faq.html",
        "url": "${BASE}/faq.html",
        "name": "Siegu FAQ — privacy, offline, sharing, and platforms",
        "isPartOf": { "@id": "${BASE}/#website" },
        "inLanguage": "en",
        "publisher": { "@id": "${BASE}/#organization" }
      },
      {
        "@type": "FAQPage",
        "mainEntity": [
${mainEntity}
        ]
      },
      ${SITE_SCHEMA_ORG}`;
}

/* ---------- Page configs ---------- */

const PAGES = [
  {
    file: 'pricing.html',
    active: 'pricing.html',
    title: 'Siegu pricing — Free, Pro, and Family plans',
    description: 'Simple, honest pricing for Siegu. The full app is free and local-first; Pro and Family add instant sharing, more devices, and early features. No cloud, no uploads.',
    keywords: 'siegu pricing, siegu pro, siegu family, photo library pricing, local-first photo app price, private photo app plans',
    url: `${BASE}/pricing.html`,
    schema: `{
        "@type": "WebPage",
        "@id": "${BASE}/pricing.html",
        "url": "${BASE}/pricing.html",
        "name": "Siegu pricing — Free, Pro, and Family plans",
        "isPartOf": { "@id": "${BASE}/#website" },
        "inLanguage": "en",
        "publisher": { "@id": "${BASE}/#organization" }
      },
      ${SITE_SCHEMA_ORG}`,
    main: PRICING_MAIN,
  },
  {
    file: 'faq.html',
    active: 'faq.html',
    title: 'Siegu FAQ — privacy, offline, sharing, and platforms',
    description: 'Frequently asked questions about Siegu: is my data private, does it work offline, how does sharing work, which platforms are supported, and how to import from Google Photos or iCloud.',
    keywords: 'siegu faq, siegu help, photo app faq, is my photo data private, offline photo library, import google photos',
    url: `${BASE}/faq.html`,
    schema: faqSchemaFor(),
    main: FAQ_MAIN,
  },
  {
    file: 'download.html',
    active: 'download.html',
    title: 'Download Siegu — free, private, local-first photo library',
    description: 'Download Siegu for free on Windows, macOS, Linux, and Android. Your photos stay on your device with on-device AI search. No account, no cloud, no uploads.',
    keywords: 'download siegu, siegu download, download photo library, private photo app download, open source photo manager',
    url: `${BASE}/download.html`,
    schema: `{
        "@type": "WebPage",
        "@id": "${BASE}/download.html",
        "url": "${BASE}/download.html",
        "name": "Download Siegu — free, private, local-first photo library",
        "isPartOf": { "@id": "${BASE}/#website" },
        "inLanguage": "en",
        "publisher": { "@id": "${BASE}/#organization" }
      },
      ${SITE_SCHEMA_ORG}`,
    main: DOWNLOAD_MAIN,
  },
  {
    file: 'docs.html',
    active: 'docs.html',
    title: 'Siegu documentation — getting started, guides, and support',
    description: 'Documentation and guides for Siegu, the private local-first photo library. Learn how to get started, organize your library, and use on-device AI search.',
    keywords: 'siegu docs, siegu documentation, photo library guide, local photo app help, siegu support',
    url: `${BASE}/docs.html`,
    schema: `{
        "@type": "WebPage",
        "@id": "${BASE}/docs.html",
        "url": "${BASE}/docs.html",
        "name": "Siegu documentation — getting started, guides, and support",
        "isPartOf": { "@id": "${BASE}/#website" },
        "inLanguage": "en",
        "publisher": { "@id": "${BASE}/#organization" }
      },
      ${SITE_SCHEMA_ORG}`,
    main: DOCS_MAIN,
  },
  {
    file: 'compare.html',
    active: 'compare.html',
    title: 'Siegu vs Google Photos — a private, local-first alternative',
    description: 'Google Photos scans and stores your memories on Google servers. Siegu keeps your photos and the AI that organizes them on your device — private, offline-capable, and open source.',
    keywords: 'google photos alternative, private photo library, local-first photo app, stop using google photos, offline photo manager',
    url: `${BASE}/compare.html`,
    schema: `{
        "@type": "WebPage",
        "@id": "${BASE}/compare.html",
        "url": "${BASE}/compare.html",
        "name": "Siegu vs Google Photos — a private, local-first alternative",
        "description": "Google Photos stores your memories on Google servers. Siegu keeps your photos and the AI that organizes them on your device.",
        "isPartOf": { "@id": "${BASE}/#website" },
        "inLanguage": "en",
        "publisher": { "@id": "${BASE}/#organization" }
      },
      ${SITE_SCHEMA_ORG}`,
    main: COMPARE_MAIN,
  },
];

async function main() {
  for (const p of PAGES) {
    const html = headFor(p) + '\n' + bodyTop(p.active) + '\n' + p.main + '\n' + TAIL + '\n';
    await writeFile(join(SRC, p.file), html, 'utf8');
    console.log(`[generate-pages] wrote ${p.file}`);
  }
}

main().catch((e) => { console.error(e); process.exit(1); });