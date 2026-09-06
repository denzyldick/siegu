/* Siegu landing — i18n, pricing, FAQ, theme, trackers. No framework, no build. */
'use strict';

// Resolve the site's base directory (with trailing slash) so the page works
// when served from a subpath (GitHub Pages at /siegu/) or from the root
// (Caddy as siegu.io). index.html always lives at the served root, so the
// current page's directory is the base.
const BASE_URL = new URL('.', window.location.href).href;

const SUPPORTED = ['en', 'nl', 'fr', 'es', 'pap', 'de', 'it', 'pt'];
const FALLBACK = 'en';

const state = {
  locale: FALLBACK,
  dict: null,
  billing: 'yearly',
  trackersOn: false,
};

/* ---------- i18n ---------- */
async function loadLocale(code) {
  try {
    const res = await fetch(`${BASE_URL}locales/${code}.json`);
    if (!res.ok) throw new Error(res.status);
    return await res.json();
  } catch {
    const fb = await fetch(`${BASE_URL}locales/${FALLBACK}.json`);
    return await fb.json();
  }
}

function t(key) {
  const parts = key.split('.');
  let node = state.dict;
  for (const p of parts) {
    if (node == null) return key;
    node = node[p];
  }
  return typeof node === 'string' ? node : key;
}

function nlLookup(parts, node) {
  for (const p of parts) {
    if (node == null) return undefined;
    node = node[p];
  }
  return node;
}

function lookup(key, dict) {
  return nlLookup(key.split('.'), dict);
}

function applyTexts() {
  document.documentElement.lang = state.locale;
  document.querySelectorAll('[data-i18n]').forEach((el) => {
    const v = t(el.getAttribute('data-i18n'));
    if (v !== el.getAttribute('data-i18n')) el.textContent = v;
  });
  document.querySelectorAll('[data-i18n-placeholder]').forEach((el) => {
    const v = t(el.getAttribute('data-i18n-placeholder'));
    if (v !== el.getAttribute('data-i18n-placeholder')) el.setAttribute('placeholder', v);
  });
  document.querySelectorAll('[data-i18n-aria]').forEach((el) => {
    const v = t(el.getAttribute('data-i18n-aria'));
    if (v !== el.getAttribute('data-i18n-aria')) el.setAttribute('aria-label', v);
  });
  document.title = t('meta.title');
  const meta = document.querySelector('meta[name="description"]');
  if (meta) meta.setAttribute('content', t('meta.description'));
  renderLangMenu();
  renderPricing();
  renderFaq();
}

/* ---------- Language menu ---------- */
function renderLangMenu() {
  const names = {
    en: 'English', nl: 'Nederlands', fr: 'Français', es: 'Español',
    pap: 'Papiamentu', de: 'Deutsch', it: 'Italiano', pt: 'Português',
  };
  const langCode = document.getElementById('langCode');
  if (langCode) langCode.textContent = state.locale.toUpperCase();
  const menu = document.getElementById('langMenu');
  if (!menu) return;
  menu.innerHTML = SUPPORTED
    .map((c) => {
      const active = c === state.locale;
      return `<button type="button" class="${active ? 'active' : ''}" data-lang="${c}">
        <span>${names[c] || c}</span>
        ${active ? '<span class="check">✓</span>' : ''}
      </button>`;
    })
    .join('');
}

function toggleLangMenu(force) {
  const menu = document.getElementById('langMenu');
  menu.classList.toggle('open', typeof force === 'boolean' ? force : !menu.classList.contains('open'));
}

/* ---------- Pricing ---------- */
function priceFor(plan, price) {
  if (plan === 'free') return '0';
  return state.billing === 'yearly' ? (price * 0.8).toFixed(2) : price.toFixed(2);
}

function renderPricing() {
  const d = state.dict || {};
  const plans = ['free', 'team', 'pro'];
  const grid = document.getElementById('pricingGrid');
  if (!grid) return;
  let html = plans
    .map((key) => {
      const p = lookup('pricing.' + key, d) || {};
      const featured = key === 'pro';
      const isYearly = state.billing === 'yearly';
      const price = { free: 0, pro: 9.99, team: 29.99 }[key];
      const feats = Array.isArray(p.features) ? p.features : [];
      const isWaitlist = key === 'team';
      // Waitlist (Team) is a secondary action; Free/Pro are primary.
      const btnClass = isWaitlist ? 'btn-ghost' : 'btn-ink';
      // Free -> download dialog; Pro -> pro dialog (Stripe inside); Team -> waitlist.
      const btnAction = isWaitlist ? 'open-waitlist'
        : key === 'free' ? 'open-download'
        : 'open-pro';
      const btnHref = '#';
      const btnTarget = 'target="_blank" rel="noopener"';
      const btnTrack = isWaitlist ? 'data-track="pricing_waitlist"' : `data-track="pricing_${key}"`;
      const btnExtra = isWaitlist ? `data-waitlist-source="family"` : '';
      // Keep the Free card's data-platform stamp for GA download breakdowns.
      let dataPlatform = '';
      if (key === 'free') {
        const pl = detectPlatform();
        if (latestAssets.length > 0 && pl.os !== 'web' && pl.os !== 'ios') {
          dataPlatform = ` data-platform="${pl.os}" data-arch="${pl.arch}"`;
        }
      }
      return `
      <div class="plan ${featured ? 'featured' : ''}">
        ${featured ? `<span class="plan-badge">${p.highlight || ''}</span>` : ''}
        <p class="plan-name">${p.name || key}</p>
        <div class="plan-price"><span class="cur">$</span>${priceFor(key, price)}</div>
        <p class="period">${p.period || ''}</p>
        ${isYearly && key !== 'free' ? `<p class="yearly-badge">${p.yearly_badge || ''}</p>` : ''}
        <p class="tagline">${p.tagline || ''}</p>
        <ul>${feats.map((f) => `<li><span class="check">✓</span><span>${f}</span></li>`).join('')}</ul>
        ${key !== 'free' ? '<p class="plan-risk">Cancel anytime</p>' : ''}
        <a class="btn ${btnClass}" href="${btnHref}" data-action="${btnAction}" ${btnTarget} ${btnExtra} ${dataPlatform} ${btnTrack}>${p.cta || ''}</a>
      </div>`;
    })
    .join('');
  grid.innerHTML = html;
}

/* ---------- FAQ ---------- */
function renderFaq() {
  const items = lookup('faq.items', state.dict) || [];
  const list = document.getElementById('faqList');
  if (!list) return;
  list.innerHTML = items
    .map((it, i) => `
      <div class="faq-item" id="faq-${i}">
        <button type="button" class="faq-q" data-faq="${i}">
          <span>${it.q || ''}</span><span class="chev">▾</span>
        </button>
        <div class="faq-a"><p>${it.a || ''}</p></div>
      </div>`)
    .join('');
}

/* ---------- Theme ---------- */
const THEME_KEY = 'siegu_theme';
const THEME_MODES = ['light', 'dark', 'system'];

function storedTheme() {
  const v = localStorage.getItem(THEME_KEY);
  return THEME_MODES.includes(v) ? v : 'system';
}

function applyTheme() {
  const mode = storedTheme();
  const dark =
    mode === 'dark' ||
    (mode === 'system' && window.matchMedia('(prefers-color-scheme: dark)').matches);
  const root = document.documentElement;
  root.setAttribute('data-theme', dark ? 'dark' : 'light');
  root.classList.toggle('is-dark', dark);
  const btn = document.getElementById('themeBtn');
  if (btn) {
    btn.setAttribute('aria-label', `Theme: ${dark ? 'dark' : 'light'} (click to cycle)`);
    btn.textContent = dark ? '☀︎' : '☾';
  }
}

function cycleTheme() {
  const isDark = document.documentElement.getAttribute('data-theme') === 'dark';
  localStorage.setItem(THEME_KEY, isDark ? 'light' : 'dark');
  applyTheme();
}

/* ---------- Trackers / analytics ----------
   Called after the (optional) consent banner. Vendor snippets load here.

   GA4: the Docker build injects the real Measurement ID into GA_MEASUREMENT_ID
   (see deploy/Caddyfile + README). When the ID is still the public placeholder
   or absent, GA stays off — including under plain `npm run dev`. */
const GA_MEASUREMENT_ID = 'G-Z1QJYVPR46';
const GA_ENABLED = /^G-[A-Z0-9]+$/.test(GA_MEASUREMENT_ID);
const CLARITY_PROJECT_ID = 'ye3gmjqs0g';
const CLARITY_ENABLED = /^[a-z0-9]+$/i.test(CLARITY_PROJECT_ID);

function trackersOn() {
  state.trackersOn = true;
  if (GA_ENABLED) {
    const s = document.createElement('script');
    s.async = true;
    s.src = `https://www.googletagmanager.com/gtag/js?id=${GA_MEASUREMENT_ID}`;
    document.head.appendChild(s);
    window.dataLayer = window.dataLayer || [];
    window.gtag = function () {
      window.dataLayer.push(arguments);
    };
    window.gtag('js', new Date());
    window.gtag('config', GA_MEASUREMENT_ID);
  }
  if (CLARITY_ENABLED) {
    (function (c, l, a, r, i, t, y) {
      c[a] = c[a] || function () { (c[a].q = c[a].q || []).push(arguments); };
      t = l.createElement(r);
      t.async = 1;
      t.src = 'https://www.clarity.ms/tag/' + i;
      y = l.getElementsByTagName(r)[0];
      y.parentNode.insertBefore(t, y);
    })(window, document, 'clarity', 'script', CLARITY_PROJECT_ID);
  }
}

/* ---------- Analytics consent ----------
   Siegu promises no telemetry — and this website honors that too. Google
   Analytics only loads after the visitor explicitly accepts the consent
   banner (choice persisted in localStorage). Declining — or ignoring it —
   means no analytics script ever loads, no cookies, nothing sent. The
   dataLayer wiring stays inert until consent is granted. */
const CONSENT_KEY = 'siegu_consent';

function readConsent() {
  try {
    return localStorage.getItem(CONSENT_KEY);
  } catch {
    return null;
  }
}

function applyConsent(value) {
  try {
    localStorage.setItem(CONSENT_KEY, value);
  } catch { /* private mode: choice lasts this session only */ }
  const band = document.getElementById('consentBanner');
  if (band) band.classList.add('is-hidden');
  if (value === 'granted') trackersOn();
}

function ensureConsentBanner() {
  if (document.getElementById('consentBanner') || (!GA_ENABLED && !CLARITY_ENABLED)) return;
  const band = document.createElement('div');
  band.id = 'consentBanner';
  band.setAttribute('role', 'dialog');
  band.setAttribute('aria-label', 'Cookie consent');
  band.classList.add('consent-banner');
  band.innerHTML = [
    '<p class="consent-text">Siegu never tracks your photos — not in the app, and on this site we only use privacy-respecting analytics',
    'if you say yes. We use Google Analytics to count visits and Microsoft Clarity to spot where the page could be more helpful.',
    'No photo content ever touches them. <a href="privacy.html">Privacy&nbsp;policy</a></p>',
    '<div class="consent-actions">',
    '  <button type="button" class="btn-ghost" data-consent="decline">Decline</button>',
    '  <button type="button" class="btn btn-ink" data-consent="accept">Accept</button>',
    '</div>',
  ].join(' ');
  document.body.appendChild(band);
  band.querySelectorAll('[data-consent]').forEach((btn) => {
    btn.addEventListener('click', () => applyConsent(btn.dataset.consent === 'accept' ? 'granted' : 'denied'));
  });
}

function reopenConsent(e) {
  if (!e.target.closest('[data-cookie-prefs]')) return;
  e.preventDefault();
  try {
    localStorage.removeItem(CONSENT_KEY);
  } catch { /* ignore */ }
  if (GA_ENABLED || CLARITY_ENABLED) {
    ensureConsentBanner();
    const band = document.getElementById('consentBanner');
    if (band) band.classList.remove('is-hidden');
  }
}

function initConsent() {
  const band = document.getElementById('consentBanner');
  const choice = readConsent();
  if (choice === 'granted') {
    if (band) band.classList.add('is-hidden');
    trackersOn();
    return;
  }
  if (choice === 'denied') return; /* nothing ever loads */
  ensureConsentBanner(); /* no choice yet — ask */
  document.addEventListener('click', reopenConsent);
}

/* ---------- Pro checkout (Stripe Payment Links) ----------
   Pro is paid. Stripe Payment Links are the merchant of record: a hosted,
   PCI-compliant checkout page Stripe generates — no backend needed on our
   side. A build script (scripts/build-static.mjs) substitutes the real links
   into these placeholders. Monthly and Yearly are separate prices (Yearly is
   ~20% off), so the Pro button hands off to whichever period is selected. */
const STRIPE_PRO_PAYMENT_LINK_MONTHLY = 'https://buy.stripe.com/test_cNiaEX8HIdZc1e7fLL9MY00';
const STRIPE_PRO_PAYMENT_LINK_YEARLY = 'https://buy.stripe.com/test_cNieVd3nocV8cWP2YZ9MY01';
// The currently-selected period is read from state.billing by proPaymentLink().
function proPaymentLink() {
  return state.billing === 'yearly'
    ? STRIPE_PRO_PAYMENT_LINK_YEARLY
    : STRIPE_PRO_PAYMENT_LINK_MONTHLY;
}
// Matches Stripe Payment Link URLs (buy.stripe.com) so an unset placeholder
// stays inert (#).
const STRIPE_PAYMENT_RE = /^https:\/\/(?:buy|checkout)\.stripe\.com\//i;

/* ---------- Free downloads (GitHub Releases) ----------
   The free app is distributed via GitHub Releases, which the release
   pipeline populates automatically. We fetch the latest release and hand
   the visitor the correct installer for their OS/architecture. Nothing is
   hardcoded: assets are selected by matching the release's asset names. */
const RELEASE_API = 'https://api.github.com/repos/denzyldick/siegu/releases/latest';
const RELEASE_PAGE = 'https://github.com/denzyldick/siegu/releases/latest';

function detectPlatform() {
  const ua = navigator.userAgent + ' ' + navigator.platform;
  const isMac = /Mac|iPhone|iPad|iPod/i.test(ua) && !/Windows/i.test(ua);
  const isWin = /Win/i.test(ua);
  const isAndroid = /Android/i.test(ua);
  const isIOS = /iPhone|iPad|iPod/i.test(ua);
  const isLinux = !isMac && !isWin && !isAndroid && !isIOS && /Linux|X11/i.test(ua);
  const os = isAndroid ? 'android' : isIOS ? 'ios' : isMac ? 'macos' : isWin ? 'windows' : isLinux ? 'linux' : 'web';
  const arm = /arm|aarch64/i.test(ua);
  const arch = arm ? 'aarch64' : 'x86_64';
  return { os, arch, ua };
}

// Match a release asset to a platform. Returns the browser_download_url or null.
function selectAsset(assets, os, arch) {
  const isM1 = arch === 'aarch64';
  const byName = (re) => {
    const hit = assets.find((a) => re.test(a.name));
    return hit ? hit.browser_download_url : null;
  };
  if (os === 'windows') {
    return byName(/\.exe$/) || byName(/\.msi$/);
  }
  if (os === 'linux') {
    return byName(/\.AppImage$/) || byName(/\.deb$/);
  }
  if (os === 'android') {
    return byName(/\.apk$/);
  }
  if (os === 'macos') {
    return (isM1 ? byName(/aarch64.*\.dmg$/) : byName(/(amd64|x64).*\.dmg$/)) || byName(/\.dmg$/);
  }
  return null;
}

let latestAssets = [];
let latestFailed = false;

async function loadLatestRelease() {
  try {
    const res = await fetch(RELEASE_API, { headers: { Accept: 'application/vnd.github+json' } });
    if (!res.ok) throw new Error(res.status);
    const rel = await res.json();
    latestAssets = Array.isArray(rel.assets) ? rel.assets : [];
  } catch {
    latestFailed = true;
    latestAssets = [];
  }
}

// Resolve the download URL for a specific OS/arch against the release assets.
function downloadUrlFor(os, arch) {
  if (!latestAssets.length) return RELEASE_PAGE;
  return selectAsset(latestAssets, os, arch) || RELEASE_PAGE;
}

/* ---------- Download dialog (modern OS picker) ---------- */
// Inline SVG platform icons (stroke-based, currentColor) — no icon font needed.
const PICO = {
  windows:
    '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.4"><rect x="3" y="3" width="8" height="8" rx="1"/><rect x="13" y="3" width="8" height="8" rx="1"/><rect x="3" y="13" width="8" height="8" rx="1"/><rect x="13" y="13" width="8" height="8" rx="1"/></svg>',
  macos:
    '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.4"><path d="M12 5c-1.5-1.7-3.4-2-5-1.8a6.5 6.5 0 0 0 1.4 12.9c.7 0 1.6-.5 2.8-.5s2 .6 2.8.6c1.8 0 4-2.5 4-5 .9-1.8-.3-3.6-2-4-.3-1.1-1.8-2.6-4-2.2z"/><path d="M9.5 3.2c-.2-1.3.9-2.6 2.3-2.9"/></svg>',
  linux:
    '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.4"><path d="M12 3c-3 0-5 2.4-5 5.5 0 2 .6 3.6 1.3 5.4.5 1.2 1 2.3 1.2 3.6.2 1.7.9 3.5 2.5 3.5s2.3-1.8 2.5-3.5c.2-1.3.7-2.4 1.2-3.6.7-1.8 1.3-3.4 1.3-5.4C17 5.4 15 3 12 3z"/><path d="M12 3c-1 1.3-1 3.7 0 6.6.7 2 2 4 2.5 5.4.3.9.5 1.8.5 2.5" opacity=".5"/></svg>',
  android:
    '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.4"><path d="M5 9v6M19 9v6"/><rect x="5" y="9" width="14" height="10" rx="2"/><path d="M8 9l-2-3M16 9l2-3M8 19v2M16 19v2"/></svg>',
  web:
    '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.4"><circle cx="12" cy="12" r="9"/><path d="M3 12h18M12 3a15 15 0 0 1 0 18M12 3a15 15 0 0 0 0 18"/></svg>',
};

// The options surfaced in the download dialog, in display order. `osMatch`
// picks the release asset prefix; `note` flags platforms with no build.
const DL_OPTIONS = [
  { key: 'windows', label: 'Windows', arch: 'x64', icon: PICO.windows, osMatch: /\.exe$|\.msi$/ },
  { key: 'macos', label: 'macOS', arch: 'Apple Silicon', icon: PICO.macos, osMatch: /aarch64.*\.dmg$|arm64.*\.dmg$/ },
  { key: 'macos-intel', label: 'macOS', arch: 'Intel', icon: PICO.macos, osMatch: /(amd64|x64|intel).*\.dmg$/ },
  { key: 'linux', label: 'Linux', arch: 'AppImage', icon: PICO.linux, osMatch: /\.AppImage$/ },
  { key: 'android', label: 'Android', arch: 'APK', icon: PICO.android, osMatch: /\.apk$/ },
  { key: 'web', label: 'Web / iOS', arch: 'coming soon', icon: PICO.web, osMatch: null, soon: true },
];

function buildDownloadDialog() {
  const grids = document.querySelectorAll('.dl-grid');
  if (!grids.length) return;
  const { os: curOs, arch: curArch } = detectPlatform();
  const content = DL_OPTIONS.map((opt) => {
    let href = '#';
    let soon = opt.soon;
    if (!soon) {
      const baseOs = opt.key === 'macos-intel' ? 'macos' : opt.key;
      const archName = baseOs === 'macos'
        ? (opt.key === 'macos' ? 'aarch64' : 'x86_64')
        : opt.arch;
      href = selectAsset(latestAssets, baseOs === 'macos-intel' ? 'macos' : baseOs, archName) || RELEASE_PAGE;
    }
    const isArm = /arm|aarch64/i.test(navigator.userAgent);
    const isCurrent =
      opt.key === curOs ||
      (opt.key === 'macos-intel' && curOs === 'macos' && !isArm) ||
      (opt.key === 'macos' && curOs === 'macos' && isArm);
    const track = soon
      ? 'data-track="download_soon"'
      : `data-track="download_${opt.key}" data-platform="${opt.key.replace(/-.*$/, '')}" data-arch="${opt.arch}"`;
    const content = `
      <span class="dl-icon">${opt.icon}</span>
      <span class="dl-name">${opt.label}</span>
      ${soon ? '<span class="dl-soon">' + opt.arch + '</span>' : '<span class="dl-arch">' + opt.arch + '</span>'}`;
    return soon
      ? `<button type="button" class="dl-opt" data-action="open-waitlist" data-waitlist-source="${opt.key}" data-track="download_${opt.key}">${content}</button>`
      : `<a class="dl-opt" href="${href}" target="_blank" rel="noopener" ${track}>${content}</a>`;
  }).join('');
  grids.forEach((grid) => { grid.innerHTML = content; });
}

/* ---------- Pro dialog (explain + Stripe pay) ---------- */
const PRO_BENEFITS = [
  'Unlimited photos and albums',
  'Advanced on-device AI search',
  'Private sharing for the whole family',
  'Priority support and early features',
];

function proPriceFor() {
  const base = 9.99;
  return state.billing === 'yearly' ? (base * 0.8).toFixed(2) : base.toFixed(2);
}

function buildProDialog() {
  const benefits = document.getElementById('proBenefits');
  if (benefits) {
    benefits.innerHTML = PRO_BENEFITS
      .map((b) => `<li><span class="check">✓</span><span>${b}</span></li>`)
      .join('');
  }
  const price = document.getElementById('proPrice');
  if (price) price.innerHTML = `$${proPriceFor()}<small> ${state.billing === 'yearly' ? '/year' : '/month'}</small>`;
  const pay = document.getElementById('proPayBtn');
  if (pay) pay.setAttribute('href', STRIPE_PAYMENT_RE.test(proPaymentLink()) ? proPaymentLink() : '#');
}



/* Wire the "Get siegu free" and Pro/upgrade buttons. Free CTAs now open the
   download dialog (OS picker) instead of auto-downloading; Pro/upgrade buttons
   open the pro modal with the Stripe pay button. */
async function applyCtas() {
  await loadLatestRelease();
  const { os } = detectPlatform();

  // Free CTAs -> open the download dialog (which offers every platform).
  document.querySelectorAll('.hero-cta a[data-track="cta_get_started"], .cta-band a[data-track="cta_footer"], .header-actions a[data-track="cta_get_started"], .site-footer a[data-action="platform-download"], a[data-action="open-download"]').forEach((a) => {
    a.removeAttribute('href');
    a.setAttribute('href', '#');
    a.setAttribute('data-action', 'open-download');
    a.setAttribute('target', '');
    a.setAttribute('rel', '');
  });

  // Pro / Upgrade buttons -> open the pro dialog (Stripe pay inside).
  document.querySelectorAll('a[data-track="cta_upgrade"], a[data-action="open-pro"]').forEach((a) => {
    a.setAttribute('href', '#');
    a.setAttribute('data-action', 'open-pro');
    a.setAttribute('target', '');
    a.setAttribute('rel', '');
  });

  // Re-render pricing so the Free card's button opens the download dialog and
  // the Pro card's button opens the pro dialog (rebuilt with current billing).
  if (document.getElementById('pricingGrid')) renderPricing();
  buildDownloadDialog();
  buildProDialog();
}

/* ---------- Track outbound / CTA clicks ---------- */
function pushEvent(name, params) {
  const payload = { event: name, ...params };
  console.debug('[track]', payload);
  if (window.dataLayer) window.dataLayer.push(payload);
  if (typeof window.gtag === 'function') {
    window.gtag('event', name, params);
  }
}

function initTracking() {
  document.addEventListener('click', (e) => {
    const el = e.target.closest('[data-track]');
    if (!el) return;
    const trackName = el.getAttribute('data-track');

    // A free CTA with a platform stamp is an actual install download.
    if (el.getAttribute('data-platform')) {
      pushEvent('download_started', {
        platform: el.getAttribute('data-platform'),
        arch: el.getAttribute('data-arch') || 'unknown',
        locale: state.locale,
      });
    }

    // "Upgrade to Pro" / Pro pricing clicks = purchase intent.
    if (trackName === 'cta_upgrade' || trackName === 'pricing_pro') {
      pushEvent('upgrade_clicked', { locale: state.locale });
    }

    // "Try the live demo" clicks = engagement / trial intent.
    if (trackName === 'demo_clicked') {
      pushEvent('demo_clicked', { locale: state.locale });
    }

    pushEvent('cta', { cta_name: trackName, locale: state.locale });
  });
}

/* ---------- Hero slide show ---------- */
const carousel = {
  slides: [],
  index: 0,
  timer: null,
  interval: 6000,
  swipeStartX: null,
  reduceMotion: false,
};

async function loadSlides() {
  try {
    const res = await fetch(`${BASE_URL}slides.json`);
    if (res.ok) carousel.slides = (await res.json()).slides || [];
  } catch { carousel.slides = []; }
  if (!carousel.slides.length) carousel.slides = [{ src: `${BASE_URL}screenshot.png`, alt: 'Siegu' }];
}

function renderCarousel() {
  const bg = document.getElementById('heroBg');
  if (!bg) return;
  bg.innerHTML = carousel.slides
    .map((s, i) => `<div class="hero-bg-slide ${i === 0 ? 'is-active' : ''}" data-i="${i}" style="background-image:url('${s.src}')"></div>`)
    .join('');
  const dots = document.getElementById('carouselDots');
  if (dots) {
    dots.innerHTML = carousel.slides
      .map((_, i) => `<button type="button" class="hero-dot ${i === 0 ? 'is-active' : ''}" data-dot="${i}" aria-label="Slide ${i + 1}"></button>`)
      .join('');
  }
}

function announceSlide() {
  const announcer = document.getElementById('heroSlideAnnouncer');
  if (!announcer) return;
  announcer.textContent = `Slide ${carousel.index + 1} of ${carousel.slides.length}` +
    (carousel.slides[carousel.index]?.label ? `: ${carousel.slides[carousel.index].label}` : '');
}

function goSlide(i) {
  carousel.index = ((i % carousel.slides.length) + carousel.slides.length) % carousel.slides.length;
  document.querySelectorAll('.hero-bg-slide').forEach((el, idx) => el.classList.toggle('is-active', idx === carousel.index));
  document.querySelectorAll('.hero-dot').forEach((el, idx) => el.classList.toggle('is-active', idx === carousel.index));
  announceSlide();
}

function startCarousel() {
  if (carousel.reduceMotion) return;
  stopCarousel();
  carousel.timer = setInterval(() => goSlide(carousel.index + 1), carousel.interval);
}
function stopCarousel() { if (carousel.timer) { clearInterval(carousel.timer); carousel.timer = null; } }

function initCarousel() {
  const hero = document.getElementById('top');
  const bg = document.getElementById('heroBg');
  if (!hero || !bg) return;
  carousel.reduceMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
  renderCarousel();
  announceSlide();
  startCarousel();

  const dotsEl = document.getElementById('carouselDots');
  if (dotsEl) dotsEl.addEventListener('click', (e) => {
    const d = e.target.closest('[data-dot]');
    if (!d) return;
    goSlide(Number(d.getAttribute('data-dot')));
    startCarousel();
  });

  // keyboard: arrow keys move the slide show; autoplay only while idle
  hero.addEventListener('keydown', (e) => {
    if (e.key === 'ArrowLeft') { goSlide(carousel.index - 1); startCarousel(); e.preventDefault(); }
    if (e.key === 'ArrowRight') { goSlide(carousel.index + 1); startCarousel(); e.preventDefault(); }
  });

  // pause while interacting with the hero, resume on leave (unless reduced motion)
  hero.addEventListener('mouseenter', stopCarousel);
  hero.addEventListener('mouseleave', startCarousel);
  hero.addEventListener('touchstart', stopCarousel, { passive: true });

  // swipe navigation
  bg.addEventListener('touchstart', (e) => { carousel.swipeStartX = e.touches[0].clientX; }, { passive: true });
  bg.addEventListener('touchend', (e) => {
    if (carousel.swipeStartX === null) return;
    const dx = e.changedTouches[0].clientX - carousel.swipeStartX;
    if (Math.abs(dx) > 50) { goSlide(carousel.index + (dx < 0 ? 1 : -1)); startCarousel(); }
    carousel.swipeStartX = null;
  }, { passive: true });

  // pause when tab hidden
  document.addEventListener('visibilitychange', () => (document.hidden ? stopCarousel() : startCarousel()));
}

/* ---------- Command palette (⌘K / Ctrl+K) ---------- */
// Quick-jump links surfaced by the palette, with search keywords so the
// natural-language query maps to the right destination.
const COMMANDS = [
  { group: 'nav.download', label: 'download.title', desc: 'download.desc',
    icon: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 3v12m0 0 4-4m-4 4-4-4"/><path d="M4 17v2a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-2"/></svg>',
    action: () => document.querySelector('[data-action="open-download"]')?.click(), keywords: ['download', 'install', 'get', 'app', 'download', 'windows', 'mac', 'linux', 'android', 'web', 'apk', 'dmg', 'deb', 'appimage'] },
  { group: 'nav.pricing', label: 'pricing.title', desc: 'pricing.subtitle',
    icon: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 2 3 7v10l9 5 9-5V7l-9-5z"/><path d="M12 22V12"/><path d="M3 7l9 5 9-5"/></svg>',
    href: 'pricing.html', keywords: ['pricing', 'price', 'cost', 'pro', 'free', 'plan', 'plans', 'upgrade', 'family', 'team', 'billing', 'subscription', 'pay', 'buy', 'monthly', 'yearly'] },
  { group: 'nav.pricing', label: 'connect.title', desc: 'connect.subtitle',
    icon: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="6" cy="6" r="3"/><circle cx="18" cy="17" r="3"/><path d="M8.1 8.1 16 16"/><circle cx="6" cy="18" r="3"/><circle cx="18" cy="6" r="3"/><path d="M8.4 16 15.6 8"/></svg>',
    href: 'connect.html', keywords: ['connect', 'pro', 'family', 'plan', 'upgrade', 'buy', 'purchase', 'subscription', 'sync', 'sharing', 'pay', 'siegu connect'] },
  { group: 'nav.faq', label: 'faq.title', desc: 'faq.eyebrow',
    icon: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><path d="M9.5 9.2a2.5 2.5 0 1 1 3.4 2.8c-.6.3-1 .7-1 1.5V14"/><path d="M12 17h.01"/></svg>',
    href: 'faq.html', keywords: ['faq', 'questions', 'help', 'support', 'privacy', 'offline', 'share', 'platforms', 'import', 'google', 'icloud'] },
  { group: 'resources', label: 'footer.resources_links.docs', desc: 'resources.docs',
    icon: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M4 19.5A2.5 2.5 0 0 1 6.5 17H20V4H6.5A2.5 2.5 0 0 0 4 6.5v13z"/><path d="M4 19.5A2.5 2.5 0 0 0 6.5 22H20v-5"/></svg>',
    href: 'docs.html', keywords: ['docs', 'documentation', 'guide', 'manual', 'help', 'tutorial'] },
  { group: 'resources', label: 'footer.resources_links.demo', desc: 'resources.demo',
    icon: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="6 3 20 12 6 21 6 3"/></svg>',
    href: 'https://siegu.onrender.com', external: true, keywords: ['demo', 'live', 'try', 'trial', 'preview', 'web app'] },
  { group: 'resources', label: 'footer.company_links.github', desc: 'resources.source',
    icon: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M9 19c-5 1.5-5-2.5-7-3m14 6v-3.9a3.4 3.4 0 0 0-.9-2.6c3-.3 6.2-1.5 6.2-6.8a5.3 5.3 0 0 0-1.4-3.7 4.9 4.9 0 0 0-.1-3.7s-1.1-.4-3.7 1.4a12.7 12.7 0 0 0-6.7 0C6.3 2.9 5.2 3.3 5.2 3.3a4.9 4.9 0 0 0-.1 3.7A5.3 5.3 0 0 0 3.7 10.7c0 5.3 3.2 6.5 6.2 6.8a3.4 3.4 0 0 0-.9 2.6V22"/></svg>',
    href: 'https://github.com/denzyldick/siegu', external: true, keywords: ['github', 'source', 'code', 'open source', 'repository'] },
];

function cmdIcon(svg) { return `<span class="cmd-ico">${svg}</span>`; }

function renderCommands(filter) {
  const groups = document.getElementById('cmdGroups');
  const empty = document.getElementById('cmdEmpty');
  const q = (filter || '').trim().toLowerCase();
  const d = state.dict || {};

  let matches = COMMANDS;
  if (q) {
    matches = COMMANDS.filter((c) => {
      const label = (lookup(c.label, d) || '').toLowerCase();
      const kw = (c.keywords || []).join(' ').toLowerCase();
      return label.includes(q) || kw.includes(q);
    });
  }
  cmdList = matches;

  let html = '';
  const byGroup = {};
  matches.forEach((c) => { (byGroup[c.group] = byGroup[c.group] || []).push(c); });

  const groupLabels = { 'nav.download': t('nav.download'), 'nav.pricing': t('nav.pricing'), 'nav.faq': t('nav.faq'), resources: t('search.resources') };
  let cmdIdx = 0;
  Object.keys(byGroup).forEach((g) => {
    html += `<div class="cmd-group-label">${groupLabels[g] || g}</div>`;
    byGroup[g].forEach((c) => {
      const label = t(c.label);
      const desc = t(c.desc);
      const idxAttr = `data-idx="${cmdIdx++}"`;
      html += c.href
        ? `<a class="cmd-item" href="${c.href}" ${c.external ? 'target="_blank" rel="noopener"' : ''} ${idxAttr} data-action="cmd-goto">${cmdIcon(c.icon)}<span class="cmd-label">${label}${desc ? `<span class="cmd-desc">${desc}</span>` : ''}</span></a>`
        : `<button type="button" class="cmd-item" ${idxAttr} data-action="cmd-act">${cmdIcon(c.icon)}<span class="cmd-label">${label}${desc ? `<span class="cmd-desc">${desc}</span>` : ''}</span></button>`;
    });
  });

  groups.innerHTML = html;
  empty.hidden = matches.length > 0;
  activeCmdIndex = 0;
  updateCmdActive();
}

let activeCmdIndex = 0;
let cmdList = [];

function cmdItems() { return Array.from(document.querySelectorAll('#cmdGroups .cmd-item')); }

function updateCmdActive() {
  const items = cmdItems();
  items.forEach((el, i) => el.classList.toggle('is-active', i === activeCmdIndex));
  const active = items[activeCmdIndex];
  if (active) active.scrollIntoView({ block: 'nearest' });
}

function cmdMove(dir) {
  const items = cmdItems();
  if (!items.length) return;
  activeCmdIndex = (activeCmdIndex + dir + items.length) % items.length;
  updateCmdActive();
}

function cmdSelect() {
  const items = cmdItems();
  const el = items[activeCmdIndex];
  if (!el) return;
  const href = el.getAttribute('href') || '';
  closeCmdPalette();
  if (el.getAttribute('data-action') === 'cmd-goto') {
    if (el.target === '_blank') { window.open(el.href, '_blank'); }
    else if (href.startsWith('#')) {
      const id = href.slice(1);
      document.getElementById(id)?.scrollIntoView({ behavior: 'smooth' });
    } else { window.location.href = el.href; }
  } else {
    const c = cmdList[Number(el.getAttribute('data-idx')) || 0];
    c?.action?.();
  }
}

function openCmdPalette() {
  const pal = document.getElementById('cmdPalette');
  if (!pal) return;
  document.getElementById('cmdInput').value = '';
  renderCommands('');
  pal.setAttribute('aria-hidden', 'false');
  pal.classList.add('open');
  document.body.style.overflow = 'hidden';
  setTimeout(() => document.getElementById('cmdInput').focus(), 30);
  pushEvent('cmd_opened', { locale: state.locale });
}
function closeCmdPalette() {
  const pal = document.getElementById('cmdPalette');
  if (!pal) return;
  pal.setAttribute('aria-hidden', 'true');
  pal.classList.remove('open');
  document.body.style.overflow = '';
}

function initMobileNav() {
  const menuBtn = document.getElementById('menuBtn');
  const overlay = document.getElementById('navOverlay');
  if (!menuBtn || !overlay) return;
  const closeBtn = overlay.querySelector('.nav-close');
  const toggle = (open) => {
    overlay.classList.toggle('open', open);
    overlay.setAttribute('aria-hidden', String(!open));
    menuBtn.setAttribute('aria-expanded', String(open));
  };
  menuBtn.addEventListener('click', () => toggle(!overlay.classList.contains('open')));
  closeBtn?.addEventListener('click', () => toggle(false));
  overlay.addEventListener('click', (e) => {
    if (e.target.closest('a') || e.target === overlay) toggle(false);
  });
  document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape' && overlay.classList.contains('open')) toggle(false);
  });
  window.addEventListener('resize', () => {
    if (window.innerWidth > 820) toggle(false);
  });
}

function initCmdPalette() {
  const pal = document.getElementById('cmdPalette');
  const input = document.getElementById('cmdInput');
  if (!pal || !input) return;

  document.getElementById('searchTrigger')?.addEventListener('click', openCmdPalette);
  input.addEventListener('input', () => renderCommands(input.value));
  input.addEventListener('keydown', (e) => {
    if (e.key === 'ArrowDown') { e.preventDefault(); cmdMove(1); }
    else if (e.key === 'ArrowUp') { e.preventDefault(); cmdMove(-1); }
    else if (e.key === 'Enter') { e.preventDefault(); cmdSelect(); }
  });
  pal.addEventListener('click', (e) => {
    if (e.target === pal) closeCmdPalette();
  });
  document.getElementById('cmdGroups')?.addEventListener('click', (e) => {
    const item = e.target.closest('.cmd-item');
    if (!item) return;
    closeCmdPalette();
    if (item.getAttribute('data-action') === 'cmd-goto') {
      const href = item.getAttribute('href') || '';
      if (item.target === '_blank') {
        window.open(href, '_blank');
      } else if (href.startsWith('#')) {
        document.getElementById(href.slice(1))?.scrollIntoView({ behavior: 'smooth' });
      } else {
        window.location.href = href;
      }
    } else {
      const c = cmdList[Number(item.getAttribute('data-idx')) || 0];
      c?.action?.();
    }
  });
  document.getElementById('cmdTheme')?.addEventListener('click', () => { cycleTheme(); closeCmdPalette(); });

  document.addEventListener('keydown', (e) => {
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'k') { e.preventDefault(); openCmdPalette(); }
    if (e.key === 'Escape') closeCmdPalette();
  });
}

/* ---------- Boot ---------- */
async function boot() {
  // The command palette must work no matter what happens below: initialize it
  // first so a failure in any other component can't take down the keyboard
  // shortcut or the search UX.
  initCmdPalette();
  initMobileNav();

  const saved = localStorage.getItem('siegu_lang');
  const nav = (navigator.language || 'en').split('-')[0];
  state.locale = SUPPORTED.includes(saved) ? saved : SUPPORTED.includes(nav) ? nav : FALLBACK;
  state.dict = await loadLocale(state.locale);
  applyTexts();
  applyCtas();

  const yearEl = document.getElementById('year');
  if (yearEl) yearEl.textContent = new Date().getFullYear();

  const langBtn = document.getElementById('langBtn');
  const langMenu = document.getElementById('langMenu');
  if (langBtn && langMenu) {
    langBtn.addEventListener('click', () => toggleLangMenu());
    document.addEventListener('click', (e) => {
      if (!langMenu.classList.contains('open')) return;
      if (e.target.closest('#langBtn') || e.target.closest('#langMenu')) return;
      toggleLangMenu(false);
    });
    langMenu.addEventListener('click', async (e) => {
      const btn = e.target.closest('[data-lang]');
      if (!btn) return;
      state.locale = btn.getAttribute('data-lang');
      localStorage.setItem('siegu_lang', state.locale);
      state.dict = await loadLocale(state.locale);
      applyTexts();
      toggleLangMenu(false);
    });
  }

  const billingToggle = document.getElementById('billingToggle');
  if (billingToggle) {
    billingToggle.addEventListener('click', (e) => {
      const btn = e.target.closest('[data-period]');
      if (!btn) return;
      state.billing = btn.getAttribute('data-period');
      document.querySelectorAll('#billingToggle button').forEach((b) => b.classList.toggle('active', b === btn));
      renderPricing();
      buildProDialog();
    });
  }

  /* Download + Pro dialogs */
  const dlModal = document.getElementById('downloadModal');
  const proModal = document.getElementById('proModal');
  function openDl(overlay) {
    if (!overlay) return;
    overlay.setAttribute('aria-hidden', 'false');
    overlay.classList.add('open');
    document.body.style.overflow = 'hidden';
  }
  function closeDl(overlay) {
    if (!overlay) return;
    overlay.setAttribute('aria-hidden', 'true');
    overlay.classList.remove('open');
    document.body.style.overflow = '';
  }
  document.addEventListener('click', (e) => {
    const dlTrig = e.target.closest('[data-action="open-download"]');
    if (dlTrig) { e.preventDefault(); openDl(dlModal); pushEvent('download_dialog_opened', { locale: state.locale }); return; }
    const proTrig = e.target.closest('[data-action="open-pro"]');
    if (proTrig) { e.preventDefault(); buildProDialog(); openDl(proModal); pushEvent('pro_dialog_opened', { locale: state.locale }); return; }
    if (e.target.closest('[data-dl-close]') || e.target === dlModal) closeDl(dlModal);
    if (e.target.closest('[data-pro-close]') || e.target === proModal) closeDl(proModal);
  });
  document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') { closeDl(dlModal); closeDl(proModal); }
  });

  /* Waitlist modal (Family plan) */
  const modal = document.getElementById('waitlistModal');
  const modalClose = document.getElementById('waitlistClose');
  function openModal() {
    if (!modal) return;
    modal.setAttribute('aria-hidden', 'false');
    modal.classList.add('open');
    document.body.style.overflow = 'hidden';
    modal.querySelector('input[type="email"]')?.focus();
  }
  function closeModal() {
    if (!modal) return;
    modal.setAttribute('aria-hidden', 'true');
    modal.classList.remove('open');
    document.body.style.overflow = '';
  }
  document.addEventListener('click', (e) => {
    const trig = e.target.closest('[data-action="open-waitlist"]');
    if (!trig) return;
    e.preventDefault();
    // Segment the waitlist by what opened it (family plan, macOS, web, ...).
    const src = trig.getAttribute('data-waitlist-source') || 'family';
    const field = document.getElementById('waitlistSource');
    if (field) field.value = src;
    openModal();
  });
  if (modalClose) modalClose.addEventListener('click', closeModal);
  if (modal) modal.addEventListener('click', (e) => { if (e.target === modal) closeModal(); });
  document.addEventListener('keydown', (e) => { if (e.key === 'Escape') closeModal(); });

  // AJAX-submit the waitlist form so we can report success/failure to GA.
  const wlForm = modal ? modal.querySelector('form.waitlist-modal-form') : null;
  if (wlForm) {
    wlForm.addEventListener('submit', async (e) => {
      e.preventDefault();
      const btn = wlForm.querySelector('button[type="submit"]');
      const email = wlForm.querySelector('input[type="email"]');
      const src = wlForm.querySelector('#waitlistSource');
      if (btn) { btn.disabled = true; btn.textContent = '…'; }
      const data = { email: email.value, source: src ? src.value : 'family', _subject: 'siegu waitlist' };
      try {
        const res = await fetch(wlForm.action, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json', Accept: 'application/json' },
          body: JSON.stringify(data),
        });
        if (res.ok) {
          pushEvent('form_submitted', { form: 'waitlist', source: data.source, locale: state.locale });
          wlForm.innerHTML = '<p class="waitlist-success" data-i18n="waitlist.success">You\'re on the list — we\'ll be in touch!</p>';
          const el = wlForm.querySelector('[data-i18n="waitlist.success"]');
          if (el && state.dict) el.textContent = t('waitlist.success');
        } else {
          pushEvent('form_failed', { form: 'waitlist', status: res.status, locale: state.locale });
          if (btn) { btn.disabled = false; btn.textContent = ''; }
          const note = wlForm.querySelector('.waitlist-note');
          if (note) note.textContent = 'Something went wrong — please try again.';
        }
      } catch (err) {
        pushEvent('form_failed', { form: 'waitlist', error: String(err), locale: state.locale });
        if (btn) { btn.disabled = false; btn.textContent = ''; }
        const note = wlForm.querySelector('.waitlist-note');
        if (note) note.textContent = 'Network error — please try again.';
      }
    });
  }

  const faqList = document.getElementById('faqList');
  if (faqList) {
    faqList.addEventListener('click', (e) => {
      const btn = e.target.closest('[data-faq]');
      if (!btn) return;
      const item = btn.closest('.faq-item');
      const isOpen = item.classList.contains('open');
      document.querySelectorAll('.faq-item').forEach((x) => { x.classList.remove('open'); x.querySelector('.faq-a').style.maxHeight = '0px'; });
      if (!isOpen) {
        item.classList.add('open');
        const a = item.querySelector('.faq-a');
        a.style.maxHeight = a.scrollHeight + 'px';
      }
    });
  }

  applyTheme();
  const themeBtn = document.getElementById('themeBtn');
  if (themeBtn) themeBtn.addEventListener('click', cycleTheme);
  window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', applyTheme);

  initTracking();

  if (document.getElementById('heroBg')) {
    await loadSlides();
    initCarousel();
  }

  // Fire trackers after a short delay (or after consent). For now, auto-enable.
  setTimeout(trackersOn, 800);

  initReveal();
}

function initReveal() {
  const cards = document.querySelectorAll('.features-grid .feature');
  if (!cards.length) return;
  // Skip if the user prefers reduced motion — cards stay visible via CSS.
  const reduce = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
  if (reduce) return;

  cards.forEach((card, i) => {
    const dir = i % 2 === 0 ? 'left' : 'right';
    card.classList.add('reveal-' + dir);
  });

  if (!('IntersectionObserver' in window)) {
    cards.forEach((c) => c.classList.add('is-visible'));
    return;
  }

  const io = new IntersectionObserver(
    (entries) => {
      entries.forEach((entry) => {
        if (entry.isIntersecting) {
          entry.target.classList.add('is-visible');
          io.unobserve(entry.target);
        }
      });
    },
    { threshold: 0.18, rootMargin: '0px 0px -40px 0px' },
  );
  cards.forEach((c) => io.observe(c));
}

document.addEventListener('DOMContentLoaded', () => {
  boot().catch((e) => console.error('[siegu] boot failed:', e));
});
