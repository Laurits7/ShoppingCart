<script lang="ts">
  import type { ShoppingList } from '../../lib/api/types/ShoppingList'
  import Icon from '../../lib/components/Icon.svelte'
  import Sheet from '../../lib/components/Sheet.svelte'
  import { router } from '../../lib/router.svelte'
  import { shopping } from '../../lib/shopping.svelte'
  import ShareSheet from './ShareSheet.svelte'

  let { open = $bindable(false), list }: { open: boolean; list: ShoppingList } = $props()
  const shop = $derived(shopping.shop(list.shop_id))
  const owner = $derived(list.role === 'owner')
  let listName = $state('')
  let shopName = $state('')
  let newCategory = $state('')
  let shareOpen = $state(false)

  $effect(() => {
    if (open) {
      listName = list.name
      shopName = shop?.name ?? ''
    }
  })

  function renameList() {
    const n = listName.trim()
    if (n && n !== list.name) shopping.patchList(list, { name: n })
  }
  function renameShop() {
    const n = shopName.trim()
    if (shop && n && n !== shop.name) shopping.patchShop(shop, { name: n })
  }

  async function deleteList() {
    const n = shopping.itemsOf(list.id).length
    if (!confirm(`Delete the list "${list.name}"${n ? ` and its ${n} item${n === 1 ? '' : 's'}` : ''}?`)) return
    open = false
    await shopping.deleteList(list)
    router.go('/', true)
  }

  async function deleteShop() {
    if (!shop) return
    const lists = shopping.lists.filter((l) => l.shop_id === shop.id)
    const items = lists.reduce((n, l) => n + shopping.itemsOf(l.id).length, 0)
    const what = `${lists.length} list${lists.length === 1 ? '' : 's'} and ${items} item${items === 1 ? '' : 's'}`
    if (!confirm(`Delete the shop "${shop.name}" with its ${what} and its staples?`)) return
    open = false
    await shopping.deleteShop(shop)
    router.go('/', true)
  }

  function addCategory(e: Event) {
    e.preventDefault()
    const n = newCategory.trim()
    if (!n) return
    shopping.createCategory(list.id, n)
    newCategory = ''
  }
</script>

<Sheet bind:open title="List and shop">
  <section>
    <h3>Sharing</h3>
    {#if owner}
      <p class="muted small">Private until you share it with people or a household.</p>
    {:else}
      <p class="muted small">{list.owner_name}'s list ({list.shop_name}); you {list.role === 'editor' ? 'can edit it' : 'can view it'}.</p>
    {/if}
    <div class="actions">
      <button class="btn small" onclick={() => ((open = false), (shareOpen = true))}>{owner ? 'Share…' : 'Who can see it…'}</button>
    </div>
  </section>
  {#if owner}
    <section>
      <h3>List</h3>
      <form class="row" onsubmit={(e) => (e.preventDefault(), renameList())}>
        <input bind:value={listName} aria-label="List name" maxlength="200" onblur={renameList} />
      </form>
      <div class="actions">
        {#if list.is_default}
          <span class="muted small"><Icon name="star" size={14} /> The default list of {shop?.name}: new items from elsewhere land here.</span>
        {:else}
          {#if !list.archived_at}<button class="btn small" onclick={() => shopping.patchList(list, { is_default: true })}>Make default</button>{/if}
          <button class="btn small" onclick={() => shopping.patchList(list, { archived: !list.archived_at })}>{list.archived_at ? 'Unarchive' : 'Archive'}</button>
          <button class="btn small danger" onclick={deleteList}>Delete list</button>
        {/if}
      </div>
    </section>
  {/if}

  {#if list.role !== 'viewer'}
    <section>
      <h3>Categories</h3>
      <p class="muted small">Items without a category are under Other. Deleting a category moves its items there.</p>
      <ul>
        {#each shopping.categoriesOf(list.id) as c (c.id)}
          <li>
            <input
              value={c.name}
              aria-label="Category name"
              maxlength="200"
              onchange={(e) => {
                const n = e.currentTarget.value.trim()
                if (n && n !== c.name) shopping.renameCategory(c, n)
              }}
            />
            <button class="icon-btn" aria-label="Delete category {c.name}" onclick={() => shopping.deleteCategory(c)}><Icon name="trash" size={18} /></button>
          </li>
        {/each}
      </ul>
      <form class="row" onsubmit={addCategory}>
        <input bind:value={newCategory} placeholder="New category" aria-label="New category" maxlength="200" />
        <button class="btn" type="submit">Add</button>
      </form>
    </section>
  {/if}

  {#if shop && owner}
    <section>
      <h3>Shop</h3>
      <form class="row" onsubmit={(e) => (e.preventDefault(), renameShop())}>
        <input bind:value={shopName} aria-label="Shop name" maxlength="200" onblur={renameShop} />
        <input
          type="color"
          class="color"
          aria-label="Shop colour"
          value={shop.color ?? '#15803d'}
          onchange={(e) => shopping.patchShop(shop, { color: e.currentTarget.value })}
        />
      </form>
      <div class="actions">
        {#if shop.color}<button class="btn small" onclick={() => shopping.patchShop(shop, { color: null })}>No colour</button>{/if}
        <span class="spacer"></span>
        <button class="btn small danger" onclick={deleteShop}>Delete shop</button>
      </div>
    </section>
  {/if}
</Sheet>

<ShareSheet bind:open={shareOpen} {list} />

<style>
  section {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding-bottom: 12px;
    border-bottom: 1px solid var(--border);
  }
  section:last-child {
    border-bottom: 0;
  }
  h3 {
    font-size: 15px;
    margin: 0;
  }
  p {
    margin: 0;
  }
  .small {
    font-size: 13px;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  li,
  .row {
    display: flex;
    gap: 6px;
    align-items: center;
  }
  .color {
    width: 52px;
    flex: none;
    padding: 4px;
  }
</style>
