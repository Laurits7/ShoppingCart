<script lang="ts">
  import { formatAmount } from '../../lib/amount'
  import type { Item } from '../../lib/api/types/Item'
  import Icon from '../../lib/components/Icon.svelte'
  import Thumb from '../../lib/components/Thumb.svelte'
  import { draggable } from '../../lib/dnd.svelte'
  import { shopping } from '../../lib/shopping.svelte'

  let {
    item,
    readonly = false,
    onedit,
    selected,
    onselect,
    onpicture,
    onassign,
    tickable = true,
  }: {
    item: Item
    readonly?: boolean
    onedit: (item: Item, focus?: 'amount') => void
    /** Defined while picking items (to move them): whether this one is picked. */
    selected?: boolean
    onselect?: (item: Item) => void
    /** Show the item's picture large. */
    onpicture?: (item: Item) => void
    /** Inbox: move it to a list. */
    onassign?: (item: Item) => void
    /** Templates and the inbox aren't ticked off; tapping the row edits it instead. */
    tickable?: boolean
  } = $props()
  const look = $derived(shopping.look(item))
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
    <span class="grip" data-handle tabindex="0" role="button" aria-describedby="dnd-help" aria-label="Move {item.name}" aria-roledescription="draggable"><Icon name="grip" size={16} /></span>
  {/if}
  {#if selecting}
    <button class="main" aria-pressed={selected} onclick={() => onselect?.(item)}>
      <span class="box" class:on={selected}><Icon name={selected ? 'checkCircle' : 'circle'} size={24} /></span>
      <span class="text"><span class="name">{item.name}</span></span>
    </button>
    <span class="amount static">{amount}</span>
  {:else}
    <button
      class="main"
      class:pad={!canDrag}
      aria-pressed={tickable ? checked : undefined}
      disabled={readonly}
      onclick={() => (tickable ? shopping.tick(item, !checked) : onedit(item))}
    >
      {#if tickable}
        <span class="box"><Icon name={checked ? 'checkCircle' : 'circle'} size={24} /></span>
      {:else}
        <span class="bullet" aria-hidden="true"></span>
      {/if}
      <span class="text">
        <span class="name">{item.name}</span>
        {#if look.brand}<span class="brand" class:pinned={!!item.product_id}>{look.brand}</span>{/if}
        {#if item.note}<span class="note">{item.note}</span>{/if}
      </span>
    </button>
    {#if look.image}
      <button class="thumb" data-nodrag aria-label="Picture of {item.name}" onclick={() => onpicture?.(item)}><Thumb id={look.image} size={40} /></button>
    {/if}
    {#if checked || readonly}
      <span class="amount static"><span class="sr-only">Amount </span>{amount}</span>
    {:else if onassign}
      <!-- In the inbox you sort rather than shop: no stepper, so the name has room on a phone. -->
      <button class="amount" data-nodrag aria-label="Amount of {item.name}: {amount}. Change" onclick={() => onedit(item, 'amount')}>{amount}</button>
    {:else}
      <span class="stepper" data-nodrag>
        <button class="icon-btn" aria-label="Less {item.name}" onclick={() => shopping.step(item, false)}><Icon name="minus" size={18} /></button>
        <button class="amount" aria-label="Amount of {item.name}: {amount}. Change" onclick={() => onedit(item, 'amount')}>{amount}</button>
        <button class="icon-btn" aria-label="More {item.name}" onclick={() => shopping.step(item, true)}><Icon name="plus" size={18} /></button>
      </span>
    {/if}
    {#if onassign && !readonly}
      <button class="btn small assign" data-nodrag aria-label="Assign {item.name} to a list" onclick={() => onassign(item)}><Icon name="move" size={16} /> List</button>
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
  .assign {
    flex: none;
    margin-left: 4px;
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
  .bullet {
    width: 8px;
    height: 8px;
    margin: 0 8px;
    border-radius: 50%;
    background: var(--muted);
    flex: none;
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
  .brand {
    font-size: 13px;
    color: var(--muted);
    overflow-wrap: anywhere;
  }
  .brand.pinned {
    color: var(--text);
    font-weight: 600;
  }
  .checked .brand {
    color: var(--muted);
  }
  .thumb {
    display: grid;
    padding: 2px;
    border-radius: 8px;
  }
  .checked .thumb {
    opacity: 0.5;
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
