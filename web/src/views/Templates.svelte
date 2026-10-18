<script lang="ts">
  // The Templates tab (D-65): per shop, templates of usual items. A template is edited like a
  // list (open it), and new lists start from it.
  import { message } from '../lib/api/client'
  import type { ShoppingList } from '../lib/api/types/ShoppingList'
  import Icon from '../lib/components/Icon.svelte'
  import UpDown from '../lib/components/UpDown.svelte'
  import { router } from '../lib/router.svelte'
  import { byPosition, shopping } from '../lib/shopping.svelte'

  let error = $state('')
  if (!shopping.loaded) shopping.load().catch((e) => (error = message(e)))

  let addingTo = $state<string | null>(null)
  let name = $state('')
  // Templates shared with you, from someone else's shop.
  const shared = $derived(shopping.templates.filter((t) => !shopping.shop(t.shop_id)).sort(byPosition))

  async function create(e: Event, shopId: string) {
    e.preventDefault()
    const n = name.trim()
    if (!n) return
    const id = await shopping.createList(shopId, n, { kind: 'template' })
    name = ''
    addingTo = null
    if (id) router.go(`/list/${id}`)
  }

  async function newList(t: ShoppingList) {
    if (!t.shop_id) return
    const id = await shopping.createList(t.shop_id, t.name, { templateId: t.id })
    if (id) router.go(`/list/${id}`)
  }
</script>

{#snippet card(t: ShoppingList, siblings: ShoppingList[] | null)}
  <div class="card tpl">
    <a class="text" href="/list/{t.id}">
      <strong>{t.name}</strong>
      <span class="muted small">{shopping.itemsOf(t.id).length} item{shopping.itemsOf(t.id).length === 1 ? '' : 's'}{t.role !== 'owner' ? ` · ${t.owner_name}'s` : ''}</span>
    </a>
    {#if siblings && siblings.length > 1}
      <UpDown what="template {t.name}" index={siblings.findIndex((x) => x.id === t.id)} count={siblings.length} onmove={(to) => shopping.moveList(t, to)} />
    {/if}
    {#if shopping.shop(t.shop_id)}
      <button class="btn small" onclick={() => newList(t)} title="A new shopping list with these items"><Icon name="plus" size={16} /> New list</button>
    {/if}
  </div>
{/snippet}

<h1 class="title">Templates</h1>
<p class="muted small intro">
  The things you buy every time, per shop. Start a shopping list from one (<strong>New list</strong> here, or when making a list), or add some of its items to a
  list you already have.
</p>

{#if error}
  <p class="card pad">Could not load: {error}</p>
{:else if !shopping.loaded}
  <p class="muted" aria-busy="true">Loading…</p>
{:else if !shopping.shops.length && !shared.length}
  <p class="card pad muted">Make a shop on the <a href="/">Shopping</a> page first.</p>
{:else}
  {#each shopping.sortedShops as s (s.id)}
    {@const templates = shopping.templatesOf(s.id)}
    <section>
      <h2><span class="dot" style:background={s.color ?? undefined}></span>{s.name}</h2>
      {#each templates as t (t.id)}{@render card(t, templates)}{/each}
      {#if addingTo === s.id}
        <form class="row" onsubmit={(e) => create(e, s.id)}>
          <!-- svelte-ignore a11y_autofocus -->
          <input bind:value={name} placeholder="Template name, e.g. Weekly" aria-label="New template in {s.name}" maxlength="200" autofocus />
          <button class="btn primary" type="submit">Create</button>
          <button class="btn" type="button" onclick={() => (addingTo = null)}>Cancel</button>
        </form>
      {:else}
        <button class="link" onclick={() => ((addingTo = s.id), (name = ''))}><Icon name="plus" size={16} /> New template in {s.name}</button>
      {/if}
    </section>
  {/each}
  {#if shared.length}
    <section>
      <h2>Shared with me</h2>
      {#each shared as t (t.id)}{@render card(t, null)}{/each}
    </section>
  {/if}
{/if}

<style>
  .title {
    margin-bottom: 6px;
  }
  .intro {
    margin: 0 0 16px;
  }
  section {
    margin-bottom: 20px;
  }
  h2 {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0 0 8px;
  }
  .tpl {
    display: flex;
    align-items: center;
    gap: 6px;
    min-height: 56px;
    padding: 0 8px 0 0;
    margin-bottom: 8px;
  }
  .text {
    flex: 1;
    min-width: 0;
    min-height: 56px;
    padding: 8px 12px;
    display: flex;
    flex-direction: column;
    justify-content: center;
    color: var(--text);
    text-decoration: none;
    overflow-wrap: anywhere;
  }
  .row {
    display: flex;
    gap: 8px;
  }
  .link {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    min-height: 40px;
    padding: 0 8px;
    color: var(--accent);
    font-weight: 600;
    font-size: 14px;
  }
  .small {
    font-size: 13px;
  }
  .pad {
    padding: 20px;
  }
</style>
