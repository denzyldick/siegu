import { createApp } from 'vue'
import { createPinia } from 'pinia'
import piniaPluginPersistedstate from 'pinia-plugin-persistedstate'
import App from './App.vue'
import 'vuetify/styles'
import '@mdi/font/css/materialdesignicons.css'

import * as components from 'vuetify/components'
import { createVuetify } from 'vuetify/dist/vuetify.js'
import { createI18n } from 'vue-i18n'
import en from './locales/en.json'
import nl from './locales/nl.json'
import fr from './locales/fr.json'
import es from './locales/es.json'
import pap from './locales/pap.json'
import de from './locales/de.json'
import it from './locales/it.json'
import pt from './locales/pt.json'

import './styles/variables.css'
import './styles/base.css'
import './styles/animations.css'

const savedLang: string = localStorage.getItem('siegu_language') || 'en'

const i18n = createI18n({
  legacy: false,
  locale: savedLang,
  fallbackLocale: 'en',
  messages: { en, nl, fr, es, pap, de, it, pt },
})

function resolveTheme(): string {
  const pref: string = localStorage.getItem('siegu_theme') || 'system'
  if (pref === 'light') return 'light'
  if (pref === 'dark') return 'dark'
  return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light'
}

function syncDataTheme(): void {
  document.documentElement.dataset.theme = resolveTheme()
}

syncDataTheme()

window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', () => {
  if ((localStorage.getItem('siegu_theme') || 'system') === 'system') {
    syncDataTheme()
  }
})

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
          primary: '#18181b',
          secondary: '#52525b',
          accent: '#71717a',
          error: '#ef4444',
          info: '#3b82f6',
          success: '#22c55e',
          warning: '#f59e0b',
        },
      },
      dark: {
        dark: true,
        colors: {
          background: '#09090b',
          surface: '#18181b',
          primary: '#fafafa',
          secondary: '#a1a1aa',
          accent: '#71717a',
          error: '#ef4444',
          info: '#3b82f6',
          success: '#22c55e',
          warning: '#f59e0b',
        },
      },
    },
  },
  defaults: {
    global: {
      fontFamily: "'Inter', sans-serif",
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
})

const pinia = createPinia()
pinia.use(piniaPluginPersistedstate)

createApp(App).use(pinia).use(vuetify).use(i18n).mount('#app')

window.addEventListener('error', (event) => {
  console.error('[Siegu] Unhandled error:', event.error)
})

window.addEventListener('unhandledrejection', (event) => {
  console.error('[Siegu] Unhandled rejection:', event.reason)
})
