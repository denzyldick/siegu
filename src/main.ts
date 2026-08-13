import { createApp } from 'vue';
import { createPinia } from 'pinia';
import piniaPluginPersistedstate from 'pinia-plugin-persistedstate';
import App from './App.vue';
import 'vuetify/styles';
import '@mdi/font/css/materialdesignicons.css';
import '@fontsource-variable/outfit';

import * as components from 'vuetify/components';
import { createVuetify } from 'vuetify/dist/vuetify.js';
import { createI18n } from 'vue-i18n';
import en from './locales/en.json';
import nl from './locales/nl.json';
import fr from './locales/fr.json';
import es from './locales/es.json';
import pap from './locales/pap.json';
import de from './locales/de.json';
import it from './locales/it.json';
import pt from './locales/pt.json';

import './styles/variables.css';
import './styles/base.css';
import './styles/animations.css';

import { getThemePreference, initSystemTheme, resolveTheme, syncTheme } from './services/theme';

const savedLang: string = localStorage.getItem('siegu_language') || 'en';

const i18n = createI18n({
  legacy: false,
  locale: savedLang,
  fallbackLocale: 'en',
  messages: { en, nl, fr, es, pap, de, it, pt },
});

const vuetify = createVuetify({
  components,
  theme: {
    defaultTheme: resolveTheme(),
    themes: {
      light: {
        dark: false,
        colors: {
          background: '#fafafa',
          surface: '#ffffff',
          'surface-light': '#f4f4f5',
          primary: '#18181b',
          onPrimary: '#ffffff',
          secondary: '#52525b',
          onSecondary: '#ffffff',
          accent: '#71717a',
          onAccent: '#ffffff',
          error: '#ef4444',
          onError: '#ffffff',
          info: '#3b82f6',
          onInfo: '#ffffff',
          success: '#22c55e',
          onSuccess: '#ffffff',
          warning: '#f59e0b',
          onWarning: '#ffffff',
          black: '#18181b',
          onBlack: '#ffffff',
          white: '#ffffff',
          onWhite: '#18181b',
        },
      },
      dark: {
        dark: true,
        colors: {
          background: '#09090b',
          surface: '#18181b',
          'surface-light': '#27272a',
          primary: '#fafafa',
          onPrimary: '#09090b',
          secondary: '#a1a1aa',
          onSecondary: '#09090b',
          accent: '#71717a',
          onAccent: '#ffffff',
          error: '#ef4444',
          onError: '#ffffff',
          info: '#3b82f6',
          onInfo: '#ffffff',
          success: '#22c55e',
          onSuccess: '#ffffff',
          warning: '#f59e0b',
          onWarning: '#09090b',
          black: '#18181b',
          onBlack: '#ffffff',
          white: '#ffffff',
          onWhite: '#18181b',
        },
      },
    },
  },
  defaults: {
    global: {
      fontFamily: "'Outfit', sans-serif",
    },
    VBtn: {
      rounded: 'md',
      variant: 'flat',
      class: 'text-none font-weight-medium',
    },
    VCard: {
      rounded: 'lg',
      elevation: 0,
      class: 'border-subtle',
    },
    VDialog: {
      cardProps: {
        rounded: 'xl',
        class: 'glass-panel',
      },
    },
    VTextField: {
      variant: 'solo-filled',
      rounded: 'lg',
    },
  },
});

function syncThemeLocal(): void {
  syncTheme((resolved) => {
    vuetify.theme.global.name.value = resolved;
  });
}

syncThemeLocal();

window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', () => {
  if (getThemePreference() === 'system') {
    syncThemeLocal();
  }
});

void initSystemTheme((resolved) => {
  vuetify.theme.global.name.value = resolved;
});

const pinia = createPinia();
pinia.use(piniaPluginPersistedstate);

createApp(App).use(pinia).use(vuetify).use(i18n).mount('#app');

window.addEventListener('error', (event) => {
  console.error('[Siegu] Unhandled error:', event.error);
});

window.addEventListener('unhandledrejection', (event) => {
  console.error('[Siegu] Unhandled rejection:', event.reason);
});

// TEMP-DIAG-START (remove after visual verification)
if (import.meta.env.DEV) {
  const ctl = async () => {
    try {
      return (await (await fetch('http://127.0.0.1:8899/ctl')).json()) as Record<string, unknown>;
    } catch {
      return {};
    }
  };
  const post = (payload: unknown) =>
    fetch('http://127.0.0.1:8899/report', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload),
    }).catch(() => {});

  const poll = async () => {
    const cmd = await ctl();
    const theme = typeof cmd.theme === 'string' ? cmd.theme : null;
    if (theme && localStorage.getItem('siegu_theme') !== theme) {
      localStorage.setItem('siegu_theme', theme);
      location.reload();
      return;
    }
    if (
      cmd.dialog === 'clear-db' &&
      cmd.target !== 'chromium' &&
      !(window as Window & { __diagDialogDone?: boolean }).__diagDialogDone
    ) {
      (window as Window & { __diagDialogDone?: boolean }).__diagDialogDone = true;
      runDialogProbe(cmd);
    }
  };
  setInterval(poll, 2000);

  async function runDialogProbe(cmd: Record<string, unknown>): Promise<void> {
    await new Promise((r) => setTimeout(r, 1000));
    try {
      const { useUiStore } = await import('./stores/ui');
      const ui = useUiStore(pinia);
      if (ui.currentPage !== 'settings') ui.setPage('settings');
    } catch {}
    const findBtn = () =>
      (document.querySelector('.mdi-trash-can-outline')?.closest('.v-btn') as HTMLElement | null) ??
      null;
    let btn = findBtn();
    for (let i = 0; i < 20 && !btn; i++) {
      await new Promise((r) => setTimeout(r, 500));
      btn = findBtn();
    }
    if (!btn) {
      post({ type: 'dialog', ok: false, reason: 'no clear-db button found' });
      return;
    }
    btn.click();
    await new Promise((r) => setTimeout(r, 1200));
    const card = document.querySelector('.v-overlay .v-card');
    if (!card) {
      post({ type: 'dialog', ok: false, reason: 'dialog did not open' });
      return;
    }
    const probe = (el: Element | null, props: string[]) => {
      if (!el) return null;
      const s = getComputedStyle(el);
      const out: Record<string, string> = {};
      for (const p of props) out[p] = s.getPropertyValue(p);
      return out;
    };
    post({
      type: 'dialog',
      run: cmd.run,
      ok: true,
      theme: cmd.theme,
      card: probe(card, [
        'background-color',
        'color',
        'backdrop-filter',
        'border-color',
        'border-width',
      ]),
      cardItem: probe(document.querySelector('.v-overlay .v-card-item'), [
        'background-color',
        'color',
      ]),
      title: probe(document.querySelector('.v-overlay .v-card-title'), ['color', 'font-weight']),
      scrim: probe(document.querySelector('.v-overlay__scrim'), ['background-color', 'opacity']),
      overlays: document.querySelectorAll('.v-overlay').length,
    });
  }

  setTimeout(() => {
    const probe = (sel: string, props: string[]) => {
      const el = document.querySelector(sel);
      if (!el) return { sel, found: false };
      const s = getComputedStyle(el);
      const out: Record<string, string | boolean> = { sel, found: true };
      for (const p of props) out[p] = s.getPropertyValue(p);
      return out;
    };
    const root = getComputedStyle(document.documentElement);
    const vars: Record<string, string> = {};
    for (const name of [
      '--color-bg-primary',
      '--color-bg-surface',
      '--color-bg-zinc-100',
      '--color-text-primary',
      '--color-text-secondary',
      '--color-text-muted',
      '--color-text-btn',
      '--color-bg-btn',
      '--color-border-subtle',
      '--color-border-default',
      '--color-bg-field',
      '--color-bg-hover',
      '--v-theme-background',
      '--v-theme-surface',
      '--v-theme-surface-light',
      '--v-theme-on-surface',
      '--v-theme-primary',
      '--v-theme-on-primary',
    ]) {
      vars[name] = root.getPropertyValue(name);
    }
    const report = {
      href: location.href,
      title: document.title,
      themePref: localStorage.getItem('siegu_theme'),
      dataTheme: document.documentElement.dataset.theme,
      themes: [...document.documentElement.classList].filter((c) => c.includes('theme')),
      bodyBg: getComputedStyle(document.body).backgroundColor,
      vars,
      toolbar: probe('.v-app-bar', ['background-color', 'color']),
      dock: probe('.dock', ['background-color', 'border-radius']),
      card: probe('.v-card', ['background-color', 'border-color', 'border-width', 'border-radius']),
      mediaCard: probe('.media-card-wrapper', [
        'background-color',
        'border-color',
        'border-width',
        'border-radius',
      ]),
      borderSubtle: probe('.border-subtle', ['border-color', 'border-width']),
      btn: probe('.v-btn', ['background-color', 'color']),
      textField: probe('.v-text-field', ['background-color']),
      chip: probe('.v-chip', ['background-color', 'color']),
      loading: {
        pageLoading: !!document.querySelector('.page-loading'),
        spinners: document.querySelectorAll('.v-progress-circular').length,
        skeletons: document.querySelectorAll('.v-skeleton-loader').length,
      },
      counts: {
        cards: document.querySelectorAll('.v-card').length,
        buttons: document.querySelectorAll('.v-btn').length,
        mediaCards: document.querySelectorAll('.media-card-wrapper').length,
        imgs: document.querySelectorAll('img').length,
      },
    };
    fetch('http://127.0.0.1:8899/report', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(report),
    }).catch(() => {});
  }, 8000);
}
// TEMP-DIAG-END
