<script lang="ts">
  // One wish: title (with its link's site), price, priority, note, and for everyone but the
  // owner of an own wishlist, who's getting it.
  import type { WishItem } from '../../lib/api/types/WishItem'
  import type { Wishlist } from '../../lib/api/types/Wishlist'
  import Icon from '../../lib/components/Icon.svelte'
  import { draggable } from '../../lib/dnd.svelte'
  import { session } from '../../lib/session.svelte'
  import { formatPrice, host, wishes } from '../../lib/wishes.svelte'

  let { item, wishlist, onedit }: { item: WishItem; wishlist: Wishlist; onedit: (i: WishItem) => void } = $props()
  const canEdit = $derived(wishlist.role !== 'viewer')
  const claim = $derived(wishlist.sees_claims ? wishes.claimOf(item.id) : undefined)
  const mine = $derived(claim?.user_id === session.me?.id)
  const site = $derived(host(item.url))
  const received = $derived(!!item.received_at)
</script>

<li
  class:received
  class:taken={!!claim && !mine}
  data-id={item.id}
  data-label={item.title}
  use:draggable={{ item: () => ({ kind: 'wish', id: item.id, wishlistId: item.wishlist_id }), disabled: !canEdit || received }}
>
  {#if canEdit && !received}
    <span class="grip" data-handle tabindex="0" role="button" aria-label="Move {item.title}" aria-roledescription="draggable"><Icon name="grip" size={16} /></span>
  {/if}
  <div class="body">
    <div class="head">
      {#if item.priority > 0}<span class="prio" title={item.priority === 2 ? 'Most wanted' : 'Would like a lot'} aria-label={item.priority === 2 ? 'most wanted' : 'would like a lot'}>{'★'.repeat(Number(item.priority))}</span>{/if}
      {#if item.url}
        <a href={item.url} target="_blank" rel="noopener noreferrer" data-nodrag>{item.title}</a>
      {:else}
        <span class="t">{item.title}</span>
      {/if}
      {#if item.price !== null}<span class="price">{formatPrice(item.price, item.currency)}</span>{/if}
    </div>
    {#if site || item.note}
      <div class="sub muted">{site ?? ''}{site && item.note ? ' · ' : ''}{item.note}</div>
    {/if}
    {#if wishlist.sees_claims && !received}
      <div class="claim" data-nodrag>
        {#if !claim}
          <button class="btn small" onclick={() => wishes.claim(item)}>I'll get this</button>
        {:else if mine}
          <span class="me">{claim.status === 'bought' ? 'You bought this' : "You're getting this"}</span>
          {#if claim.status === 'planned'}<button class="btn small" onclick={() => wishes.setBought(claim, true)}>Bought</button>{/if}
          <button class="btn small" onclick={() => wishes.release(claim)}>{claim.status === 'bought' ? 'Not bought' : 'Release'}</button>
        {:else}
          <span class="other">{claim.status === 'bought' ? `Bought by ${claim.user_name}` : `${claim.user_name} is getting this`}</span>
        {/if}
      </div>
    {/if}
  </div>
  {#if canEdit}
    <button class="icon-btn" data-nodrag aria-label="Edit {item.title}" onclick={() => onedit(item)}><Icon name="more" /></button>
  {/if}
</li>

<style>
  li {
    display: flex;
    align-items: flex-start;
    gap: 6px;
    padding: 10px 4px 10px 8px;
    border-top: 1px solid var(--border);
    background: var(--surface);
  }
  .grip {
    min-height: 32px;
  }
  .body {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .head {
    display: flex;
    align-items: baseline;
    gap: 8px;
    flex-wrap: wrap;
  }
  .head a,
  .t {
    font-weight: 600;
    overflow-wrap: anywhere;
  }
  .prio {
    color: var(--star);
    letter-spacing: -2px;
  }
  .price {
    margin-left: auto;
    font-variant-numeric: tabular-nums;
    color: var(--muted);
  }
  .sub {
    font-size: 13px;
    overflow-wrap: anywhere;
  }
  .claim {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
    font-size: 13px;
  }
  .me {
    color: var(--accent);
    font-weight: 600;
  }
  .other {
    color: var(--muted);
    font-weight: 600;
  }
  .taken .head a,
  .taken .t {
    color: var(--muted);
  }
  .received {
    background: transparent;
  }
  .received .head a,
  .received .t {
    color: var(--muted);
  }
</style>
