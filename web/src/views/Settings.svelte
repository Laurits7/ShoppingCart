<script lang="ts">
  import { api, message } from '../lib/api/client'
  import Icon from '../lib/components/Icon.svelte'
  import { session } from '../lib/session.svelte'
  import { toast } from '../lib/toast.svelte'
  import Households from './settings/Households.svelte'
  import Profile from './settings/Profile.svelte'
  import Security from './settings/Security.svelte'
  import Tokens from './settings/Tokens.svelte'
  import Users from './settings/Users.svelte'

  let { onlogout }: { onlogout: () => void } = $props()
  let version = $state('')
  api.get<{ version: string }>('/health').then((h) => (version = h.version), () => {})

  async function logout() {
    try {
      await api.post('/auth/logout')
      onlogout()
    } catch (e) {
      toast(message(e), 'error')
    }
  }
</script>

<div class="head">
  <h1>Settings</h1>
  <button class="btn" onclick={logout}><Icon name="logout" size={18} /> Sign out</button>
</div>

<Profile />
<Households />
<Security />
<Tokens />
{#if session.me?.is_admin}<Users />{/if}

<section class="card about">
  <h2>About</h2>
  <p class="muted">
    ShoppingCart {version} · <a href="/api/docs" target="_blank" rel="noopener">API documentation</a> ·
    <a href="https://laurits7.github.io/ShoppingCart/" target="_blank" rel="noopener noreferrer">User guide</a>
  </p>
</section>

<style>
  .head {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 16px;
  }
  .about {
    padding: 20px;
  }
  .about p {
    margin: 8px 0 0;
  }
  :global(.settings-section) {
    padding: 20px;
    margin-bottom: 16px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  :global(.settings-section p) {
    margin: 0;
  }
  :global(.settings-section .row) {
    display: flex;
    gap: 8px;
    align-items: center;
    flex-wrap: wrap;
  }
  :global(.settings-section .row > input),
  :global(.settings-section .row > select) {
    flex: 1 1 180px;
    width: auto;
  }
  :global(.settings-section ul) {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
  }
  :global(.settings-section li) {
    display: flex;
    gap: 8px;
    align-items: center;
    justify-content: space-between;
    padding: 10px 0;
    border-top: 1px solid var(--border);
    flex-wrap: wrap;
  }
  :global(.settings-section label > span) {
    display: block;
    font-size: 13px;
    color: var(--muted);
    margin-bottom: 4px;
  }
  :global(.settings-section .small) {
    font-size: 13px;
  }
</style>
