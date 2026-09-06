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
import { buildDocsMain } from './docs-from-md.mjs';

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

function headFor({ title, description, keywords, url, ogTitle, ogDesc, schema, feedUrl }) {
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
  <meta property="og:image" content="${BASE}/social-card.jpg" />
  <meta property="og:image:width" content="1200" />
  <meta property="og:image:height" content="630" />
  <meta name="twitter:card" content="summary_large_image" />
  <meta name="twitter:title" content="${ogTitle || title}" />
  <meta name="twitter:description" content="${ogDesc || description}" />
  <meta name="twitter:image" content="${BASE}/social-card.jpg" />

  <!-- SEO -->
  <link rel="canonical" href="${url}" />
  ${feedUrl ? `<link rel="alternate" type="application/atom+xml" title="${title} — Atom feed" href="${feedUrl}" />` : ''}
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
    const active = n.href === activeHref
      ? ' class="is-active" aria-current="page"'
      : n.external ? '' : '';
    const ext = n.external ? ' target="_blank" rel="noopener" data-track="demo_clicked"' : '';
    return `        <a href="${n.href}"${active} data-i18n="${n.key}"${ext}>${n.label}</a>`;
  }).join('\n');
  return `<body>
  <a class="skip" href="#main">Skip to content</a>
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
        <button class="menu-btn" id="menuBtn" type="button" aria-label="Open menu" aria-expanded="false" aria-controls="navOverlay">☰</button>
      </div>
    </div>
  </header>

  <div id="navOverlay" class="nav-overlay" aria-hidden="true">
    <button class="nav-close" type="button" aria-label="Close menu">✕</button>
    <nav class="nav" aria-label="Primary">
${nav}
    </nav>
  </div>

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
          <a href="connect.html" data-i18n="footer.product_links.connect">Connect</a>
          <a href="changelog.html" data-i18n="footer.product_links.changelog">Changelog</a>
          <a href="roadmap.html" data-i18n="footer.product_links.roadmap">Roadmap</a>
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
          <a href="about.html" data-i18n="footer.company_links.about">About</a>
          <a href="https://github.com/denzyldick/siegu" target="_blank" rel="noopener" data-i18n="footer.company_links.github">GitHub</a>
          <a href="blog.html" data-i18n="footer.company_links.blog">Blog</a>
        </div>
      </div>
      <div class="footer-bottom">
        <span>© <span id="year"></span> siegu. <span data-i18n="footer.rights">All rights reserved.</span></span>
        <span class="footer-legal">
          <a href="#" data-cookie-prefs>Cookie preferences</a>
          <a href="privacy.html" data-i18n="footer.legal">Privacy & Terms</a>
        </span>
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
      <p class="pro-founding" id="proFounding" hidden data-founding>
        <a id="proFoundingBtn" target="_blank" rel="noopener">Or get <strong>Lifetime Pro</strong> for $99 &mdash; one-time payment, yours forever.</a>
      </p>
      <p class="dl-note" data-i18n="pro.note">Secure checkout by Stripe.</p>
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
          <span class="tdot">·</span><span class="gh-stars" id="ghStars" aria-hidden="true"></span>
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
        <p class="pricing-note center" data-i18n="pricing.note">Prices in USD. No extra fees for more photos.</p>
        <p class="pricing-note center guarantee" data-i18n="pricing.guarantee">14-day money-back guarantee on every paid plan — no questions asked.</p>
      </div>
    </section>

    <section class="section page-body">
      <div class="container">
        <div class="price-anchor">
          <div>
            <h2 data-i18n="pricing.anchor_title">Google Photos charges by the gigabyte, and it sees whatever you upload.</h2>
            <p class="sub" data-i18n="pricing.anchor_body">Siegu is a flat fee for a private, infinite library that lives on your device. Pay once — upgrades included. Your memories aren&rsquo;t training data.</p>
          </div>
          <a class="btn btn-ghost btn-lg" href="compare.html" data-i18n="pricing.anchor_cta">See the comparison</a>
        </div>
      </div>
    </section>

    <div class="sticky-pro" id="stickyPro" hidden>
      <span data-i18n="pricing.sticky">Save 20% with the annual plan — cancel anytime.</span>
      <a class="btn btn-ink btn-sm" data-action="open-pro" data-track="cta_upgrade" role="button" data-i18n="cta.upgrade">Upgrade to Pro</a>
    </div>
    <p class="founding-note center" id="foundingNote" hidden data-founding>Launch offer: <strong>Lifetime Pro for $99</strong> — one-time, yours forever. <a data-action="open-pro" role="button">Claim it</a></p>`;

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

const CONNECT_MAIN = `
    <section class="page-hero">
      <div class="container">
        <p class="eyebrow">Siegu Connect</p>
        <h1>Sync, share, and keep the family together</h1>
        <p class="sub">Siegu Connect is the hosted relay behind sync &amp; sharing. It helps your devices find each other across the internet &mdash; while your photos and videos never leave your devices.</p>
        <div class="trust-row">
          <span>Open source</span><span class="tdot">&middot;</span>
          <span>End-to-end encrypted</span><span class="tdot">&middot;</span>
          <span>Files never touch the relay</span>
          <span class="tdot">&middot;</span><span class="gh-stars" id="ghStars" aria-hidden="true"></span>
        </div>
      </div>
    </section>

    <section class="section">
      <div class="container center pricing-head">
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
        <p class="pricing-note center" data-i18n="pricing.note">Prices in USD. No extra fees for more photos.</p>
        <p class="pricing-note center guarantee" data-i18n="pricing.guarantee">14-day money-back guarantee on every paid plan — no questions asked.</p>
      </div>
    </section>

    <div class="sticky-pro" id="stickyPro" hidden>
      <span data-i18n="pricing.sticky">Save 20% with the annual plan — cancel anytime.</span>
      <a class="btn btn-ink btn-sm" data-action="open-pro" data-track="cta_upgrade" role="button" data-i18n="cta.upgrade">Upgrade to Pro</a>
    </div>
    <p class="founding-note center" id="foundingNote" hidden data-founding>Launch offer: <strong>Lifetime Pro for $99</strong> — one-time, yours forever. <a data-action="open-pro" role="button">Claim it</a></p>

    <section class="section page-body">
      <div class="container">
        <div class="narrow">
          <h2>What Siegu Connect does</h2>
          <p>Siegu is local-first by design, but your devices still need to meet when they aren&rsquo;t on the same network. Connect provides a minimal relay that introduces your devices to each other with an encrypted WebRTC handshake &mdash; then steps out of the way. Your data travels directly between your devices, end-to-end encrypted.</p>
          <ul>
            <li><strong>Sync between your own devices</strong> over the internet &mdash; phone to PC, on any network.</li>
            <li><strong>Share collections</strong> with anyone through the view-only web client.</li>
            <li><strong>Hosted by the Siegu team</strong> &mdash; no ports to open, no servers to run.</li>
            <li><strong>Included with Pro and Family</strong> &mdash; no separate subscription.</li>
          </ul>
          <p class="docs-more">The relay only ever sees encrypted connection details &mdash; never your files, metadata, or manifests. See <a href="docs.html#security-privacy">the security model</a> and the <a href="privacy.html">privacy policy</a>.</p>
          <p>Not sure where to start? <a href="download.html">Download Siegu for free</a> &mdash; the app is free, local, and fully yours; upgrade any time.</p>
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

const ABOUT_MAIN = `
    <section class="page-hero">
      <div class="container">
        <p class="eyebrow" data-i18n="about.eyebrow">About</p>
        <h1 data-i18n="about.title">A private photo library, for good</h1>
        <p class="sub">Siegu keeps your photos and videos on your device — and nowhere else.</p>
      </div>
    </section>

    <section class="section page-body">
      <div class="container">
        <div class="narrow">
          <p>Siegu is a privacy-first, local-only media manager. It organizes your library entirely on your machine: automatic scanning, EXIF extraction, map heatmaps, on-device AI search, face grouping, and encrypted peer-to-peer sync between devices you own.</p>
          <p>No accounts. No uploads. No telemetry. Every AI feature runs locally via ONNX Runtime &mdash; the same capability, without sending a single photo elsewhere to get it.</p>

          <h2>Why we're building it</h2>
          <p>Modern photo services charge nothing up front and collect everything in exchange: your library, your faces, your location history. We believe a photo library is personal by definition. Siegu flips the model &mdash; the software runs where your photos already live, and &ldquo;free&rdquo; means free to use, never free in exchange for your data.</p>

          <h2>Who's behind it</h2>
          <p>Siegu is built by <a href="https://github.com/denzyldick" target="_blank" rel="noopener">Denzyl Dick</a>, in the open. The entire codebase is on GitHub and the roadmap is public. Because it's open source, you can read the code, audit what it does with your data, and run it exactly the way you want. There is no service to cancel.</p>

          <p>Curious how it's made? <a href="https://github.com/denzyldick/siegu" target="_blank" rel="noopener">Browse the source on GitHub &rarr;</a></p>
        </div>
      </div>
    </section>`;

const CHANGELOG_MAIN = `
    <section class="page-hero">
      <div class="container">
        <p class="eyebrow">Changelog</p>
        <h1>What's new in Siegu</h1>
        <p class="sub">Every release, in the open. Tags and full history live <a href="https://github.com/denzyldick/siegu/releases" target="_blank" rel="noopener" class="is-link">on GitHub</a>.</p>
      </div>
    </section>

    <section class="section page-body">
      <div class="container">
        <div class="narrow">
          <h2>v0.1.17 &mdash; Latest</h2>
          <ul>
            <li>Seamless live-mirror: changes sync between your own devices in near real time.</li>
            <li>Better duplicate detection during import.</li>
            <li>Reliability pass: fsync-on-write, capped manifests, backpressure, and an edge-to-edge gallery.</li>
            <li>Collection sharing: unblocked host, scoped guest access, and correct URLs.</li>
            <li>Fixed order-dependent face-grouping splits during bulk analysis.</li>
          </ul>

          <h2>All versions</h2>
          <ul class="changelog-list">
            <li><a href="https://github.com/denzyldick/siegu/commits/v0.1.16">v0.1.16</a></li>
            <li><a href="https://github.com/denzyldick/siegu/commits/v0.1.15">v0.1.15</a></li>
            <li><a href="https://github.com/denzyldick/siegu/commits/v0.1.14">v0.1.14</a></li>
            <li><a href="https://github.com/denzyldick/siegu/commits/v0.1.13">v0.1.13</a></li>
            <li><a href="https://github.com/denzyldick/siegu/commits/v0.1.12">v0.1.12</a></li>
            <li><a href="https://github.com/denzyldick/siegu/commits/v0.1.11">v0.1.11</a></li>
            <li><a href="https://github.com/denzyldick/siegu/commits/v0.1.10-e2842a03f">v0.1.10</a></li>
            <li><a href="https://github.com/denzyldick/siegu/commits/v0.1.9-8dad9c0ea">v0.1.9</a></li>
            <li><a href="https://github.com/denzyldick/siegu/commits/v0.1.8">v0.1.8</a></li>
            <li><a href="https://github.com/denzyldick/siegu/commits/v0.1.7">v0.1.7</a></li>
            <li><a href="https://github.com/denzyldick/siegu/commits/v0.1.6">v0.1.6</a></li>
            <li><a href="https://github.com/denzyldick/siegu/commits/v0.1.5">v0.1.5</a></li>
            <li><a href="https://github.com/denzyldick/siegu/commits/v0.1.4">v0.1.4</a></li>
            <li><a href="https://github.com/denzyldick/siegu/commits/v0.1.3">v0.1.3</a></li>
            <li><a href="https://github.com/denzyldick/siegu/commits/v0.1.2">v0.1.2</a></li>
            <li><a href="https://github.com/denzyldick/siegu/commits/v0.1.1">v0.1.1</a></li>
          </ul>
        </div>
      </div>
    </section>`;

const ROADMAP_MAIN = `
    <section class="page-hero">
      <div class="container">
        <p class="eyebrow">Roadmap</p>
        <h1>Where Siegu is headed</h1>
        <p class="sub">A living list. Priorities shift with feedback &mdash; if something's missing, say so on GitHub.</p>
      </div>
    </section>

    <section class="section page-body">
      <div class="container">
        <div class="narrow">
          <h2>Now</h2>
          <ul>
            <li>Family sharing with end-to-end encryption between accounts you trust.</li>
            <li>Mobile apps, starting with Android (already in the downloads).</li>
            <li>Video improvements: trimming, smart previews, and richer metadata.</li>
          </ul>

          <h2>Next</h2>
          <ul>
            <li>Opt-in cloud sync for people who want off-device redundancy &mdash; end-to-end encrypted, never an upload of your library.</li>
            <li>More on-device AI: natural-language queries across your whole library.</li>
            <li>Set-by-set sharing with expiry and access control.</li>
          </ul>

          <h2>Later</h2>
          <ul>
            <li>iOS support.</li>
            <li>A plugin system for custom pipelines and exporters.</li>
          </ul>

          <h2>Done</h2>
          <ul>
            <li>Semantic search with CLIP, running on your own hardware.</li>
            <li>Face recognition, people groups, rename, and merge.</li>
            <li>On-device captions (BLIP), objects (YOLO), OCR, aesthetics, depth maps, and Whisper transcription.</li>
            <li>Encrypted peer-to-peer sync over WebRTC with mesh networking.</li>
          </ul>

          <p>Track progress and weigh in: <a href="https://github.com/denzyldick/siegu/issues" target="_blank" rel="noopener">GitHub issues &rarr;</a></p>
        </div>
      </div>
    </section>`;

const BLOG_POSTS = [
  {
    id: 'private-by-design',
    date: '2026-09-01',
    label: 'September 2026',
    title: 'Why &ldquo;private by design&rdquo; beats a privacy policy',
    body: `
            <p>Every photo service has a privacy policy. And every one of them <em>can</em> upload your photos. A policy just describes what a company may legally do with your data &mdash; it doesn't prevent anything.</p>
            <p>Private by design means there is nothing to prevent, because there's nothing to take. Siegu runs its whole pipeline &mdash; indexing, search, face recognition, captions &mdash; on your own hardware. The &ldquo;cloud&rdquo; isn't where features live; it's what we avoid. That's a small difference in marketing and a huge difference in where your memories end up.</p>`,
  },
  {
    id: 'search-engine-for-one-person',
    date: '2026-08-01',
    label: 'August 2026',
    title: 'How a search engine for one person works',
    body: `
            <p>Semantic search usually means uploading your library so a server can index it. Siegu does the same job with CLIP running locally through ONNX Runtime: describe a photo (&ldquo;sunsets at the beach&rdquo;) and it's found on your drive, not a data center.</p>
            <p>Local AI is slower per query than a warehouse of GPUs, so the app indexes in the background and answers from a pre-computed index. You get the magic without shipping your memories anywhere.</p>`,
  },
];

const BLOG_MAIN = `
    <section class="page-hero">
      <div class="container">
        <p class="eyebrow">Blog</p>
        <h1>Notes from the Siegu project</h1>
        <p class="sub">What it's like building a private, local-first photo library &mdash; from the person building it.</p>
      </div>
    </section>

    <section class="section page-body">
      <div class="container">
        <div class="narrow">
          ${BLOG_POSTS.map((b) => `
          <article class="blog-post" id="${b.id}">
            <p class="blog-date">${b.label}</p>
            <h2>${b.title}</h2>
            ${b.body}
          </article>`).join('\n')}
        </div>
      </div>
    </section>`;

function feedText(html) {
  const entities = {
    '&ldquo;': '\u201C', '&rdquo;': '\u201D', '&lsquo;': '\u2018', '&rsquo;': '\u2019',
    '&mdash;': '\u2014', '&ndash;': '\u2013', '&amp;': '&', '&lt;': '<', '&gt;': '>',
  };
  let s = html.replace(/<[^>]+>/g, ' ').replace(/\s+/g, ' ').trim();
  s = s.replace(/&(ldquo|rdquo|lsquo|rsquo|mdash|ndash|amp|lt|gt);/g, (m) => entities[m]);
  return s.replace(/[&<>]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;' }[c]));
}

function buildBlogFeed({ url, title, description }) {
  const updated = BLOG_POSTS[0] ? BLOG_POSTS[0].date : null;
  const entries = BLOG_POSTS.map((b) => {
    const postUrl = `${url}#${b.id}`;
    return `<entry>
    <title>${feedText(b.title)}</title>
    <link href="${postUrl}" rel="alternate"/>
    <id>${postUrl}</id>
    <updated>${b.date}T00:00:00Z</updated>
    <published>${b.date}T00:00:00Z</published>
    <summary>${feedText(b.body)}</summary>
  </entry>`;
  }).join('\n  ');
  return `<?xml version="1.0" encoding="utf-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <title>${feedText(title)}</title>
  <link href="${url}" rel="alternate"/>
  <link href="${url.replace(/blog\.html$/, 'feed.xml')}" rel="self" type="application/atom+xml"/>
  <id>${BASE}/</id>
  <updated>${updated || new Date().toISOString().slice(0, 10)}T00:00:00Z</updated>
  <subtitle>${feedText(description)}</subtitle>
  <author><name>Siegu</name></author>
  ${entries}
</feed>
`;
}

const PRIVACY_MAIN = `
    <section class="page-hero">
      <div class="container">
        <p class="eyebrow">Legal</p>
        <h1>Privacy Policy &amp; Terms</h1>
        <p class="sub">Short version: your photos never leave your device. What follows is the short version, in full.</p>
      </div>
    </section>

    <section class="section page-body">
      <div class="container">
        <div class="narrow">
          <h2 class="privacy-h">Privacy</h2>

          <h3>Your media is local</h3>
          <p>Siegu stores your photos, videos, and metadata only on your device. Nothing is uploaded for storage or processing. All AI features run locally via ONNX Runtime. This is a property of the software, not a promise.</p>

          <h3>No accounts</h3>
          <p>Siegu does not create user accounts and has no cloud backend to sign into. Sharing works peer-to-peer over encrypted, live connections between the devices you choose.</p>

          <h3>Sync and the signalling server</h3>
          <p>To reach devices that aren't on the same network, Siegu can use a signalling server (the default is <code>wss://siegu.io/ws</code>, and you can self-host your own or point the app at any URL). Its only job is to introduce two of your devices to each other: it relays the brief WebRTC offer/answer/ICE handshake &mdash; just connection details, nothing else. Once the peer-to-peer link is established, your photos and videos travel directly between your devices over an end-to-end encrypted channel, and the signalling server is out of the data path. It never sees file contents, metadata, or manifests. On the same LAN, Siegu uses a built-in server embedded in the app instead, so no external server is involved at all.</p>

          <h3>This website</h3>
          <p>This website uses Google Analytics and Microsoft Clarity &mdash; but only after you accept the consent banner. Google Analytics counts which pages are useful and roughly where visitors come from, as aggregate data. Microsoft Clarity records heatmaps and anonymized session replays so we can spot where the page is confusing. Both run over HTTPS, never touch photo content, and nothing at all is loaded if you decline or make no choice. You can change your mind any time via the &ldquo;cookie preferences&rdquo; link in the footer.</p>

          <h3>Waitlist emails</h3>
          <p>If you join the waitlist via the family-plan form, the address you type is sent to Formspree (our form processor) and used solely to notify you when it launches. You can ask to be removed at any time by replying to any email.</p>

          <h3>Purchases</h3>
          <p>Pro payments are handled by Stripe. Card details go directly to them over TLS; Siegu never sees or stores them.</p>

          <h3>Telemetry in the app</h3>
          <p>The Siegu app sends no telemetry, analytics, or crash reports. There is no &ldquo;home call&rdquo; &mdash; by design.</p>

          <h2 class="privacy-h">Terms</h2>

          <h3>License</h3>
          <p>Siegu is open source and distributed under an open license. You may use it for any lawful purpose and modify it under the terms of that license. See the <a href="https://github.com/denzyldick/siegu" target="_blank" rel="noopener">repository</a> for the exact text.</p>

          <h3>No warranty</h3>
          <p>Siegu is provided &ldquo;as is&rdquo;, without warranty of any kind. You are responsible for backing up your own library (which, being local, is entirely in your hands).</p>

          <h3>Not medical / not legal advice</h3>
          <p>Projections, plans, and estimates on this site are aspirational and may change. Nothing here constitutes professional advice.</p>

          <h3>Contact</h3>
          <p>Questions about this policy? Open an issue on <a href="https://github.com/denzyldick/siegu" target="_blank" rel="noopener">GitHub</a> and we'll respond publicly &mdash; the most transparent inbox there is.</p>
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
    title: 'Siegu docs — getting started, sync, sharing, and configuration',
    description: 'Siegu documentation: get started with the private local-first photo library, sync and share collections, configure the app, and understand its security model.',
    keywords: 'siegu docs, siegu documentation, siegu sync, siegu sharing, siegu configuration, siegu security, photo library guide, local photo app help, siegu support',
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
    main: '', // filled by buildDocsMain() in main()
  },
  {
    file: 'connect.html',
    active: 'connect.html',
    title: 'Siegu Connect — Pro & Family — private sync and sharing',
    description: 'Buy Siegu Pro or join the Family plan. Siegu Connect syncs your library between devices over the internet and shares collections with anyone — end-to-end encrypted, relay never sees your files.',
    keywords: 'siegu connect, buy siegu pro, siegu pro, siegu family, siegu family plan, private sync, family photo sharing, siegu upgrade, purchase siegu, photo sync, share photos privately, siegu subscription',
    url: `${BASE}/connect.html`,
    schema: `{
        "@type": "WebPage",
        "@id": "${BASE}/connect.html",
        "url": "${BASE}/connect.html",
        "name": "Siegu Connect — Pro & Family — private sync and sharing",
        "description": "Buy Siegu Pro or join the Family plan. Siegu Connect syncs and shares your library — end-to-end encrypted, with the relay never seeing your files.",
        "isPartOf": { "@id": "${BASE}/#website" },
        "inLanguage": "en",
        "publisher": { "@id": "${BASE}/#organization" }
      },
      ${SITE_SCHEMA_ORG}`,
    main: CONNECT_MAIN,
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
  {
    file: 'about.html',
    active: 'about.html',
    title: 'About Siegu — the private, local-first photo library',
    description: 'Siegu is a privacy-first, local-only media manager. No accounts, no uploads, no telemetry — your photos and the AI that organizes them live on your device.',
    keywords: 'about siegu, siegu team, private photo library, open source photo app, local-first software',
    url: `${BASE}/about.html`,
    schema: `{
        "@type": "WebPage",
        "@id": "${BASE}/about.html",
        "url": "${BASE}/about.html",
        "name": "About Siegu — the private, local-first photo library",
        "isPartOf": { "@id": "${BASE}/#website" },
        "inLanguage": "en",
        "publisher": { "@id": "${BASE}/#organization" }
      },
      ${SITE_SCHEMA_ORG}`,
    main: ABOUT_MAIN,
  },
  {
    file: 'blog.html',
    active: 'blog.html',
    title: 'Siegu blog — notes from building a private photo library',
    description: 'Thoughts on building a private, local-first photo library: why private by design beats a privacy policy, how on-device AI works, and more from the Siegu project.',
    keywords: 'siegu blog, private photo library, local AI, on-device machine learning, open source photo app',
    url: `${BASE}/blog.html`,
    feedUrl: `${BASE}/feed.xml`,
    schema: `{
        "@type": "WebPage",
        "@id": "${BASE}/blog.html",
        "url": "${BASE}/blog.html",
        "name": "Siegu blog — notes from building a private photo library",
        "isPartOf": { "@id": "${BASE}/#website" },
        "inLanguage": "en",
        "publisher": { "@id": "${BASE}/#organization" }
      },
      ${SITE_SCHEMA_ORG}`,
    main: BLOG_MAIN,
  },
  {
    file: 'changelog.html',
    active: 'changelog.html',
    title: 'Siegu changelog — what\u2019s new in every release',
    description: 'The changelog for Siegu, the private local-first photo library. Latest release highlights and the complete version history, in the open.',
    keywords: 'siegu changelog, siegu release notes, siegu updates, photo library changelog, siegu versions',
    url: `${BASE}/changelog.html`,
    schema: `{
        "@type": "WebPage",
        "@id": "${BASE}/changelog.html",
        "url": "${BASE}/changelog.html",
        "name": "Siegu changelog — what\u2019s new in every release",
        "isPartOf": { "@id": "${BASE}/#website" },
        "inLanguage": "en",
        "publisher": { "@id": "${BASE}/#organization" }
      },
      ${SITE_SCHEMA_ORG}`,
    main: CHANGELOG_MAIN,
  },
  {
    file: 'roadmap.html',
    active: 'roadmap.html',
    title: 'Siegu roadmap — what\u2019s next for the private photo library',
    description: 'The public roadmap for Siegu: family sharing, mobile apps, opt-in cloud sync, and more on-device AI. Progress tracked in the open on GitHub.',
    keywords: 'siegu roadmap, siegu roadmap plan, photo library features, private photo app roadmap, on-device AI roadmap',
    url: `${BASE}/roadmap.html`,
    schema: `{
        "@type": "WebPage",
        "@id": "${BASE}/roadmap.html",
        "url": "${BASE}/roadmap.html",
        "name": "Siegu roadmap — what\u2019s next for the private photo library",
        "isPartOf": { "@id": "${BASE}/#website" },
        "inLanguage": "en",
        "publisher": { "@id": "${BASE}/#organization" }
      },
      ${SITE_SCHEMA_ORG}`,
    main: ROADMAP_MAIN,
  },
  {
    file: 'privacy.html',
    active: 'privacy.html',
    title: 'Siegu Privacy Policy & Terms — your photos stay on your device',
    description: 'Siegu privacy policy and terms of use: your photos never leave your device, no accounts, no telemetry, open source. This website uses consent-gated analytics and Formspree for emails.',
    keywords: 'siegu privacy, siegu terms, photo app privacy policy, local-first privacy, siegu legal',
    url: `${BASE}/privacy.html`,
    schema: `{
        "@type": "WebPage",
        "@id": "${BASE}/privacy.html",
        "url": "${BASE}/privacy.html",
        "name": "Siegu Privacy Policy & Terms — your photos stay on your device",
        "isPartOf": { "@id": "${BASE}/#website" },
        "inLanguage": "en",
        "publisher": { "@id": "${BASE}/#organization" }
      },
      ${SITE_SCHEMA_ORG}`,
    main: PRIVACY_MAIN,
  },
];

async function main() {
  /* Docs page: build from the app repo's real markdown when available. */
  const docs = buildDocsMain();
  const docsPage = PAGES.find((p) => p.file === 'docs.html');
  docsPage.main = docs.main;

  for (const p of PAGES) {
    const html = headFor(p) + '\n' + bodyTop(p.active) + '\n' + p.main + '\n' + TAIL + '\n';
    await writeFile(join(SRC, p.file), html, 'utf8');
    console.log(`[generate-pages] wrote ${p.file}`);
  }

  /* Atom feed for the blog. */
  const blogPage = PAGES.find((p) => p.file === 'blog.html');
  await writeFile(
    join(SRC, 'feed.xml'),
    buildBlogFeed({ url: blogPage.url, title: blogPage.title, description: blogPage.description }),
    'utf8'
  );
  console.log('[generate-pages] wrote feed.xml');

  /* Keep the sitemap's docs lastmod in sync with the app's doc commits. */
  if (docs.ok) {
    const sitemapPath = join(SRC, 'sitemap.xml');
    const sitemap = await readFile(sitemapPath, 'utf8');
    const from = /(<url>\s*<loc>https:\/\/denzyldick\.github\.io\/siegu\/docs\.html<\/loc>\s*<lastmod>)[^<]+/;
    if (from.test(sitemap)) {
      await writeFile(sitemapPath, sitemap.replace(from, `$1${docs.lastMod}`), 'utf8');
      console.log(`[generate-pages] sitemap docs.html lastmod → ${docs.lastMod}`);
    }
  }
}

main().catch((e) => { console.error(e); process.exit(1); });