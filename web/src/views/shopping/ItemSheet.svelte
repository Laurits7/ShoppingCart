<script lang="ts">
  import { COMMON_UNITS, defaultStep, normalizeUnit } from '../../lib/amount'
  import type { Item } from '../../lib/api/types/Item'
  import Sheet from '../../lib/components/Sheet.svelte'
  import UpDown from '../../lib/components/UpDown.svelte'
  import { shopping } from '../../lib/shopping.svelte'
  import { toast } from '../../lib/toast.svelte'
  import MoveSheet from './MoveSheet.svelte'

  let { item, open = $bindable(false), focus }: { item: Item | null; open: boolean; focus?: 'amount' } = $props()

  let name = $state('')
  let amount = $state('')
  let unit = $state('')
  let step = $state('')
  let note = $state('')
  let category = $state('')

  $effect(() => {
    if (open && item) {
      name = item.name
      amount = String(item.amount)
      unit = item.unit ?? ''
      step = item.step == null ? '' : String(item.step)
      note = item.note
      category = item.category_id ?? ''
    }
  })

  const categories = $derived(item ? shopping.categoriesOf(item.list_id) : [])
  // Its place among the open items of its category, for moving without dragging.
  const section = $derived(item && !item.checked_at ? shopping.sections(item.list_id).find((s) => s.items.some((i) => i.id === item!.id)) : undefined)
  const place = $derived(section && item ? section.items.findIndex((i) => i.id === item!.id) : -1)
  const num = (s: string) => Number(s.trim().replace(',', '.'))

  function save(e: Event) {
    e.preventDefault()
    if (!item) return
    const patch: Parameters<typeof shopping.editItem>[1] = {}
    const n = name.trim()
    if (!n) return toast('The name is empty', 'error')
    if (n !== item.name) patch.name = n
    const a = num(amount)
    if (!(a > 0)) return toast('The amount must be more than 0', 'error')
    if (a !== item.amount) patch.amount = a
    const u = normalizeUnit(unit)
    if (u !== item.unit) patch.unit = u
    const s = step.trim() ? num(step) : null
    if (s !== null && !(s > 0)) return toast('The step must be more than 0', 'error')
    if (s !== item.step) patch.step = s
    if (note.trim() !== item.note) patch.note = note.trim()
    const c = category || null
    if (c !== item.category_id) patch.category_id = c
    if (Object.keys(patch).length) shopping.editItem(item, patch)
    open = false
  }

  // Put this item into one of the shop's templates too.
  let toTemplate = $state('')
  const templates = $derived(item ? shopping.templatesOf(shopping.list(item.list_id)?.shop_id ?? '').filter((t) => t.id !== item!.list_id) : [])

  let moveOpen = $state(false)
  let brandOpen = $state(false)
  const entry = $derived(item ? shopping.entryOf(item) : undefined)
  const otherLists = $derived(shopping.lists.some((l) => l.id !== item?.list_id && !l.archived_at && l.role !== 'viewer'))

  function remove() {
    if (!item) return
    shopping.remove(item)
    open = false
  }
</script>

<Sheet bind:open title="Edit item">
  {#if item}
    <form class="fields" onsubmit={save}>
      <label class="field wide"><span>Name</span><input bind:value={name} maxlength="200" required /></label>
      <!-- svelte-ignore a11y_autofocus -->
      <label class="field"><span>Amount</span><input bind:value={amount} inputmode="decimal" required autofocus={focus === 'amount'} /></label>
      <label class="field"><span>Unit</span><input bind:value={unit} list="units" placeholder="pieces" maxlength="16" /></label>
      <label class="field"><span>Category</span>
        <select bind:value={category}>
          {#each categories as c (c.id)}<option value={c.id}>{c.name}</option>{/each}
          <option value="">Other</option>
        </select>
      </label>
      <label class="field"><span>−/+ step</span><input bind:value={step} inputmode="decimal" placeholder={String(defaultStep(normalizeUnit(unit)))} /></label>
      <label class="field wide"><span>Note</span><textarea bind:value={note} maxlength="1000"></textarea></label>
      {#if section && item && section.items.length > 1}
        <div class="field wide order">
          <span>Order in {section.name}</span>
          <UpDown what={item.name} index={place} count={section.items.length} onmove={(to) => shopping.moveItem(item!, item!.list_id, section!.id === 'other' ? null : section!.id, to)} />
        </div>
      {/if}
      <div class="actions wide">
        <button class="btn primary" type="submit">Save</button>
        {#if entry}<button class="btn" type="button" onclick={() => ((open = false), (brandOpen = true))}>Brand…</button>{/if}
        {#if templates.length}
          <span class="to-template">
            <select bind:value={toTemplate} aria-label="Add to a template">
              <option value="">Add to template…</option>
              {#each templates as t (t.id)}<option value={t.id}>{t.name}</option>{/each}
            </select>
            <button
              class="btn"
              type="button"
              disabled={!toTemplate}
              onclick={() => {
                const t = shopping.list(toTemplate)
                if (t && item) shopping.addToTemplate(item, t)
                toTemplate = ''
              }}>Add</button
            >
          </span>
        {/if}
        {#if otherLists}<button class="btn" type="button" onclick={() => ((open = false), (moveOpen = true))}>Move to…</button>{/if}
        <span class="spacer"></span>
        <button class="btn danger" type="button" onclick={remove}>Delete</button>
      </div>
    </form>
    <datalist id="units">{#each COMMON_UNITS.filter(Boolean) as u (u)}<option value={u}></option>{/each}</datalist>
  {/if}
</Sheet>

{#if item}<MoveSheet bind:open={moveOpen} items={[item]} />{/if}
{#if item && entry && brandOpen}
  <!-- The brand chooser (with the Open Food Facts search) loads when first opened. -->
  {#await import('./ProductSheet.svelte') then m}
    <m.default bind:open={brandOpen} catalogId={entry.id} {item} shopId={shopping.list(item.list_id)?.shop_id ?? null} />
  {/await}
{/if}

<style>
  .to-template {
    display: inline-flex;
    gap: 6px;
  }
  .to-template select {
    width: auto;
  }
  .order {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
</style>
