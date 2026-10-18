<script lang="ts">
  // Settings → Integrations (SPEC §6.10): Mealie as an inbox. The token is write-only.
  import { api, message } from '../../lib/api/client'
  import type { Integration } from '../../lib/api/types/Integration'
  import type { MealieList } from '../../lib/api/types/MealieList'
  import type { MealieSettings } from '../../lib/api/types/MealieSettings'
  import type { SyncOutcome } from '../../lib/api/types/SyncOutcome'
  import { shopping } from '../../lib/shopping.svelte'
  import { toast } from '../../lib/toast.svelte'

  let integrations = $state<Integration[]>([])
  let loaded = $state(false)
  let address = $state('')
  let token = $state('')
  let newToken = $state('')
  let lists = $state<MealieList[]>([])
  let listsError = $state('')
  let busy = $state(false)
  let form = $state<MealieSettings | null>(null)
  let target = $state('')

  const mealie = $derived(integrations.find((i) => i.provider === 'mealie'))

  async function load() {
    try {
      integrations = await api.get<Integration[]>('/integrations')
      if (!shopping.loaded) await shopping.load()
    } catch (e) {
      toast(message(e), 'error')
    }
    loaded = true
    if (mealie) edit(mealie)
  }
  load()

  function edit(i: Integration) {
    form = { ...i.settings }
    target = i.settings.target_list_id ?? ''
    loadLists(i)
  }

  async function loadLists(i: Integration) {
    listsError = ''
    try {
      lists = await api.get<MealieList[]>(`/integrations/${i.id}/mealie/lists`)
    } catch (e) {
      lists = []
      listsError = message(e)
    }
  }

  const replace = (i: Integration) => (integrations = integrations.map((x) => (x.id === i.id ? i : x)))

  async function connect(e: Event) {
    e.preventDefault()
    busy = true
    try {
      const i = await api.post<Integration>('/integrations', {
        provider: 'mealie',
        secret: token.trim(),
        settings: { base_url: address.trim(), target_list_id: null, labels_as_categories: true, create_categories: false, after_import: 'check' },
      })
      integrations = [...integrations, i]
      token = ''
      edit(i)
    } catch (err) {
      toast(message(err), 'error')
    }
    busy = false
  }

  async function save(e: Event) {
    e.preventDefault()
    if (!mealie || !form) return
    const settings: MealieSettings = {
      ...form,
      list_name: lists.find((l) => l.id === form!.list_id)?.name ?? form.list_name,
      target_list_id: target || null,
    }
    const ready = !!settings.list_id
    try {
      const i = await api.patch<Integration>(`/integrations/${mealie.id}`, { settings, secret: newToken.trim() || undefined, enabled: ready && (mealie.enabled || !mealie.last_sync_at) })
      replace(i)
      newToken = ''
      toast(ready ? 'Saved' : 'Saved; choose the Mealie list to switch it on')
      if (settings.base_url !== mealie.settings.base_url) loadLists(i)
    } catch (err) {
      toast(message(err), 'error')
    }
  }

  async function toggle() {
    if (!mealie) return
    try {
      replace(await api.patch<Integration>(`/integrations/${mealie.id}`, { enabled: !mealie.enabled }))
    } catch (err) {
      toast(message(err), 'error')
    }
  }

  async function syncNow() {
    if (!mealie) return
    busy = true
    try {
      const r = await api.post<SyncOutcome>(`/integrations/${mealie.id}/sync`)
      replace(r.integration)
      if (r.imported) toast(`Imported ${r.imported} item${r.imported === 1 ? '' : 's'} from Mealie`)
      if (r.error) toast(r.error, 'error')
      else if (!r.imported) toast('Nothing new in Mealie')
      shopping.sync()
    } catch (err) {
      toast(message(err), 'error')
    }
    busy = false
  }

  async function disconnect() {
    if (!mealie || !confirm('Disconnect Mealie? Its token is deleted here; items already imported stay.')) return
    try {
      await api.del(`/integrations/${mealie.id}`)
      integrations = integrations.filter((i) => i.id !== mealie.id)
      form = null
    } catch (err) {
      toast(message(err), 'error')
    }
  }

  const when = (ts: string | null) => (ts ? new Date(ts).toLocaleString() : 'never')
</script>

<section class="card settings-section">
  <h2>Mealie</h2>
  <p class="muted small">
    Use a <a href="https://mealie.io" target="_blank" rel="noopener noreferrer">Mealie</a> shopping list as an inbox: what you add there (e.g. a recipe's
    ingredients) moves into a list here every few minutes and is ticked in Mealie.
  </p>
  {#if !loaded}
    <p class="muted">Loading…</p>
  {:else if !mealie}
    <form class="fields" onsubmit={connect}>
      <label class="field wide"><span>Mealie's address</span><input bind:value={address} placeholder="http://mealie:9000" required /></label>
      <label class="field wide"><span>Mealie API token (your profile in Mealie → API Tokens)</span><input bind:value={token} type="password" autocomplete="off" required /></label>
      <div class="actions wide"><button class="btn primary" type="submit" disabled={busy}>Connect</button></div>
    </form>
  {:else if form}
    <p class="small">
      {#if mealie.last_error}<span class="error" role="status">Problem: {mealie.last_error}</span><br />{/if}
      <span class="muted">Last synced: {when(mealie.last_sync_at)}{mealie.enabled ? ` · next: ${when(mealie.next_run_at)}` : ' · switched off'}</span>
    </p>
    <form class="fields" onsubmit={save}>
      <label class="field wide"><span>Mealie's address</span><input bind:value={form.base_url} required /></label>
      <label class="field wide">
        <span>Mealie list to watch</span>
        <select bind:value={form.list_id} aria-describedby={listsError ? 'mealie-lists-error' : undefined}>
          <option value={null}>Choose…</option>
          {#each lists as l (l.id)}<option value={l.id}>{l.name}</option>{/each}
          {#if form.list_id && !lists.some((l) => l.id === form!.list_id)}<option value={form.list_id}>{form.list_name ?? form.list_id}</option>{/if}
        </select>
        {#if listsError}<span id="mealie-lists-error" class="error small" role="alert">{listsError}</span>{/if}
      </label>
      <label class="field wide">
        <span>Items go to</span>
        <select bind:value={target}>
          <option value="">Your inbox (assign them to lists there)</option>
          <optgroup label="Or straight to a list">
            {#each shopping.lists.filter((l) => l.kind === 'list' && l.role !== 'viewer' && !l.archived_at) as l (l.id)}<option value={l.id}>{l.shop_name} · {l.name}</option>{/each}
          </optgroup>
        </select>
      </label>
      <label class="check wide"><input type="checkbox" bind:checked={form.labels_as_categories} /> Mealie labels pick the category of the same name</label>
      <label class="check wide"><input type="checkbox" bind:checked={form.create_categories} disabled={!form.labels_as_categories} /> Create that category if the list doesn't have it</label>
      <label class="field wide">
        <span>Afterwards, in Mealie</span>
        <select bind:value={form.after_import}>
          <option value="check">Tick them (Mealie keeps them as done)</option>
          <option value="delete">Remove them</option>
          <option value="keep">Leave them</option>
        </select>
      </label>
      <label class="field wide"><span>New token (leave empty to keep the current one)</span><input bind:value={newToken} type="password" autocomplete="off" /></label>
      <div class="actions wide">
        <button class="btn primary" type="submit">Save</button>
        <button class="btn" type="button" onclick={syncNow} disabled={busy || !mealie.settings.list_id}>{busy ? 'Syncing…' : 'Sync now'}</button>
        <button class="btn" type="button" onclick={toggle}>{mealie.enabled ? 'Switch off' : 'Switch on'}</button>
        <span class="spacer"></span>
        <button class="btn danger" type="button" onclick={disconnect}>Disconnect</button>
      </div>
    </form>
  {/if}
</section>

<style>
  .error {
    color: var(--danger);
  }
  .check {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .check input {
    width: 20px;
    min-height: 20px;
    accent-color: var(--accent);
  }
</style>
