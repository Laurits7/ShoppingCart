<script lang="ts">
  import type { Item } from '../../lib/api/types/Item'
  import type { ShoppingList } from '../../lib/api/types/ShoppingList'
  import { collapsed } from '../../lib/collapsed.svelte'
  import Icon from '../../lib/components/Icon.svelte'
  import PictureSheet from '../../lib/components/PictureSheet.svelte'
  import { dnd, draggable, dropList, type DragItem } from '../../lib/dnd.svelte'
  import { shopping } from '../../lib/shopping.svelte'
  import ItemRow from './ItemRow.svelte'
  import ItemSheet from './ItemSheet.svelte'
  import MoveSheet from './MoveSheet.svelte'
  import QuickAdd from './QuickAdd.svelte'

  let { list }: { list: ShoppingList } = $props()
  const readonly = $derived(list.role === 'viewer')

  // While dragging, empty categories show up as places to drop into.
  const sections = $derived(shopping.sections(list.id, dnd.item?.kind === 'item' || dnd.item?.kind === 'category'))
  const named = $derived(sections.filter((s) => s.id !== 'other'))
  const other = $derived(sections.find((s) => s.id === 'other'))
  const cart = $derived(shopping.cartOf(list.id))

  let editing = $state<Item | null>(null)
  let editOpen = $state(false)
  let focus = $state<'amount' | undefined>()
  function edit(item: Item, f?: 'amount') {
    editing = item
    focus = f
    editOpen = true
  }

  let shown = $state<Item | null>(null)
  let pictureOpen = $state(false)
  const showPicture = (item: Item) => ((shown = item), (pictureOpen = true))
  const shownLook = $derived(shown ? shopping.look(shown) : null)

  // Picking several items to move them to another list.
  let picking = $state(false)
  let picked = $state<string[]>([])
  let moveOpen = $state(false)
  const pickedItems = $derived(shopping.itemsOf(list.id).filter((i) => picked.includes(i.id)))
  const pick = (item: Item) => (picked = picked.includes(item.id) ? picked.filter((p) => p !== item.id) : [...picked, item.id])
  const stopPicking = () => ((picking = false), (picked = []))

  const itemDrop = (categoryId: string | null) => ({
    accepts: (it: DragItem) => it.kind === 'item' && !readonly,
    drop: (it: DragItem, index: number) => {
      const item = shopping.items.find((i) => i.id === it.id)
      if (item) shopping.moveItem(item, list.id, categoryId, index)
    },
    keyMove: (id: string, index: number) => {
      const item = shopping.items.find((i) => i.id === id)
      if (item) shopping.moveItem(item, list.id, categoryId, index)
    },
  })

  /** A drop among the shown categories, as an index among all of the list's categories
   * (empty ones may be hidden). */
  function categoryIndex(id: string, index: number) {
    const all = shopping.categoriesOf(list.id).filter((c) => c.id !== id)
    const shown = named.map((s) => s.id).filter((c) => c !== id)
    if (index < shown.length) return all.findIndex((c) => c.id === shown[index])
    return shown.length ? all.findIndex((c) => c.id === shown[shown.length - 1]) + 1 : all.length
  }
  function moveCategory(id: string, index: number) {
    const c = shopping.categories.find((x) => x.id === id)
    if (c) shopping.moveCategory(c, categoryIndex(id, index))
  }
</script>

{#if !readonly}
  <QuickAdd listId={list.id} shopId={list.shop_id} />
  {#if sections.length}
    <div class="pickbar">
      {#if picking}
        <span class="muted">{picked.length} picked</span>
        <span class="spacer"></span>
        <button class="btn small primary" disabled={!picked.length} onclick={() => (moveOpen = true)}><Icon name="move" size={16} /> Move to…</button>
        <button class="btn small" onclick={stopPicking}>Cancel</button>
      {:else}
        <span class="spacer"></span>
        <button class="btn small" onclick={() => (picking = true)}>Select…</button>
      {/if}
    </div>
  {/if}
{/if}

{#if !sections.some((s) => s.items.length) && !cart.length}
  <div class="card empty">
    <p><strong>Nothing to buy yet.</strong></p>
    <p class="muted">Type above to add things. What you add is remembered and suggested next time.</p>
  </div>
{/if}

{#snippet section(s: (typeof sections)[number], movable: boolean)}
  {@const isCollapsed = collapsed.has(s.id) && !dnd.item}
  <section
    class="card"
    data-id={movable ? s.id : undefined}
    data-label={s.name}
    use:draggable={{ item: () => ({ kind: 'category', id: s.id, listId: list.id }), disabled: !movable || readonly || picking, handleOnly: true }}
  >
    <h2>
      {#if movable && !readonly && !picking}
        <span class="grip" data-handle tabindex="0" role="button" aria-label="Move category {s.name}" aria-roledescription="draggable"><Icon name="grip" size={16} /></span>
      {/if}
      <button class="cat" aria-expanded={!isCollapsed} onclick={() => collapsed.toggle(s.id)}>
        <span class="chev" class:collapsed={isCollapsed}><Icon name="chevronDown" size={18} /></span>
        {s.name}
        <span class="badge">{s.items.length}</span>
      </button>
    </h2>
    {#if !isCollapsed}
      <ul class:empty-drop={!s.items.length} use:dropList={itemDrop(s.id === 'other' ? null : s.id)}>
        {#each s.items as item (item.id)}
          <ItemRow {item} {readonly} onedit={edit} onpicture={showPicture} selected={picking ? picked.includes(item.id) : undefined} onselect={pick} />
        {/each}
      </ul>
    {/if}
  </section>
{/snippet}

<div
  use:dropList={{
    accepts: (it) => it.kind === 'category' && it.listId === list.id,
    drop: (it, index) => moveCategory(it.id, index),
    keyMove: moveCategory,
  }}
>
  {#each named as s (s.id)}{@render section(s, true)}{/each}
</div>
{#if other}{@render section(other, false)}{/if}

{#if cart.length}
  <section class="card cart">
    <div class="cart-head">
      <h2>In the cart <span class="badge">{cart.length}</span></h2>
      {#if !readonly}<button class="btn primary small" onclick={() => shopping.finish(list.id)}><Icon name="check" size={16} /> Finish shopping</button>{/if}
    </div>
    <ul>
      {#each cart as item (item.id)}<ItemRow {item} {readonly} onedit={edit} onpicture={showPicture} />{/each}
    </ul>
  </section>
{/if}

<ItemSheet item={editing} bind:open={editOpen} {focus} />
<PictureSheet
  bind:open={pictureOpen}
  image={shownLook?.image ?? null}
  title={shown?.name ?? ''}
  note={shownLook?.from === 'preferred' ? `Any brand will do; usually ${shownLook.brand?.replace(/^usually /, '')}.` : shownLook?.brand}
/>
<MoveSheet bind:open={moveOpen} items={pickedItems} ondone={stopPicking} />

<style>
  .card {
    margin-bottom: 12px;
    overflow: hidden;
  }
  .empty {
    padding: 20px;
  }
  .empty p {
    margin: 0 0 6px;
  }
  .pickbar {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: -6px 0 10px;
  }
  .spacer {
    flex: 1;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  ul.empty-drop {
    min-height: 48px;
    border-top: 1px dashed var(--border);
  }
  h2 {
    display: flex;
    align-items: center;
    font-size: 14px;
  }
  h2 .grip {
    margin-left: 2px;
  }
  .cat {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 44px;
    padding: 6px 12px 6px 6px;
    font-weight: 700;
    text-align: left;
  }
  .chev {
    display: grid;
    color: var(--muted);
    transition: transform 0.15s;
  }
  .chev.collapsed {
    transform: rotate(-90deg);
  }
  .cart {
    background: var(--surface-2);
  }
  .cart-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 8px 8px 8px 12px;
  }
  .cart-head h2 {
    gap: 8px;
  }
</style>
