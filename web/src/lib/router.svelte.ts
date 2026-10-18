// Tiny history-API router. Routes are matched in App.svelte.

class Router {
  path = $state(location.pathname)

  go(path: string, replace = false) {
    if (path === this.path) return
    if (replace) history.replaceState({}, '', path)
    else history.pushState({}, '', path)
    this.path = path
    window.scrollTo(0, 0)
  }
}

export const router = new Router()

window.addEventListener('popstate', () => (router.path = location.pathname))

// Intercept same-origin <a href="/..."> clicks so navigation stays client-side.
document.addEventListener('click', (e) => {
  if (e.defaultPrevented || e.button !== 0 || e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return
  const a = (e.target as Element).closest?.('a')
  if (!a || a.target || a.hasAttribute('download')) return
  const href = a.getAttribute('href')
  if (!href || !href.startsWith('/') || href.startsWith('//') || href.startsWith('/api/')) return
  e.preventDefault()
  router.go(href)
})

export type Route =
  | { name: 'shopping'; listId?: string }
  | { name: 'wishlist'; wishlistId?: string }
  | { name: 'settings'; page?: 'catalog' }
  | { name: 'notfound' }

export function match(path: string): Route {
  const p = path.replace(/\/+$/, '') || '/'
  if (p === '/' || p === '/shopping') return { name: 'shopping' }
  const list = /^\/list\/([0-9A-Za-z]{26})$/.exec(p)
  if (list) return { name: 'shopping', listId: list[1].toUpperCase() }
  if (p === '/wishlist') return { name: 'wishlist' }
  const wl = /^\/wishlist\/([0-9A-Za-z]{26})$/.exec(p)
  if (wl) return { name: 'wishlist', wishlistId: wl[1].toUpperCase() }
  if (p === '/settings') return { name: 'settings' }
  if (p === '/settings/catalog') return { name: 'settings', page: 'catalog' }
  return { name: 'notfound' }
}
