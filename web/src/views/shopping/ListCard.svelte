<script lang="ts">
  // One list on the summary page: its items to tick and step right there (ticked ones at the
  // end, greyed and struck through), a compact quick add and Done. The title opens the full
  // list. The inbox uses it too, with "Assign" on each item instead.
  import type { Item } from '../../lib/api/types/Item'
  import type { ShoppingList } from '../../lib/api/types/ShoppingList'
  import { collapsed } from '../../lib/collapsed.svelte'
  import Icon from '../../lib/components/Icon.svelte'
  import { draggable, dropList, type DragItem } from '../../lib/dnd.svelte'
  import { shopping } from '../../lib/shopping.svelte'
  import ItemRow from './ItemRow.svelte'
  import QuickAdd from './QuickAdd.svelte'

  let {
    list,
    onedit,
    ontemplate,
    onmenu,
    onpicture,
    onassign,
  }: {
    list: ShoppingList
    onedit: (item: Item, focus?: 'amount') => void
    ontemplate: (list: ShoppingList) => void
    onmenu: (list: ShoppingList) => void
    onpicture: (item: Item) => void
    /** Inbox only: move an item to a list. */
    onassign?: (item: Item) => void
  } = $props()
  const inbox = $derived(list.kind === 'inbox')

  const readonly = $derived(list.role === 'viewer')
  const sections = $derived(shopping.sections(list.id))
  const cart = $derived(shopping.cartOf(list.id))
  const open = $derived(sections.reduce((n, s) => n + s.items.length, 0))
  const key = $derived(`list:${list.id}`)
  const isCollapsed = $derived(collapsed.has(key))

  // Items can be dropped here from any list on the page.
  const itemDrop = (categoryId: string | null) => ({
    accepts: (it: DragItem) => it.kind === 'item' && !readonly,
    drop: (it: DragItem, index: number) => move(it.id, index, categoryId),
    keyMove: (id: string, index: number) => move(id, index, categoryId),
  })
  function move(id: string, index: number, categoryId: string | null = null) {
    const item = shopping.items.find((i) => i.id === id)
    if (item) shopping.moveItem(item, list.id, categoryId, index)
  }
</script>

<article
  class="card"
  data-id={list.id}
  data-label={list.name}
  use:draggable={{ item: () => ({ kind: 'list', id: list.id, shopId: list.shop_id ?? '' }), disabled: list.role !== 'owner' || inbox, handleOnly: true }}
>
  <header>
    {#if list.role === 'owner' && !inbox}
      <span class="grip" data-handle tabindex="0" role="button" aria-describedby="dnd-help" aria-label="Move list {list.name}" aria-roledescription="draggable"><Icon name="grip" size={16} /></span>
    {/if}
    <button class="icon-btn" aria-expanded={!isCollapsed} aria-label="{isCollapsed ? 'Show' : 'Hide'} items of {list.name}" onclick={() => collapsed.toggle(key)}>
      <span class="chev" class:collapsed={isCollapsed}><Icon name="chevronDown" size={18} /></span>
    </button>
    <h3 class="heading"><a class="title" href="/list/{list.id}">
      <span>
        {list.name}
        {#if inbox}<span class="from">From Mealie and other apps: assign each to a list</span>
        {:else if list.role !== 'owner'}<span class="from">{list.shop_name} · {list.owner_name}{list.role === 'viewer' ? ' · view only' : ''}</span>{/if}
      </span>
    </a></h3>
    <span class="badge">{open}<span class="sr-only"> open items</span></span>
    {#if cart.length && !readonly && !inbox}
      <button class="btn primary small done" title="Done: clear the ticked items" onclick={() => shopping.finish(list.id)}><Icon name="check" size={16} /> Done</button>
    {/if}
    {#if !readonly && !inbox}
      <button class="icon-btn" aria-label="Add from a template to {list.name}" title="Add from a template" onclick={() => ontemplate(list)}><Icon name="staples" size={20} /></button>
    {/if}
    {#if !inbox}
      <button class="icon-btn" aria-label="Settings of {list.name}" title="List and shop" onclick={() => onmenu(list)}><Icon name="more" /></button>
    {/if}
  </header>

  {#if !isCollapsed}
    {#if sections.length}
      {#each sections as s (s.id)}
        {#if sections.length > 1 || s.id !== 'other'}<div class="cat">{s.name}</div>{/if}
        <ul aria-label={s.name} use:dropList={itemDrop(s.id === 'other' ? null : s.id)}>
          {#each s.items as item (item.id)}<ItemRow {item} {readonly} {onedit} {onpicture} {onassign} tickable={!inbox} />{/each}
        </ul>
      {/each}
    {:else if !cart.length}
      <div class="nothing" use:dropList={itemDrop(null)}><p class="muted">Nothing to buy.</p></div>
    {/if}
    {#if cart.length}
      <!-- Got it: at the end, greyed out and struck through, until Done. -->
      <ul class="got" aria-label="Got">
        {#each cart as item (item.id)}<ItemRow {item} {readonly} {onedit} {onpicture} />{/each}
      </ul>
    {/if}
    {#if !readonly && !inbox}
      <div class="add"><QuickAdd listId={list.id} shopId={list.shop_id} compact placeholder="Add to {list.name}…" /></div>
    {/if}
  {/if}
</article>

<style>
  article {
    margin-bottom: 12px;
    overflow: hidden;
  }
  header {
    display: flex;
    align-items: center;
    gap: 2px;
    padding: 2px 4px 2px 0;
    background: var(--surface);
  }
  .heading {
    flex: 1;
    min-width: 0;
    font-size: inherit;
  }
  .title {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 6px;
    min-height: 44px;
    font-size: 17px;
    font-weight: 700;
    color: inherit;
    text-decoration: none;
    overflow-wrap: anywhere;
  }
  .title:hover {
    text-decoration: underline;
  }
  .title > span {
    display: flex;
    flex-direction: column;
  }
  .from {
    font-size: 12px;
    font-weight: 500;
    color: var(--muted);
  }
  .chev {
    display: grid;
    transition: transform 0.15s;
  }
  .chev.collapsed {
    transform: rotate(-90deg);
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .cat {
    padding: 8px 12px 2px;
    font-size: 12px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--muted);
    border-top: 1px solid var(--border);
  }
  .nothing {
    padding: 10px 14px;
    border-top: 1px solid var(--border);
  }
  .nothing p {
    margin: 0;
  }
  header .grip {
    margin-left: 2px;
  }
  .add {
    padding: 10px 12px;
    border-top: 1px solid var(--border);
  }
  .done {
    flex: none;
    margin-right: 2px;
  }
</style>
