<script lang="ts">
  // The Shopping tab: every shop with its lists and their items on one page, or one list
  // full screen (/list/{id}).
  import { message } from '../lib/api/client'
  import type { Item } from '../lib/api/types/Item'
  import type { ShoppingList } from '../lib/api/types/ShoppingList'
  import Icon from '../lib/components/Icon.svelte'
  import { router } from '../lib/router.svelte'
  import { shopping } from '../lib/shopping.svelte'
  import ItemSheet from './shopping/ItemSheet.svelte'
  import ListCard from './shopping/ListCard.svelte'
  import ListMenu from './shopping/ListMenu.svelte'
  import ListView from './shopping/ListView.svelte'
  import StaplesSheet from './shopping/StaplesSheet.svelte'

  let { listId }: { listId?: string } = $props()
  let error = $state('')
  let newShop = $state('')
  let newList = $state('')
  let addingListTo = $state<string | null>(null)
  let showArchived = $state<string | null>(null)

  // Sheets shared by all cards on the summary page.
  let editing = $state<Item | null>(null)
  let editOpen = $state(false)
  let editFocus = $state<'amount' | undefined>()
  let menuList = $state<ShoppingList | null>(null)
  let menuOpen = $state(false)
  let staplesList = $state<ShoppingList | null>(null)
  let staplesOpen = $state(false)

  const edit = (item: Item, focus?: 'amount') => ((editing = item), (editFocus = focus), (editOpen = true))
  const menu = (l: ShoppingList) => ((menuList = l), (menuOpen = true))
  const staples = (l: ShoppingList) => ((staplesList = l), (staplesOpen = true))

  if (!shopping.loaded) shopping.load().catch((e) => (error = message(e)))

  const current = $derived(listId ? (shopping.list(listId) ?? null) : null)
  const shop = $derived(current ? shopping.shop(current.shop_id) : undefined)
  // Lists shared with you sit under their owner's shop, which isn't one of yours (Phase 4).
  const shared = $derived(shopping.lists.filter((l) => !l.archived_at && !shopping.shop(l.shop_id)))

  async function createShop(e: Event) {
    e.preventDefault()
    const name = newShop.trim()
    if (!name) return
    await shopping.createShop(name)
    newShop = ''
  }

  async function createList(e: Event, shopId: string) {
    e.preventDefault()
    const name = newList.trim()
    if (!name) return
    await shopping.createList(shopId, name)
    newList = ''
    addingListTo = null
  }
</script>

{#if error}
  <div class="card pad">
    <p>Could not load your lists: {error}</p>
    <button class="btn" onclick={() => ((error = ''), shopping.load().catch((e) => (error = message(e))))}>Try again</button>
  </div>
{:else if !shopping.loaded}
  <p class="muted" aria-busy="true">Loading…</p>
{:else if listId}
  {#if current}
    <a class="back" href="/"><Icon name="chevronRight" size={18} /> All lists</a>
    <header class="list-head">
      <span class="titles">
        <span class="shop"><span class="dot" style:background={shop?.color ?? undefined}></span>{shop?.name ?? 'Shared list'}</span>
        <h1>{current.name}{#if current.archived_at}<span class="muted small"> (archived)</span>{/if}</h1>
      </span>
      <span class="tools">
        {#if current.role === 'owner'}<button class="icon-btn" aria-label="Staples" title="Staples" onclick={() => staples(current)}><Icon name="staples" /></button>{/if}
        <button class="icon-btn" aria-label="List and shop settings" title="List and shop" onclick={() => menu(current)}><Icon name="more" /></button>
      </span>
    </header>
    {#key current.id}
      <ListView list={current} />
    {/key}
  {:else}
    <h1>List not found</h1>
    <p class="muted">It may have been deleted. <a href="/">Go to all lists</a></p>
  {/if}
{:else}
  <h1 class="page-title">Shopping</h1>
  {#if !shopping.shops.length && !shared.length}
    <div class="card pad">
      <p><strong>Where do you shop?</strong> Start with one shop, say your supermarket. It gets a shopping list right away; add more shops and lists any time.</p>
    </div>
  {/if}

  {#each shopping.sortedShops as s (s.id)}
    {@const archived = shopping.listsOf(s.id, true)}
    <section class="shop-block">
      <h2><span class="dot" style:background={s.color ?? undefined}></span>{s.name}</h2>
      {#each shopping.listsOf(s.id) as l (l.id)}
        <ListCard list={l} onedit={edit} onstaples={staples} onmenu={menu} />
      {/each}
      <div class="shop-foot">
        {#if addingListTo === s.id}
          <form class="row" onsubmit={(e) => createList(e, s.id)}>
            <!-- svelte-ignore a11y_autofocus -->
            <input bind:value={newList} placeholder="List name, e.g. Party on Saturday" aria-label="New list in {s.name}" maxlength="200" autofocus />
            <button class="btn" type="submit">Add</button>
            <button class="btn" type="button" onclick={() => (addingListTo = null)}>Cancel</button>
          </form>
        {:else}
          <button class="link" onclick={() => ((addingListTo = s.id), (newList = ''))}><Icon name="plus" size={16} /> New list in {s.name}</button>
        {/if}
        {#if archived.length}
          <button class="link muted" onclick={() => (showArchived = showArchived === s.id ? null : s.id)}>{showArchived === s.id ? 'Hide' : 'Show'} archived ({archived.length})</button>
        {/if}
      </div>
      {#if showArchived === s.id}
        <ul class="archived">
          {#each archived as l (l.id)}<li><a href="/list/{l.id}">{l.name}</a> <span class="muted">({shopping.openCount(l.id)})</span></li>{/each}
        </ul>
      {/if}
    </section>
  {/each}

  {#if shared.length}
    <section class="shop-block">
      <h2>Shared with me</h2>
      {#each shared as l (l.id)}<ListCard list={l} onedit={edit} onstaples={staples} onmenu={menu} />{/each}
    </section>
  {/if}

  <form class="row new-shop" onsubmit={createShop}>
    <input bind:value={newShop} placeholder={shopping.shops.length ? 'New shop, e.g. Pharmacy' : 'Shop name, e.g. Rimi'} aria-label="New shop" maxlength="200" />
    <button class="btn" class:primary={!shopping.shops.length} type="submit">Create shop</button>
  </form>

  <ItemSheet item={editing} bind:open={editOpen} focus={editFocus} />
{/if}

{#if menuList}<ListMenu bind:open={menuOpen} list={menuList} />{/if}
{#if staplesList}<StaplesSheet bind:open={staplesOpen} list={staplesList} />{/if}

<style>
  .pad {
    padding: 20px;
    margin-bottom: 16px;
  }
  .pad p {
    margin: 0 0 8px;
  }
  .page-title {
    margin-bottom: 12px;
  }
  .row {
    display: flex;
    gap: 8px;
    flex: 1;
  }
  .shop-block {
    margin-bottom: 20px;
  }
  .shop-block h2 {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0 0 8px 4px;
  }
  .shop-foot {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px;
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
  .link.muted {
    color: var(--muted);
    font-weight: 500;
  }
  .archived {
    margin: 4px 0 0;
    padding-left: 28px;
  }
  .archived li {
    padding: 4px 0;
  }
  .new-shop {
    margin-top: 8px;
  }
  .back {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    font-size: 14px;
    font-weight: 600;
    text-decoration: none;
    margin-bottom: 6px;
  }
  .back :global(svg) {
    transform: rotate(180deg);
  }
  .list-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    margin-bottom: 14px;
  }
  .titles {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .shop {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 13px;
    font-weight: 600;
    color: var(--muted);
  }
  h1 {
    overflow-wrap: anywhere;
  }
  .small {
    font-size: 14px;
    font-weight: 500;
  }
  .tools {
    display: flex;
    flex: none;
  }
</style>
