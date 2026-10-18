<script lang="ts">
  import { api, ApiError, onUnauthorized } from './lib/api/client'
  import Icon, { type IconName } from './lib/components/Icon.svelte'
  import Toasts from './lib/components/Toasts.svelte'
  import { match, router } from './lib/router.svelte'
  import { session } from './lib/session.svelte'
  import Login from './views/Login.svelte'
  import Settings from './views/Settings.svelte'
  import Shopping from './views/Shopping.svelte'
  import Wishlist from './views/Wishlist.svelte'

  let phase = $state<'loading' | 'login' | 'setup' | 'app' | 'error'>('loading')

  async function boot() {
    phase = 'loading'
    try {
      await session.load()
      phase = 'app'
    } catch (e) {
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
  onUnauthorized(() => {
    if (phase === 'app') {
      session.me = null
      phase = 'login'
    }
  })

  const route = $derived(match(router.path))
  const tabs: { path: string; name: string; label: string; icon: IconName }[] = [
    { path: '/', name: 'shopping', label: 'Shopping', icon: 'cart' },
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
      {#if route.name === 'shopping'}
        <Shopping />
      {:else if route.name === 'wishlist'}
        <Wishlist />
      {:else if route.name === 'settings'}
        <Settings onlogout={() => ((session.me = null), (phase = 'login'))} />
      {:else}
        <h1>Not found</h1>
        <p><a href="/">Go to your lists</a></p>
      {/if}
    </main>
  </div>
{/if}
<Toasts />

<style>
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
    grid-template-columns: repeat(3, 1fr);
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
