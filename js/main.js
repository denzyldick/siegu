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
  document.getElementById('langCode').textContent = state.locale.toUpperCase();
  const menu = document.getElementById('langMenu');
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
  const plans = ['free', 'pro', 'team'];
  const grid = document.getElementById('pricingGrid');
  let html = plans
    .map((key) => {
      const p = lookup('pricing.' + key, d) || {};
      const featured = key === 'pro';
      const isYearly = state.billing === 'yearly';
      const price = { free: 0, pro: 9.99, team: 9.99 }[key];
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
const GA_MEASUREMENT_ID = 'G-8Q72K460VG';
const GA_ENABLED = /^G-[A-Z0-9]+$/.test(GA_MEASUREMENT_ID);

function trackersOn() {
  state.trackersOn = true;
  if (!GA_ENABLED) return;
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
  const grid = document.getElementById('dlGrid');
  if (!grid) return;
  const { os: curOs, arch: curArch } = detectPlatform();
  grid.innerHTML = DL_OPTIONS.map((opt) => {
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

/* ---------- Hero carousel ---------- */
const carousel = {
  slides: [],
  index: 0,
  timer: null,
  interval: 5000,
};

async function loadSlides() {
  try {
    const res = await fetch(`${BASE_URL}slides.json`);
    if (res.ok) carousel.slides = (await res.json()).slides || [];
  } catch { carousel.slides = []; }
  if (!carousel.slides.length) carousel.slides = [{ src: `${BASE_URL}screenshot.png`, alt: 'Siegu' }];
}

function renderCarousel() {
  const viewport = document.getElementById('carouselViewport');
  const dots = document.getElementById('carouselDots');
  // First slide is active immediately; others fade in when the timer advances.
  viewport.innerHTML = carousel.slides
    .map((s, i) => `
      <div class="carousel-slide ${i === 0 ? 'is-active' : ''}" data-i="${i}">
        <img src="${s.src}" alt="${s.alt || ''}" loading="${i === 0 ? 'eager' : 'lazy'}" />
        ${s.label ? `<div class="carousel-label">${s.label}</div>` : ''}
      </div>`)
    .join('');
  dots.innerHTML = carousel.slides
    .map((_, i) => `<button type="button" class="carousel-dot ${i === 0 ? 'is-active' : ''}" data-dot="${i}" aria-label="Slide ${i + 1}"></button>`)
    .join('');
}

function goSlide(i) {
  carousel.index = ((i % carousel.slides.length) + carousel.slides.length) % carousel.slides.length;
  document.querySelectorAll('.carousel-slide').forEach((el, idx) => el.classList.toggle('is-active', idx === carousel.index));
  document.querySelectorAll('.carousel-dot').forEach((el, idx) => el.classList.toggle('is-active', idx === carousel.index));
}

function startCarousel() {
  stopCarousel();
  carousel.timer = setInterval(() => goSlide(carousel.index + 1), carousel.interval);
}
function stopCarousel() { if (carousel.timer) { clearInterval(carousel.timer); carousel.timer = null; } }

function initCarousel() {
  const car = document.getElementById('heroCarousel');
  if (!car) return;
  renderCarousel();
  startCarousel();

  document.getElementById('carouselPrev').addEventListener('click', () => { goSlide(carousel.index - 1); startCarousel(); });
  document.getElementById('carouselNext').addEventListener('click', () => { goSlide(carousel.index + 1); startCarousel(); });
  document.getElementById('carouselDots').addEventListener('click', (e) => {
    const d = e.target.closest('[data-dot]');
    if (!d) return;
    goSlide(Number(d.getAttribute('data-dot')));
    startCarousel();
  });

  car.addEventListener('mouseenter', stopCarousel);
  car.addEventListener('mouseleave', startCarousel);
  // pause when tab hidden
  document.addEventListener('visibilitychange', () => (document.hidden ? stopCarousel() : startCarousel()));
}

/* ---------- Boot ---------- */
async function boot() {
  const saved = localStorage.getItem('siegu_lang');
  const nav = (navigator.language || 'en').split('-')[0];
  state.locale = SUPPORTED.includes(saved) ? saved : SUPPORTED.includes(nav) ? nav : FALLBACK;
  state.dict = await loadLocale(state.locale);
  applyTexts();
  applyCtas();

  document.getElementById('year').textContent = new Date().getFullYear();

  document.getElementById('langBtn').addEventListener('click', () => toggleLangMenu());
  document.addEventListener('click', (e) => {
    const b = document.querySelector('#langMenu');
    if (!b.classList.contains('open')) return;
    if (e.target.closest('#langBtn') || e.target.closest('#langMenu')) return;
    toggleLangMenu(false);
  });
  document.getElementById('langMenu').addEventListener('click', async (e) => {
    const btn = e.target.closest('[data-lang]');
    if (!btn) return;
    state.locale = btn.getAttribute('data-lang');
    localStorage.setItem('siegu_lang', state.locale);
    state.dict = await loadLocale(state.locale);
    applyTexts();
    toggleLangMenu(false);
  });

  document.getElementById('billingToggle').addEventListener('click', (e) => {
    const btn = e.target.closest('[data-period]');
    if (!btn) return;
    state.billing = btn.getAttribute('data-period');
    document.querySelectorAll('#billingToggle button').forEach((b) => b.classList.toggle('active', b === btn));
    renderPricing();
    buildProDialog();
  });

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

  document.getElementById('faqList').addEventListener('click', (e) => {
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

  applyTheme();
  const themeBtn = document.getElementById('themeBtn');
  if (themeBtn) themeBtn.addEventListener('click', cycleTheme);
  window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', applyTheme);

  initTracking();

  await loadSlides();
  initCarousel();

  // Fire trackers after a short delay (or after consent). For now, auto-enable.
  setTimeout(trackersOn, 800);
}

document.addEventListener('DOMContentLoaded', boot);
