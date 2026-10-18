<script lang="ts">
  import { api, message } from '../../lib/api/client'
  import type { ApiToken } from '../../lib/api/types/ApiToken'
  import { toast } from '../../lib/toast.svelte'

  let tokens = $state<ApiToken[]>([])
  let name = $state('')
  let scope = $state<'read' | 'write'>('read')
  let created = $state<string | null>(null)

  api.get<ApiToken[]>('/tokens').then(
    (t) => (tokens = t),
    (e) => toast(message(e), 'error'),
  )

  async function create(e: Event) {
    e.preventDefault()
    try {
      const r = await api.post<{ token: string; info: ApiToken }>('/tokens', { name, scope })
      tokens = [...tokens, r.info]
      created = r.token
      name = ''
    } catch (err) {
      toast(message(err), 'error')
    }
  }

  async function revoke(t: ApiToken) {
    if (!confirm(`Revoke "${t.name}"? Apps using it stop working.`)) return
    try {
      await api.del(`/tokens/${t.id}`)
      tokens = tokens.filter((x) => x.id !== t.id)
    } catch (err) {
      toast(message(err), 'error')
    }
  }

  async function copy() {
    try {
      await navigator.clipboard.writeText(created!)
      toast('Copied')
    } catch {
      toast('Select the token and copy it by hand', 'error')
    }
  }
</script>

<section class="card settings-section">
  <h2>API tokens</h2>
  <p class="muted small">
    For other apps such as Streamline, Home Assistant or scripts. A <em>read</em> token can only look; a <em>write</em> token can
    also change things.
  </p>
  {#if created}
    <div class="new" role="status">
      <p><strong>Copy this token now.</strong> It won't be shown again.</p>
      <div class="row"><input readonly value={created} aria-label="New token" onfocus={(e) => e.currentTarget.select()} /><button class="btn" onclick={copy}>Copy</button></div>
      <button class="btn small" onclick={() => (created = null)}>Done</button>
    </div>
  {/if}
  <form class="row" onsubmit={create}>
    <input aria-label="Token name" placeholder="Name, e.g. Streamline" maxlength="100" bind:value={name} required />
    <select aria-label="Scope" bind:value={scope}>
      <option value="read">Read only</option>
      <option value="write">Read and write</option>
    </select>
    <button class="btn" type="submit">Create</button>
  </form>
  {#if tokens.length}
    <ul>
      {#each tokens as t (t.id)}
        <li>
          <span>
            {t.name} <span class="muted small">· {t.scope === 'write' ? 'read and write' : 'read only'}</span>
            <br /><span class="muted small">{t.last_used_at ? `Last used ${new Date(t.last_used_at).toLocaleDateString()}` : 'Never used'}</span>
          </span>
          <button class="btn small danger" onclick={() => revoke(t)}>Revoke</button>
        </li>
      {/each}
    </ul>
  {/if}
</section>

<style>
  .new {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 12px;
    border-radius: var(--radius-sm);
    background: var(--accent-soft);
  }
  .new input {
    font-family: ui-monospace, monospace;
    font-size: 13px;
  }
  .new .btn.small {
    align-self: flex-start;
  }
</style>
