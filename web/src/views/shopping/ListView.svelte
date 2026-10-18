<script lang="ts">
  import type { Item } from '../../lib/api/types/Item'
  import type { ShoppingList } from '../../lib/api/types/ShoppingList'
  import Icon from '../../lib/components/Icon.svelte'
  import { collapsed } from '../../lib/collapsed.svelte'
  import { shopping } from '../../lib/shopping.svelte'
  import ItemRow from './ItemRow.svelte'
  import ItemSheet from './ItemSheet.svelte'
  import QuickAdd from './QuickAdd.svelte'

  let { list }: { list: ShoppingList } = $props()
  const readonly = $derived(list.role === 'viewer')

  const sections = $derived(shopping.sections(list.id))
  const cart = $derived(shopping.cartOf(list.id))

  let editing = $state<Item | null>(null)
  let editOpen = $state(false)
  let focus = $state<'amount' | undefined>()
  function edit(item: Item, f?: 'amount') {
    editing = item
    focus = f
    editOpen = true
  }
</script>

{#if !readonly}<QuickAdd listId={list.id} shopId={list.shop_id} />{/if}

{#if !sections.length && !cart.length}
  <div class="card empty">
    <p><strong>Nothing to buy yet.</strong></p>
    <p class="muted">Type above to add things. What you add is remembered and suggested next time.</p>
  </div>
{/if}

{#each sections as s (s.id)}
  {@const isCollapsed = collapsed.has(s.id)}
  <section class="card">
    <h2>
      <button class="cat" aria-expanded={!isCollapsed} onclick={() => collapsed.toggle(s.id)}>
        <span class="chev" class:collapsed={isCollapsed}><Icon name="chevronDown" size={18} /></span>
        {s.name}
        <span class="badge">{s.items.length}</span>
      </button>
    </h2>
    {#if !isCollapsed}
      <ul>
        {#each s.items as item (item.id)}<ItemRow {item} {readonly} onedit={edit} />{/each}
      </ul>
    {/if}
  </section>
{/each}

{#if cart.length}
  <section class="card cart">
    <div class="cart-head">
      <h2>In the cart <span class="badge">{cart.length}</span></h2>
      {#if !readonly}<button class="btn primary small" onclick={() => shopping.finish(list.id)}><Icon name="check" size={16} /> Finish shopping</button>{/if}
    </div>
    <ul>
      {#each cart as item (item.id)}<ItemRow {item} {readonly} onedit={edit} />{/each}
    </ul>
  </section>
{/if}

<ItemSheet item={editing} bind:open={editOpen} {focus} />

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
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  h2 {
    font-size: 14px;
  }
  .cat {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 44px;
    padding: 6px 12px;
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
    display: flex;
    gap: 8px;
    align-items: center;
  }
</style>
