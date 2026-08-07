import { resolve } from 'path'
import { defineConfig } from 'vitest/config'
import vue from '@vitejs/plugin-vue'

export default defineConfig({
  plugins: [vue()],
  resolve: {
    alias: {
      '@': resolve(__dirname, 'src'),
    },
  },
  test: {
    environment: 'happy-dom',
    globals: true,
    setupFiles: ['./vitest.setup.ts'],
    coverage: {
      provider: 'v8',
      reporter: ['text', 'json-summary'],
      exclude: [
        'src/**/*.test.ts',
        'src/**/*.d.ts',
        'src/**/__tests__/**',
        'src/main.ts',
        'src/env.d.ts',
        'src/locales/**',
      ],
      thresholds: {
        statements: 40,
        branches: 28,
        functions: 30,
        lines: 41,
      },
    },
  },
})
