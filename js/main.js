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
      // Free -> platform download; Pro -> Stripe Payment Link; Team -> waitlist.
      const btnHref = isWaitlist ? '#'
        : key === 'free' ? freeDownloadUrl()
        : (STRIPE_PAYMENT_RE.test(proPaymentLink()) ? proPaymentLink() : '#');
      const btnTrack = isWaitlist ? 'data-track="pricing_waitlist"' : `data-track="pricing_${key}"`;
      // Tag the Free (download) card with platform/arch for GA breakdowns.
      let dataPlatform = '';
      if (key === 'free' && !isWaitlist) {
        const pl = detectPlatform();
        if (latestAssets.length > 0 && pl.os !== 'web' && pl.os !== 'ios') {
          dataPlatform = ` data-platform="${pl.os}" data-arch="${pl.arch}"`;
        }
      }
      const btnExtra = isWaitlist ? 'data-action="open-waitlist"'
        : (btnHref !== '#' ? `target="_blank" rel="noopener"${dataPlatform}` : '');
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
        <a class="btn ${btnClass}" href="${btnHref}" ${btnExtra} ${btnTrack}>${p.cta || ''}</a>
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

// Resolve the download URL for the visitor's platform. Returns null when we
// can't match an installer (falls back to the release page).
function freeDownloadUrl() {
  const { os, arch } = detectPlatform();
  if (latestAssets.length) {
    const direct = selectAsset(latestAssets, os, arch);
    if (direct) return direct;
  }
  // No matching asset yet — send them to the release page so they always get
  // the latest working installers regardless of platform.
  return RELEASE_PAGE;
}

/* Wire the "Get siegu free" links. Free always goes to the platform download
   (or opens the waitlist modal for platforms with no build — e.g. web/iOS),
   and the Pro/upgrade buttons go to the Stripe Payment Link */
async function applyCtas() {
  await loadLatestRelease();
  const { os, arch } = detectPlatform();
  const freeHref = freeDownloadUrl();
  const waitlistPlatforms = ['ios', 'web'];
  const isWaitlistPlatform = waitlistPlatforms.includes(os);
  // A direct asset match (not the fallback release page) is a real download.
  const isDirectAsset = latestAssets.length > 0 && os !== 'web' && os !== 'ios';

  document.querySelectorAll('.hero-cta a[data-track="cta_get_started"], .cta-band a[data-track="cta_footer"], .header-actions a[data-track="cta_get_started"], .site-footer a[data-action="platform-download"]').forEach((a) => {
    if (isWaitlistPlatform) {
      a.setAttribute('href', '#');
      a.setAttribute('data-action', 'open-waitlist');
      a.setAttribute('target', '');
      a.removeAttribute('data-track');
      a.setAttribute('data-track', 'cta_waitlist');
      a.removeAttribute('data-platform');
      a.removeAttribute('data-arch');
      // Remember which platform triggered the waitlist so the email list is
      // segmented (macOS, iOS, etc.) instead of everything lumped as "family".
      a.setAttribute('data-waitlist-source', os);
    } else {
      a.setAttribute('href', freeHref);
      a.removeAttribute('data-action');
      a.setAttribute('target', '_blank');
      a.setAttribute('rel', 'noopener');
      // Stamp platform/arch so GA can break out downloads by OS.
      if (isDirectAsset) {
        a.setAttribute('data-platform', os);
        a.setAttribute('data-arch', arch);
      } else {
        a.removeAttribute('data-platform');
        a.removeAttribute('data-arch');
      }
    }
  });

  document.querySelectorAll('a[data-track="cta_upgrade"]').forEach((a) => {
    a.setAttribute('href', STRIPE_PAYMENT_RE.test(proPaymentLink()) ? proPaymentLink() : '#');
  });

  // Re-render pricing so the Free card's button resolves to the direct
  // platform download now that release assets are loaded — and stamp it too.
  if (document.getElementById('pricingGrid')) renderPricing();
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
