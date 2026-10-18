<script lang="ts">
  import { formatAmount } from '../../lib/amount'
  import type { Item } from '../../lib/api/types/Item'
  import Icon from '../../lib/components/Icon.svelte'
  import { draggable } from '../../lib/dnd.svelte'
  import { shopping } from '../../lib/shopping.svelte'

  let {
    item,
    readonly = false,
    onedit,
    selected,
    onselect,
  }: {
    item: Item
    readonly?: boolean
    onedit: (item: Item, focus?: 'amount') => void
    /** Defined while picking items (to move them): whether this one is picked. */
    selected?: boolean
    onselect?: (item: Item) => void
  } = $props()
  const checked = $derived(!!item.checked_at)
  const amount = $derived(formatAmount(item.amount, item.unit))
  const selecting = $derived(selected !== undefined)
  // Items in the cart, read-only lists and picking mode don't drag.
  const canDrag = $derived(!readonly && !checked && !selecting)
</script>

<li
  class:checked
  data-id={item.id}
  data-label={item.name}
  use:draggable={{ item: () => ({ kind: 'item', id: item.id, listId: item.list_id }), disabled: !canDrag }}
>
  {#if canDrag}
    <span class="grip" data-handle tabindex="0" role="button" aria-label="Move {item.name}" aria-roledescription="draggable"><Icon name="grip" size={16} /></span>
  {/if}
  {#if selecting}
    <button class="main" aria-pressed={selected} onclick={() => onselect?.(item)}>
      <span class="box" class:on={selected}><Icon name={selected ? 'checkCircle' : 'circle'} size={24} /></span>
      <span class="text"><span class="name">{item.name}</span></span>
    </button>
    <span class="amount static">{amount}</span>
  {:else}
    <button class="main" class:pad={!canDrag} aria-pressed={checked} disabled={readonly} onclick={() => shopping.tick(item, !checked)}>
      <span class="box"><Icon name={checked ? 'checkCircle' : 'circle'} size={24} /></span>
      <span class="text">
        <span class="name">{item.name}</span>
        {#if item.note}<span class="note">{item.note}</span>{/if}
      </span>
    </button>
    {#if checked || readonly}
      <span class="amount static" aria-label="Amount">{amount}</span>
    {:else}
      <span class="stepper" data-nodrag>
        <button class="icon-btn" aria-label="Less {item.name}" onclick={() => shopping.step(item, false)}><Icon name="minus" size={18} /></button>
        <button class="amount" aria-label="Amount of {item.name}: {amount}. Change" onclick={() => onedit(item, 'amount')}>{amount}</button>
        <button class="icon-btn" aria-label="More {item.name}" onclick={() => shopping.step(item, true)}><Icon name="plus" size={18} /></button>
      </span>
    {/if}
    {#if !readonly}
      <button class="icon-btn" data-nodrag aria-label="Edit {item.name}" onclick={() => onedit(item)}><Icon name="more" /></button>
    {/if}
  {/if}
</li>

<style>
  li {
    display: flex;
    align-items: center;
    gap: 2px;
    min-height: 52px;
    padding: 0 4px 0 0;
    border-top: 1px solid var(--border);
    background: var(--surface);
  }
  li.checked {
    background: transparent;
  }
  .grip {
    margin-left: 2px;
  }
  .main {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: 52px;
    padding: 6px 4px 6px 2px;
    text-align: left;
  }
  .main.pad {
    padding-left: 12px;
  }
  .main:disabled {
    cursor: default;
  }
  .box {
    color: var(--muted);
    display: grid;
  }
  .checked .box,
  .box.on {
    color: var(--accent);
  }
  .text {
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .name {
    overflow-wrap: anywhere;
  }
  .checked .name {
    color: var(--muted);
    text-decoration: line-through;
  }
  .note {
    font-size: 13px;
    color: var(--muted);
    overflow-wrap: anywhere;
  }
  .stepper {
    display: flex;
    align-items: center;
  }
  .stepper .icon-btn {
    min-width: 40px;
    color: var(--text);
  }
  .amount {
    min-width: 44px;
    min-height: 40px;
    padding: 0 6px;
    border-radius: var(--radius-sm);
    font-weight: 600;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
    text-align: center;
  }
  button.amount {
    background: var(--surface-2);
  }
  .amount.static {
    display: grid;
    place-items: center;
    color: var(--muted);
    font-weight: 500;
  }
</style>
