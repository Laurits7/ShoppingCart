<script lang="ts">
  // Add items from a template to a list: the template's items as a checklist, those not on
  // the list yet ticked.
  import { formatAmount } from '../../lib/amount'
  import type { ShoppingList } from '../../lib/api/types/ShoppingList'
  import { normalizeName } from '../../lib/catalog'
  import Sheet from '../../lib/components/Sheet.svelte'
  import { byPosition, shopping } from '../../lib/shopping.svelte'

  let { open = $bindable(false), list }: { open: boolean; list: ShoppingList } = $props()
  // This shop's templates first, then others you can see.
  const templates = $derived([...shopping.templates].sort((a, b) => Number(b.shop_id === list.shop_id) - Number(a.shop_id === list.shop_id) || byPosition(a, b)))
  let templateId = $state('')
  let picked = $state<string[]>([])
  const template = $derived(shopping.list(templateId))
  const tItems = $derived(template ? shopping.itemsOf(template.id).sort(byPosition) : [])

  function pick() {
    const onList = new Set(shopping.itemsOf(list.id).filter((i) => !i.checked_at).map((i) => normalizeName(i.name)))
    picked = tItems.filter((i) => !onList.has(normalizeName(i.name))).map((i) => i.id)
  }

  // Each time it opens: the first template, with what isn't on the list yet ticked.
  $effect(() => {
    if (open) {
      if (!template) templateId = templates[0]?.id ?? ''
      pick()
    }
  })

  function add() {
    if (!template) return
    shopping.addFromTemplate(list.id, template, picked)
    open = false
  }
</script>

<Sheet bind:open title="Add from a template">
  {#if !templates.length}
    <p class="muted">No templates yet. Make one in <a href="/templates" onclick={() => (open = false)}>Templates</a>: the things you buy every time.</p>
  {:else}
    <label class="field">
      <span>Template</span>
      <select bind:value={templateId} onchange={pick}>
        {#each templates as t (t.id)}<option value={t.id}>{t.name}{t.shop_id !== list.shop_id ? ` (${t.shop_name})` : ''}</option>{/each}
      </select>
    </label>
    {#if tItems.length}
      <p class="muted small">Ticked ones are added; those already on the list start unticked.</p>
      <ul>
        {#each tItems as i (i.id)}
          <li>
            <label>
              <input type="checkbox" value={i.id} bind:group={picked} />
              <span class="name">{i.name}</span>
              <span class="muted">{formatAmount(i.amount, i.unit)}</span>
            </label>
          </li>
        {/each}
      </ul>
      <button class="btn primary" disabled={!picked.length} onclick={add}>Add {picked.length} to {list.name}</button>
    {:else}
      <p class="muted">This template is empty. <a href="/list/{template?.id}" onclick={() => (open = false)}>Open it</a> to add things.</p>
    {/if}
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
    border-top: 1px solid var(--border);
  }
  li label {
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: 48px;
    cursor: pointer;
  }
  li input {
    width: 22px;
    min-height: 22px;
    accent-color: var(--accent);
  }
  .name {
    flex: 1;
  }
</style>
