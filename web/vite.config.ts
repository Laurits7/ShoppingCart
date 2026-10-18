/// <reference types="vitest/config" />
import { svelte } from '@sveltejs/vite-plugin-svelte'
import { defineConfig } from 'vite'

// In dev, Vite serves the UI and proxies the API to the Rust server (`make dev-server`).
const backend = process.env.SHOPPINGCART_BACKEND ?? 'http://127.0.0.1:3100'

export default defineConfig({
  plugins: [svelte()],
  server: {
    proxy: {
      '/api': { target: backend, changeOrigin: false },
    },
  },
  test: {
    environment: 'happy-dom',
    include: ['src/**/*.test.ts'],
  },
  // Svelte's browser build (runes, $state) also in tests.
  resolve: process.env.VITEST ? { conditions: ['browser'] } : undefined,
  build: {
    target: 'es2022',
    cssCodeSplit: false,
    reportCompressedSize: true,
  },
})
