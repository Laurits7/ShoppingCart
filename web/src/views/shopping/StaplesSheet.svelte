<script lang="ts">
  import { formatAmount, parse } from '../../lib/amount'
  import type { ShoppingList } from '../../lib/api/types/ShoppingList'
  import { normalizeName } from '../../lib/catalog'
  import Icon from '../../lib/components/Icon.svelte'
  import { draggable, dropList } from '../../lib/dnd.svelte'
  import Sheet from '../../lib/components/Sheet.svelte'
  import { shopping } from '../../lib/shopping.svelte'

  let { open = $bindable(false), list }: { open: boolean; list: ShoppingList } = $props()
  const shop = $derived(shopping.shop(list.shop_id))
  const staples = $derived(shopping.staplesOf(list.shop_id))
  let picked = $state<string[]>([])
  let text = $state('')

  const openNames = () => new Set(shopping.itemsOf(list.id).filter((i) => !i.checked_at).map((i) => normalizeName(i.name)))

  // Each time it opens, pre-tick the staples that aren't on the list yet.
  $effect(() => {
    if (open) {
      const onList = openNames()
      picked = shopping.staplesOf(list.shop_id).filter((s) => !onList.has(normalizeName(s.name))).map((s) => s.id)
    }
  })

  function moveStaple(id: string, index: number) {
    const s = shopping.staples.find((x) => x.id === id)
    if (s) shopping.moveStaple(s, index)
  }

  function addPicked() {
    shopping.addStaples(list.id, staples.filter((s) => picked.includes(s.id)))
    open = false
  }

  function create(e: Event) {
    e.preventDefault()
    const p = parse(text.trim())
    if (!p.name) return
    const known = shopping.known(p.name)
    shopping.createStaple(list.shop_id, {
      name: known?.name ?? p.name,
      amount: p.amount ?? known?.amount ?? 1,
      unit: p.amount !== null ? p.unit : (known?.unit ?? null),
      category_name: known?.category_name ?? null,
    })
    text = ''
  }
</script>

<Sheet bind:open title="Staples of {shop?.name ?? 'this shop'}">
  {#if staples.length}
    <p class="muted small">What you usually buy here. Ticked ones are added to the list; those already on it start unticked.</p>
    <ul use:dropList={{ accepts: (it) => it.kind === 'staple' && it.shopId === list.shop_id, drop: (it, i) => moveStaple(it.id, i), keyMove: moveStaple }}>
      {#each staples as s (s.id)}
        <li data-id={s.id} data-label={s.name} use:draggable={{ item: () => ({ kind: 'staple', id: s.id, shopId: s.shop_id }), disabled: list.role !== 'owner' }}>
          {#if list.role === 'owner'}
            <span class="grip" data-handle tabindex="0" role="button" aria-label="Move staple {s.name}" aria-roledescription="draggable"><Icon name="grip" size={16} /></span>
          {/if}
          <label>
            <input type="checkbox" value={s.id} bind:group={picked} />
            <span class="name">{s.name}</span>
            <span class="muted">{formatAmount(s.amount, s.unit)}</span>
          </label>
          {#if list.role === 'owner'}
            <button class="icon-btn" aria-label="Remove staple {s.name}" onclick={() => shopping.deleteStaple(s)}><Icon name="trash" size={18} /></button>
          {/if}
        </li>
      {/each}
    </ul>
    {#if list.role !== 'viewer'}
      <button class="btn primary" disabled={!picked.length} onclick={addPicked}>Add {picked.length} to the list</button>
    {/if}
  {:else}
    <p class="muted">No staples yet. Add the things you buy here every time, or turn an item into a staple from its menu.</p>
  {/if}
  {#if list.role === 'owner'}
    <form class="row" onsubmit={create}>
      <input bind:value={text} placeholder="New staple, e.g. milk 2" aria-label="New staple" maxlength="200" />
      <button class="btn" type="submit">Add staple</button>
    </form>
  {/if}
</Sheet>

<style>
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
  }
  li {
    display: flex;
    align-items: center;
    gap: 4px;
    border-top: 1px solid var(--border);
    background: var(--surface);
  }
  label {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: 48px;
    cursor: pointer;
  }
  label input {
    width: 22px;
    min-height: 22px;
    accent-color: var(--accent);
  }
  .name {
    flex: 1;
  }
  .row {
    display: flex;
    gap: 8px;
  }
</style>
