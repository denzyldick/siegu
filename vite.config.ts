import { execSync } from 'child_process'
import { resolve } from 'path'
import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'

const host = process.env.TAURI_DEV_HOST

function resolveCommitSha(): string {
  if (process.env.APP_COMMIT_SHA) return process.env.APP_COMMIT_SHA;
  try {
    return execSync('git rev-parse --short=9 HEAD').toString().trim();
  } catch {
    return 'dev';
  }
}

export default defineConfig(async () => ({
  plugins: [vue()],

  define: {
    __APP_COMMIT_SHA__: JSON.stringify(resolveCommitSha()),
  },

  resolve: {
    alias: {
      '@': resolve(__dirname, 'src'),
      shared: resolve(__dirname, 'shared'),
    },
  },

  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: 'ws',
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      ignored: ['**/src-tauri/**'],
    },
    // Browser (webHost) dev ergonomics: proxy the Rust host's data-plane routes
    // so the full app in a browser resolves /session, /rpc, /thumb and /media
    // same-origin (Mode A, #24/#26/#28). Desktop (Tauri) is unaffected.
    proxy: process.env.SIEGU_WEB_HOST
      ? {
          '/session': { target: process.env.SIEGU_WEB_HOST },
          '/rpc': { target: process.env.SIEGU_WEB_HOST },
          '/thumb': { target: process.env.SIEGU_WEB_HOST },
          '/media': { target: process.env.SIEGU_WEB_HOST },
        }
      : undefined,
  },
}))
