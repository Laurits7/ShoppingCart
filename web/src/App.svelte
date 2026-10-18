<script lang="ts">
  import { live } from './lib/announce.svelte'
  import { api, ApiError, onUnauthorized } from './lib/api/client'
  import Icon, { type IconName } from './lib/components/Icon.svelte'
  import Toasts from './lib/components/Toasts.svelte'
  import { match, router } from './lib/router.svelte'
  import { session } from './lib/session.svelte'
  import { connect, disconnect } from './lib/live'
  import { pwa } from './lib/pwa.svelte'
  import { clearSnapshot, loadSnapshot } from './lib/snapshot'
  import { shopping } from './lib/shopping.svelte'
  import Login from './views/Login.svelte'
  import Shopping from './views/Shopping.svelte'

  // Only the Shopping tab is in the first download; other pages load when opened (SPEC §4.1).
  const settings = () => import('./views/Settings.svelte')
  const catalogSettings = () => import('./views/settings/Catalog.svelte')
  const wishlist = () => import('./views/Wishlist.svelte')
  const templates = () => import('./views/Templates.svelte')

  let phase = $state<'loading' | 'login' | 'setup' | 'app' | 'error'>('loading')

  async function boot() {
    phase = 'loading'
    shopping.reset()
    try {
      await session.load()
      // Send what was waiting from last time, and listen for changes from elsewhere.
      shopping.outbox.start(session.me!.id)
      connect(() => shopping.sync())
      phase = 'app'
    } catch (e) {
      // No connection: start from what was saved on this device, if anything (SPEC §6.7).
      if (e instanceof ApiError && e.status === 0) {
        const snap = await loadSnapshot()
        if (snap) {
          session.me = snap.me
          shopping.outbox.online = false
          shopping.apply(snap.data)
          shopping.outbox.start(snap.me.id)
          connect(() => shopping.sync())
          phase = 'app'
          return
        }
      }
      if (e instanceof ApiError && e.status === 401) {
        try {
          const s = await api.get<{ needs_setup: boolean }>('/setup')
          phase = s.needs_setup ? 'setup' : 'login'
        } catch {
          phase = 'error'
        }
      } else {
        phase = 'error'
      }
    }
  }

  boot()
  pwa.start()
  onUnauthorized(() => {
    if (phase === 'app') {
      // Changes not sent yet stay queued for after signing in again.
      disconnect()
      session.me = null
      shopping.reset()
      phase = 'login'
    }
  })

  const route = $derived(match(router.path))

  // Back online: send what's waiting and catch up.
  window.addEventListener('online', () => {
    shopping.outbox.retry()
    shopping.sync()
  })
  const offline = $derived(!shopping.outbox.online)

  // After moving to another page: say where we are (title) and move focus to its heading,
  // so screen readers announce the new page.
  let firstPage = true
  $effect(() => {
    const name = route.name
    void router.path
    document.title = `${{ shopping: 'Shopping', templates: 'Templates', wishlist: 'Wishlist', settings: 'Settings', notfound: 'Not found' }[name]} · ShoppingCart`
    if (firstPage) {
      firstPage = false
      return
    }
    requestAnimationFrame(() => {
      const h = document.querySelector<HTMLElement>('main h1')
      if (!h) return
      h.tabIndex = -1
      h.focus({ preventScroll: true })
    })
  })
  const tabs: { path: string; name: string; label: string; icon: IconName }[] = [
    { path: '/', name: 'shopping', label: 'Shopping', icon: 'cart' },
    { path: '/templates', name: 'templates', label: 'Templates', icon: 'staples' },
    { path: '/wishlist', name: 'wishlist', label: 'Wishlist', icon: 'gift' },
    { path: '/settings', name: 'settings', label: 'Settings', icon: 'settings' },
  ]
</script>

{#if phase === 'loading'}
  <div class="center muted" aria-busy="true">Loading…</div>
{:else if phase === 'error'}
  <div class="center">
    <p>Could not reach the server.</p>
    <button class="btn" onclick={boot}>Try again</button>
  </div>
{:else if phase === 'login' || phase === 'setup'}
  <Login setup={phase === 'setup'} onlogin={boot} />
{:else}
  <div class="shell">
    <nav aria-label="Main">
      <div class="brand"><img src="/icons/icon.svg" width="28" height="28" alt="" /> ShoppingCart</div>
      {#each tabs as t (t.name)}
        <a href={t.path} class:active={route.name === t.name} aria-current={route.name === t.name ? 'page' : undefined}>
          <Icon name={t.icon} />
          <span>{t.label}</span>
        </a>
      {/each}
    </nav>
    <main>
      {#if offline}
        <div class="offline" role="status">
          Offline{#if shopping.outbox.pending}: {shopping.outbox.pending} change{shopping.outbox.pending === 1 ? '' : 's'} waiting{/if}. Keep going; they're sent when the connection is back.
        </div>
      {/if}
      {#if route.name === 'shopping'}
        <Shopping listId={route.listId} />
      {:else if route.name === 'templates'}
        {#await templates() then m}<m.default />{/await}
      {:else if route.name === 'wishlist'}
        {#await wishlist() then m}<m.default wishlistId={route.wishlistId} />{/await}
      {:else if route.name === 'settings'}
        {#if route.page === 'catalog'}
          {#await catalogSettings() then m}<m.default />{/await}
        {:else}
          {#await settings() then m}<m.default onlogout={() => (disconnect(), clearSnapshot(), pwa.signedOut(), (session.me = null), shopping.reset(true), (phase = 'login'))} />{/await}
        {/if}
      {:else}
        <h1>Not found</h1>
        <p><a href="/">Go to your lists</a></p>
      {/if}
    </main>
  </div>
{/if}
<Toasts />
<div id="live-region" class="sr-only" aria-live="assertive">{live.message}</div>
<!-- How keyboard moves work, for every drag grip (aria-describedby). -->
<p id="dnd-help" class="sr-only">To move it, press Space, then the up and down arrows, and Space again to drop it.</p>

<style>
  .offline {
    position: sticky;
    top: 0;
    z-index: 5;
    margin: -8px 0 12px;
    padding: 8px 12px;
    border-radius: var(--radius-sm);
    background: var(--star-soft);
    color: var(--text);
    font-size: 14px;
  }
  main :global(h1:focus) {
    outline: none;
  }
  .center {
    min-height: 100dvh;
    display: grid;
    place-content: center;
    gap: 12px;
    text-align: center;
  }
  .shell {
    min-height: 100dvh;
  }
  main {
    max-width: 760px;
    margin: 0 auto;
    padding: 20px 16px calc(var(--nav-h) + 32px + env(safe-area-inset-bottom));
  }
  nav {
    position: fixed;
    inset: auto 0 0 0;
    height: calc(var(--nav-h) + env(safe-area-inset-bottom));
    padding-bottom: env(safe-area-inset-bottom);
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    background: var(--surface);
    border-top: 1px solid var(--border);
    z-index: 10;
  }
  .brand {
    display: none;
  }
  nav a {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 2px;
    font-size: 12px;
    font-weight: 600;
    color: var(--muted);
    text-decoration: none;
  }
  nav a.active {
    color: var(--accent);
  }
  @media (min-width: 900px) {
    nav {
      inset: 0 auto 0 0;
      width: var(--sidebar-w);
      height: auto;
      display: flex;
      flex-direction: column;
      gap: 4px;
      padding: 20px 12px;
      border-top: 0;
      border-right: 1px solid var(--border);
    }
    .brand {
      display: flex;
      align-items: center;
      gap: 10px;
      font-weight: 700;
      font-size: 17px;
      padding: 0 8px 16px;
    }
    nav a {
      flex-direction: row;
      justify-content: flex-start;
      gap: 12px;
      padding: 10px 12px;
      border-radius: var(--radius-sm);
      font-size: 15px;
    }
    nav a.active {
      background: var(--accent-soft);
    }
    main {
      margin-left: calc(var(--sidebar-w) + max(0px, (100vw - var(--sidebar-w) - 760px) / 2));
      padding: 32px 32px 48px;
    }
  }
</style>
