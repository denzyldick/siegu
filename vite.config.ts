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
  },
}))
