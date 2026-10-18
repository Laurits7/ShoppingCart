// The installable app (SPEC §6.13): the service worker over HTTPS (it isn't allowed, or
// useful, over plain HTTP on a LAN), "a new version is ready", and the install prompt.

import { toast } from './toast.svelte'

type InstallEvent = Event & { prompt: () => Promise<void>; userChoice: Promise<{ outcome: string }> }

class Pwa {
  /** The browser offered to install the app (Chrome, Edge, Android). */
  installable = $state(false)
  #prompt: InstallEvent | null = null

  /** Running as an installed app. */
  get installed() {
    return matchMedia('(display-mode: standalone)').matches || (navigator as Navigator & { standalone?: boolean }).standalone === true
  }

  /** iOS Safari: installing is Share → Add to Home Screen, there's no prompt. */
  get ios() {
    return /iPad|iPhone|iPod/.test(navigator.userAgent) && !this.installed
  }

  start() {
    addEventListener('beforeinstallprompt', (e) => {
      e.preventDefault()
      this.#prompt = e as InstallEvent
      this.installable = true
    })
    addEventListener('appinstalled', () => {
      this.installable = false
      this.#prompt = null
    })
    if (location.protocol !== 'https:' || !('serviceWorker' in navigator)) return
    navigator.serviceWorker
      .register('/sw.js')
      .then((reg) => {
        const offer = (w: ServiceWorker) =>
          toast('A new version of ShoppingCart is ready', 'info', {
            label: 'Reload',
            run: () => w.postMessage('update'),
          })
        if (reg.waiting && navigator.serviceWorker.controller) offer(reg.waiting)
        reg.addEventListener('updatefound', () => {
          const w = reg.installing
          w?.addEventListener('statechange', () => {
            if (w.state === 'installed' && navigator.serviceWorker.controller) offer(w)
          })
        })
        // Look for a new version now and then while the app stays open.
        setInterval(() => reg.update().catch(() => {}), 60 * 60 * 1000)
      })
      .catch(() => {})
    let reloading = false
    navigator.serviceWorker.addEventListener('controllerchange', () => {
      if (reloading) return
      reloading = true
      location.reload()
    })
  }

  async install() {
    if (!this.#prompt) return
    await this.#prompt.prompt()
    await this.#prompt.userChoice.catch(() => {})
    this.#prompt = null
    this.installable = false
  }

  /** Signed out: the service worker forgets cached pictures. */
  signedOut() {
    navigator.serviceWorker?.controller?.postMessage('signed-out')
  }
}

export const pwa = new Pwa()
