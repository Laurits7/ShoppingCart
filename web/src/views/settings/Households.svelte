<script lang="ts">
  import { api, message } from '../../lib/api/client'
  import type { Household } from '../../lib/api/types/Household'
  import type { UserSummary } from '../../lib/api/types/UserSummary'
  import { session } from '../../lib/session.svelte'
  import { toast } from '../../lib/toast.svelte'

  let households = $state<Household[]>([])
  let directory = $state<UserSummary[]>([])
  let newName = $state('')
  let adding = $state<Record<string, string>>({})

  async function load() {
    try {
      ;[households, directory] = await Promise.all([api.get<Household[]>('/households'), api.get<UserSummary[]>('/users/directory')])
    } catch (e) {
      toast(message(e), 'error')
    }
  }
  load()

  const me = () => session.me!.id
  const isOwner = (h: Household) => session.me!.is_admin || h.members.some((m) => m.user_id === me() && m.role === 'owner')
  const replace = (h: Household) => (households = households.map((x) => (x.id === h.id ? h : x)))

  async function create(e: Event) {
    e.preventDefault()
    try {
      households = [...households, await api.post<Household>('/households', { name: newName })]
      newName = ''
    } catch (err) {
      toast(message(err), 'error')
    }
  }

  async function rename(h: Household) {
    const name = prompt('New name', h.name)?.trim()
    if (!name || name === h.name) return
    try {
      replace(await api.patch<Household>(`/households/${h.id}`, { name }))
    } catch (err) {
      toast(message(err), 'error')
    }
  }

  async function addMember(h: Household) {
    const user_id = adding[h.id]
    if (!user_id) return
    try {
      replace(await api.post<Household>(`/households/${h.id}/members`, { user_id }))
      adding[h.id] = ''
    } catch (err) {
      toast(message(err), 'error')
    }
  }

  async function setRole(h: Household, user_id: string, role: 'owner' | 'member') {
    try {
      replace(await api.patch<Household>(`/households/${h.id}/members/${user_id}`, { role }))
    } catch (err) {
      toast(message(err), 'error')
    }
  }

  async function remove(h: Household, user_id: string) {
    const leaving = user_id === me()
    if (!confirm(leaving ? `Leave "${h.name}"? You lose access to everything shared with it.` : 'Remove this member? They lose access to everything shared with the household.')) return
    try {
      await api.del(`/households/${h.id}/members/${user_id}`)
      if (leaving && !session.me!.is_admin) households = households.filter((x) => x.id !== h.id)
      else load()
    } catch (err) {
      toast(message(err), 'error')
    }
  }

  async function del(h: Household) {
    if (!confirm(`Delete "${h.name}"? Everything shared with it stops being shared.`)) return
    try {
      await api.del(`/households/${h.id}`)
      households = households.filter((x) => x.id !== h.id)
    } catch (err) {
      toast(message(err), 'error')
    }
  }
</script>

<section class="card settings-section">
  <h2>Households</h2>
  <p class="muted small">Groups of people you share lists and wishlists with. Nothing is shared until you share it.</p>
  {#each households as h (h.id)}
    <div class="household">
      <div class="row between">
        <strong>{h.name}</strong>
        {#if isOwner(h)}
          <span class="row">
            <button class="btn small" onclick={() => rename(h)}>Rename</button>
            <button class="btn small danger" onclick={() => del(h)}>Delete</button>
          </span>
        {/if}
      </div>
      <ul>
        {#each h.members as m (m.user_id)}
          <li>
            <span>{m.display_name} <span class="muted small">@{m.username}{m.role === 'owner' ? ' · owner' : ''}</span></span>
            <span class="row">
              {#if isOwner(h) && m.user_id !== me()}
                <button class="btn small" onclick={() => setRole(h, m.user_id, m.role === 'owner' ? 'member' : 'owner')}>{m.role === 'owner' ? 'Make member' : 'Make owner'}</button>
                <button class="btn small danger" onclick={() => remove(h, m.user_id)}>Remove</button>
              {:else if m.user_id === me()}
                <button class="btn small" onclick={() => remove(h, m.user_id)}>Leave</button>
              {/if}
            </span>
          </li>
        {/each}
      </ul>
      {#if isOwner(h)}
        {@const candidates = directory.filter((u) => !h.members.some((m) => m.user_id === u.id))}
        {#if candidates.length}
          <div class="row">
            <select aria-label="Person to add to {h.name}" bind:value={adding[h.id]}>
              <option value="">Add a person…</option>
              {#each candidates as u (u.id)}<option value={u.id}>{u.display_name} (@{u.username})</option>{/each}
            </select>
            <button class="btn" onclick={() => addMember(h)} disabled={!adding[h.id]}>Add</button>
          </div>
        {/if}
      {/if}
    </div>
  {/each}
  <form class="row" onsubmit={create}>
    <input aria-label="New household name" placeholder="New household, e.g. Home" maxlength="100" bind:value={newName} required />
    <button class="btn" type="submit">Create</button>
  </form>
</section>

<style>
  .household {
    padding: 12px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .between {
    justify-content: space-between;
  }
</style>
