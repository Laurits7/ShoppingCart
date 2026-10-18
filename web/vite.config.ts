/// <reference types="vitest/config" />
import { createHash } from 'node:crypto'
import { readFileSync } from 'node:fs'
import { svelte } from '@sveltejs/vite-plugin-svelte'
import { defineConfig, type Plugin } from 'vite'

// Writes dist/sw.js from sw/sw.js with the list of built files and a version that changes
// whenever they do, so phones pick up new versions.
function serviceWorker(): Plugin {
  const statics = ['/', '/manifest.webmanifest', '/favicon.svg', '/icons/icon.svg', '/icons/icon-192.png', '/icons/icon-512.png', '/icons/maskable-512.png', '/icons/apple-touch-icon.png']
  return {
    name: 'service-worker',
    apply: 'build',
    generateBundle(_, bundle) {
      const files = [...statics, ...Object.keys(bundle).filter((f) => f.startsWith('assets/')).map((f) => `/${f}`)]
      const version = createHash('sha256').update(files.join('\n')).digest('hex').slice(0, 12)
      const source = readFileSync(new URL('./sw/sw.js', import.meta.url), 'utf8')
        .replace('__VERSION__', version)
        .replace('__FILES__', JSON.stringify(files))
      this.emitFile({ type: 'asset', fileName: 'sw.js', source })
    },
  }
}

// In dev, Vite serves the UI and proxies the API to the Rust server (`make dev-server`).
const backend = process.env.SHOPPINGCART_BACKEND ?? 'http://127.0.0.1:3100'

export default defineConfig({
  plugins: [svelte(), serviceWorker()],
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
