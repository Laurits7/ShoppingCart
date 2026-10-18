// Service worker (SPEC §6.7, §6.13), only registered over HTTPS. Keeps the app shell
// available offline; pictures are kept once seen. API data never goes through here: the
// app's outbox and its saved copy of the lists handle being offline. Generated at build time
// (vite.config.ts fills in VERSION and FILES).
const VERSION = '__VERSION__'
const SHELL = `shell-${VERSION}`
const PICTURES = 'pictures'
const FILES = __FILES__

self.addEventListener('install', (e) => {
  e.waitUntil(caches.open(SHELL).then((c) => c.addAll(FILES)))
})

self.addEventListener('activate', (e) => {
  e.waitUntil(
    caches
      .keys()
      .then((keys) => Promise.all(keys.filter((k) => k.startsWith('shell-') && k !== SHELL).map((k) => caches.delete(k))))
      .then(() => self.clients.claim()),
  )
})

self.addEventListener('message', (e) => {
  // The app asks for the new version when the user chooses to reload.
  if (e.data === 'update') self.skipWaiting()
  // Signed out: forget the pictures.
  if (e.data === 'signed-out') caches.delete(PICTURES)
})

const cacheFirst = async (req, cache) => {
  const hit = await caches.match(req)
  if (hit) return hit
  const res = await fetch(req)
  if (res.ok) (await caches.open(cache)).put(req, res.clone())
  return res
}

self.addEventListener('fetch', (e) => {
  const req = e.request
  const url = new URL(req.url)
  if (req.method !== 'GET' || url.origin !== location.origin) return
  // Pictures never change for an id.
  if (url.pathname.startsWith('/api/v1/images/')) return e.respondWith(cacheFirst(req, PICTURES))
  if (url.pathname.startsWith('/api/')) return
  // Pages: the network first (new version), the shell when offline.
  if (req.mode === 'navigate') {
    return e.respondWith(fetch(req).catch(() => caches.match('/', { ignoreSearch: true })))
  }
  e.respondWith(cacheFirst(req, SHELL))
})
