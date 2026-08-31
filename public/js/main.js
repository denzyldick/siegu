/* Siegu landing — i18n, pricing, FAQ, theme, trackers. No framework, no build. */
'use strict';

const SUPPORTED = ['en', 'nl', 'fr', 'es', 'pap', 'de', 'it', 'pt'];
const FALLBACK = 'en';

const state = {
  locale: FALLBACK,
  dict: null,
  billing: 'monthly',
  trackersOn: false,
};

/* ---------- i18n ---------- */
async function loadLocale(code) {
  try {
    const res = await fetch(`/locales/${code}.json`);
    if (!res.ok) throw new Error(res.status);
    return await res.json();
  } catch {
    const fb = await fetch(`/locales/${FALLBACK}.json`);
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
  return state.billing === 'yearly' ? (price * 0.8).toFixed(0) : String(price);
}

function renderPricing() {
  const d = state.dict || {};
  const plans = ['free', 'pro', 'team'];
  const grid = document.getElementById('pricingGrid');
  const html = plans
    .map((key) => {
      const p = lookup('pricing.' + key, d) || {};
      const featured = key === 'pro';
      const isYearly = state.billing === 'yearly';
      const price = { free: 0, pro: 5, team: 9 }[key];
      const feats = Array.isArray(p.features) ? p.features : [];
      return `
      <div class="plan ${featured ? 'featured' : ''}">
        ${featured ? `<span class="plan-badge">${p.highlight || ''}</span>` : ''}
        <p class="plan-name">${p.name || key}</p>
        <div class="plan-price"><span class="cur">$</span>${priceFor(key, price)}</div>
        <p class="period">${p.period || ''}</p>
        ${isYearly && key !== 'free' ? `<p class="yearly-badge">${p.yearly_badge || ''}</p>` : ''}
        <p class="tagline">${p.tagline || ''}</p>
        <ul>${feats.map((f) => `<li><span class="check">✓</span><span>${f}</span></li>`).join('')}</ul>
        <a class="btn ${featured ? 'btn-ink' : 'btn-ghost'}" href="#" data-track="pricing_${key}">${p.cta || ''}</a>
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
function applyTheme() {
  const dark = window.matchMedia('(prefers-color-scheme: dark)').matches;
  const root = document.documentElement;
  root.className = dark ? 'is-dark' : '';
}

/* ---------- Trackers / analytics ----------
   Called after the (optional) consent banner. Vendor snippets load here. */
function trackersOn() {
  state.trackersOn = true;
  //
  // ==== ADD YOUR TRACKERS BELOW ====
  // Example (self-hosted Plausible):
  //   const s = document.createElement('script');
  //   s.defer = true; s.dataset.domain = 'siegu.io';
  //   s.src = 'https://plausible.yourhost.com/js/script.js';
  //   document.head.appendChild(s);
  //
}

/* ---------- Track outbound / CTA clicks ---------- */
function initTracking() {
  document.addEventListener('click', (e) => {
    const el = e.target.closest('[data-track]');
    if (!el) return;
    const ev = { name: el.getAttribute('data-track'), locale: state.locale };
    console.debug('[track]', ev);
    if (window.dataLayer) window.dataLayer.push({ event: 'cta', ...ev });
  });
}

/* ---------- Boot ---------- */
async function boot() {
  const saved = localStorage.getItem('siegu_lang');
  const nav = (navigator.language || 'en').split('-')[0];
  state.locale = SUPPORTED.includes(saved) ? saved : SUPPORTED.includes(nav) ? nav : FALLBACK;
  state.dict = await loadLocale(state.locale);
  applyTexts();

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
  window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', applyTheme);

  initTracking();

  // Fire trackers after a short delay (or after consent). For now, auto-enable.
  setTimeout(trackersOn, 800);
}

document.addEventListener('DOMContentLoaded', boot);
