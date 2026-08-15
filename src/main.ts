import { createApp } from 'vue';
import { createPinia } from 'pinia';
import piniaPluginPersistedstate from 'pinia-plugin-persistedstate';
import App from './App.vue';
import 'vuetify/styles';
import '@mdi/font/css/materialdesignicons.css';

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
