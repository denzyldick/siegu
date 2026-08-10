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
