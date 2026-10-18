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

export type Route = { name: 'shopping' } | { name: 'wishlist' } | { name: 'settings' } | { name: 'notfound' }

export function match(path: string): Route {
  const p = path.replace(/\/+$/, '') || '/'
  if (p === '/' || p === '/shopping') return { name: 'shopping' }
  if (p === '/wishlist') return { name: 'wishlist' }
  if (p === '/settings') return { name: 'settings' }
  return { name: 'notfound' }
}
