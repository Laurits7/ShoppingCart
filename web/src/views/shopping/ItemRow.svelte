<script lang="ts">
  import { formatAmount } from '../../lib/amount'
  import type { Item } from '../../lib/api/types/Item'
  import Icon from '../../lib/components/Icon.svelte'
  import { shopping } from '../../lib/shopping.svelte'

  let { item, readonly = false, onedit }: { item: Item; readonly?: boolean; onedit: (item: Item, focus?: 'amount') => void } = $props()
  const checked = $derived(!!item.checked_at)
  const amount = $derived(formatAmount(item.amount, item.unit))
</script>

<li class:checked>
  <button class="main" aria-pressed={checked} disabled={readonly} onclick={() => shopping.tick(item, !checked)}>
    <span class="box"><Icon name={checked ? 'checkCircle' : 'circle'} size={24} /></span>
    <span class="text">
      <span class="name">{item.name}</span>
      {#if item.note}<span class="note">{item.note}</span>{/if}
    </span>
  </button>
  {#if checked || readonly}
    <span class="amount static" aria-label="Amount">{amount}</span>
  {:else}
    <span class="stepper">
      <button class="icon-btn" aria-label="Less {item.name}" onclick={() => shopping.step(item, false)}><Icon name="minus" size={18} /></button>
      <button class="amount" aria-label="Amount of {item.name}: {amount}. Change" onclick={() => onedit(item, 'amount')}>{amount}</button>
      <button class="icon-btn" aria-label="More {item.name}" onclick={() => shopping.step(item, true)}><Icon name="plus" size={18} /></button>
    </span>
  {/if}
  {#if !readonly}
    <button class="icon-btn" aria-label="Edit {item.name}" onclick={() => onedit(item)}><Icon name="more" /></button>
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
  }
  .main {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: 52px;
    padding: 6px 4px 6px 12px;
    text-align: left;
  }
  .main:disabled {
    cursor: default;
  }
  .box {
    color: var(--muted);
    display: grid;
  }
  .checked .box {
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
