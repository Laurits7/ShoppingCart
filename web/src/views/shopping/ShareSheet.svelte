<script lang="ts">
  // Who can see a list and why (SPEC §6.8). The owner shares with people or households as
  // editor or viewer and revokes; someone it's shared with directly can leave it.
  import { api, message } from '../../lib/api/client'
  import type { Household } from '../../lib/api/types/Household'
  import type { Share } from '../../lib/api/types/Share'
  import type { ShoppingList } from '../../lib/api/types/ShoppingList'
  import type { UserSummary } from '../../lib/api/types/UserSummary'
  import Icon from '../../lib/components/Icon.svelte'
  import Sheet from '../../lib/components/Sheet.svelte'
  import { router } from '../../lib/router.svelte'
  import { session } from '../../lib/session.svelte'
  import { shopping } from '../../lib/shopping.svelte'
  import { toast } from '../../lib/toast.svelte'

  let { open = $bindable(false), list }: { open: boolean; list: ShoppingList } = $props()
  const owner = $derived(list.role === 'owner')
  let shares = $state<Share[]>([])
  let people = $state<UserSummary[]>([])
  let households = $state<Household[]>([])
  let target = $state('')
  let role = $state<'editor' | 'viewer'>('editor')
  let loading = $state(false)

  $effect(() => {
    if (open) load()
  })

  async function load() {
    loading = true
    try {
      ;[shares, people, households] = await Promise.all([
        api.get<Share[]>(`/lists/${list.id}/shares`),
        owner ? api.get<UserSummary[]>('/users/directory') : Promise.resolve([]),
        owner ? api.get<Household[]>('/households') : Promise.resolve([]),
      ])
    } catch (e) {
      toast(message(e), 'error')
    }
    loading = false
  }

  const me = $derived(session.me?.id)
  // Admins see every household; only the ones I'm in can be shared with.
  const myHouseholds = $derived(households.filter((h) => h.members.some((m) => m.user_id === me)))
  const candidates = $derived(people.filter((p) => p.id !== me && !shares.some((s) => s.principal_type === 'user' && s.principal_id === p.id)))
  const freeHouseholds = $derived(myHouseholds.filter((h) => !shares.some((s) => s.principal_type === 'household' && s.principal_id === h.id)))
  const mine = $derived(shares.find((s) => s.principal_type === 'user' && s.principal_id === me))

  async function add(e: Event) {
    e.preventDefault()
    if (!target) return
    const [principal_type, principal_id] = target.split(':')
    try {
      const s = await api.post<Share>(`/lists/${list.id}/shares`, { principal_type, principal_id, role })
      shares = [...shares.filter((x) => x.id !== s.id), s]
      target = ''
    } catch (err) {
      toast(message(err), 'error')
    }
  }

  async function setRole(s: Share, r: string) {
    try {
      const n = await api.patch<Share>(`/shares/${s.id}`, { role: r })
      shares = shares.map((x) => (x.id === n.id ? n : x))
    } catch (err) {
      toast(message(err), 'error')
    }
  }

  async function revoke(s: Share) {
    if (!confirm(`Stop sharing "${list.name}" with ${s.name}?`)) return
    try {
      await api.del(`/shares/${s.id}`)
      shares = shares.filter((x) => x.id !== s.id)
    } catch (err) {
      toast(message(err), 'error')
    }
  }

  async function leave() {
    if (!mine || !confirm(`Leave "${list.name}"? ${list.owner_name} can share it with you again.`)) return
    try {
      await api.del(`/shares/${mine.id}`)
      open = false
      await shopping.sync()
      router.go('/', true)
    } catch (err) {
      toast(message(err), 'error')
    }
  }
</script>

<Sheet bind:open title="Sharing {list.name}">
  <ul>
    <li>
      <span class="who"><strong>{owner ? 'You' : list.owner_name}</strong><span class="muted small">Owner</span></span>
    </li>
    {#each shares as s (s.id)}
      <li>
        <span class="who">
          <strong>{s.principal_type === 'household' ? `Household ${s.name}` : s.principal_id === me ? 'You' : s.name}</strong>
          <span class="muted small">
            {#if s.principal_type === 'household'}Everyone in it, now and later{#if s.members.length}: {s.members.join(', ')}{/if}{:else}Invited directly{/if}
          </span>
        </span>
        {#if owner}
          <select aria-label="Role of {s.name}" value={s.role} onchange={(e) => setRole(s, e.currentTarget.value)}>
            <option value="editor">Can edit</option>
            <option value="viewer">Can view</option>
          </select>
          <button class="icon-btn" aria-label="Stop sharing with {s.name}" onclick={() => revoke(s)}><Icon name="x" size={18} /></button>
        {:else}
          <span class="muted small role">{s.role === 'editor' ? 'Can edit' : 'Can view'}</span>
        {/if}
      </li>
    {/each}
  </ul>
  {#if loading && !shares.length}<p class="muted">Loading…</p>{/if}

  {#if owner}
    {#if candidates.length || freeHouseholds.length}
      <form class="add" onsubmit={add}>
        <select bind:value={target} aria-label="Share with">
          <option value="">Share with…</option>
          {#if freeHouseholds.length}
            <optgroup label="Households">
              {#each freeHouseholds as h (h.id)}<option value="household:{h.id}">{h.name} ({h.members.length})</option>{/each}
            </optgroup>
          {/if}
          {#if candidates.length}
            <optgroup label="People">
              {#each candidates as p (p.id)}<option value="user:{p.id}">{p.display_name} (@{p.username})</option>{/each}
            </optgroup>
          {/if}
        </select>
        <select bind:value={role} aria-label="Role">
          <option value="editor">Can edit</option>
          <option value="viewer">Can view</option>
        </select>
        <button class="btn primary" type="submit" disabled={!target}>Share</button>
      </form>
    {/if}
    <p class="muted small">
      Editors add, tick and change items and categories; viewers only look. Only you rename, delete or share the list. Stopping a share takes effect at once,
      also on phones that have the list open.
    </p>
  {:else if mine}
    <button class="btn danger" onclick={leave}>Leave this list</button>
  {:else}
    <p class="muted small">It's shared with you through a household; leave the household in Settings to stop seeing it.</p>
  {/if}
</Sheet>

<style>
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  li {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 0;
    border-top: 1px solid var(--border);
  }
  .who {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    overflow-wrap: anywhere;
  }
  li select {
    width: auto;
    min-height: 36px;
  }
  .add {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
  }
  .add select:first-child {
    flex: 1 1 180px;
    width: auto;
  }
  .add select {
    width: auto;
  }
  p {
    margin: 0;
  }
  .small {
    font-size: 13px;
  }
  .role {
    white-space: nowrap;
  }
</style>
