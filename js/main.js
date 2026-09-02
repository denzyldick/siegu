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
      const btnClass = featured ? 'btn-ink' : isWaitlist ? 'btn-ghost' : 'btn-ghost';
      const btnHref = isWaitlist ? '#' : checkoutHref(key);
      const btnTrack = isWaitlist ? 'data-track="pricing_waitlist"' : `data-track="pricing_${key}"`;
      const btnExtra = isWaitlist ? 'data-action="open-waitlist"' : (btnHref !== '#' ? 'target="_blank" rel="noopener"' : '');
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

/* ---------- Gumroad checkout ----------
   First distribution channel. A build script (scripts/build-static.mjs)
   substitutes the real per-tier product URLs into the GUMROAD_* placeholders.
   While the placeholders remain unset, the matching buttons stay inert (#). */
const GUMROAD_PRODUCTS = {
  free: 'https://3848817799485.gumroad.com/l/nrohiy',
  pro: 'https://3848817799485.gumroad.com/l/acudv',
  team: '',
  primary: 'https://3848817799485.gumroad.com/l/acudv',
};

// Accepts custom-subdomain or standard Gumroad product links, e.g.
//   https://3848817799485.gumroad.com/l/acudv   (custom subdomain, /l/{slug})
//   https://yourname.gumroad.com/l/slug
//   https://gumroad.com/l/slug
const GUMROAD_RE = /^https:\/\/[a-z0-9.-]*gumroad\.com\/l\/[a-z0-9-]+$/i;

function checkoutHref(planKey) {
  // A pricing row uses its own tier URL when set; otherwise every button
  // falls back to the primary URL so a single-product launch wires them all.
  const candidates = [planKey && GUMROAD_PRODUCTS[planKey], GUMROAD_PRODUCTS.primary];
  for (const url of candidates) {
    if (GUMROAD_RE.test(url || '')) return url;
  }
  return '#';
}

/* Point the header / footer / hero "Get siegu free" links at the free
   checkout (free-first funnel), and the dedicated upgrade button at Pro. */
function applyCtas() {
  document.querySelectorAll('.hero-cta a[data-track="cta_get_started"], .cta-band a[data-track="cta_footer"], .header-actions a[data-track="cta_get_started"]').forEach((a) => {
    a.setAttribute('href', checkoutHref('free'));
  });
  document.querySelectorAll('a[data-track="cta_upgrade"]').forEach((a) => {
    a.setAttribute('href', checkoutHref('pro'));
  });
}

/* ---------- Track outbound / CTA clicks ---------- */
function initTracking() {
  document.addEventListener('click', (e) => {
    const el = e.target.closest('[data-track]');
    if (!el) return;
    const ev = { name: el.getAttribute('data-track'), locale: state.locale };
    console.debug('[track]', ev);
    if (window.dataLayer) window.dataLayer.push({ event: 'cta', ...ev });
    if (typeof window.gtag === 'function') {
      window.gtag('event', 'cta', { cta_name: ev.name, locale: ev.locale });
    }
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
    if (e.target.closest('[data-action="open-waitlist"]')) {
      e.preventDefault();
      openModal();
    }
  });
  if (modalClose) modalClose.addEventListener('click', closeModal);
  if (modal) modal.addEventListener('click', (e) => { if (e.target === modal) closeModal(); });
  document.addEventListener('keydown', (e) => { if (e.key === 'Escape') closeModal(); });

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
