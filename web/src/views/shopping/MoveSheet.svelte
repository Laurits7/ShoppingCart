<script lang="ts">
  // Pick another list (of any shop) to move items to.
  import type { Item } from '../../lib/api/types/Item'
  import Sheet from '../../lib/components/Sheet.svelte'
  import { shopping } from '../../lib/shopping.svelte'

  let { open = $bindable(false), items, ondone }: { open: boolean; items: Item[]; ondone?: () => void } = $props()
  const from = $derived(items[0]?.list_id)

  function move(listId: string) {
    shopping.moveItems(items, listId)
    open = false
    ondone?.()
  }
</script>

<Sheet bind:open title="Move {items.length === 1 ? items[0].name : `${items.length} items`} to…">
  {#each shopping.sortedShops as s (s.id)}
    {@const lists = shopping.listsOf(s.id).filter((l) => l.id !== from && l.role !== 'viewer')}
    {#if lists.length}
      <section>
        <h3><span class="dot" style:background={s.color ?? undefined}></span>{s.name}</h3>
        {#each lists as l (l.id)}
          <button class="target" onclick={() => move(l.id)}>
            <span>{l.name}</span>
            <span class="badge">{shopping.openCount(l.id)}</span>
          </button>
        {/each}
      </section>
    {/if}
  {:else}
    <p class="muted">No other lists yet.</p>
  {/each}
</Sheet>

<style>
  h3 {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 14px;
    margin: 4px 0;
  }
  .target {
    width: 100%;
    display: flex;
    align-items: center;
    justify-content: space-between;
    min-height: 46px;
    padding: 6px 10px;
    border-radius: var(--radius-sm);
    text-align: left;
  }
  .target:hover {
    background: var(--surface-2);
  }
</style>
