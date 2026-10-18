<script lang="ts">
  import { COMMON_UNITS, defaultStep, normalizeUnit } from '../../lib/amount'
  import type { Item } from '../../lib/api/types/Item'
  import Sheet from '../../lib/components/Sheet.svelte'
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

  function makeStaple() {
    if (!item) return
    const shopId = shopping.list(item.list_id)?.shop_id
    if (!shopId) return
    if (shopping.staplesOf(shopId).some((s) => s.name.toLowerCase() === item!.name.toLowerCase())) {
      toast(`${item.name} is already a staple`)
      return
    }
    const cat = shopping.categories.find((c) => c.id === item!.category_id)?.name ?? null
    shopping.createStaple(shopId, { name: item.name, amount: item.amount, unit: item.unit, category_name: cat })
    toast(`${item.name} is now a staple of ${shopping.shop(shopId)?.name ?? 'this shop'}`)
  }

  let moveOpen = $state(false)
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
      <div class="actions wide">
        <button class="btn primary" type="submit">Save</button>
        <button class="btn" type="button" onclick={makeStaple}>Make staple</button>
        {#if otherLists}<button class="btn" type="button" onclick={() => ((open = false), (moveOpen = true))}>Move to…</button>{/if}
        <span class="spacer"></span>
        <button class="btn danger" type="button" onclick={remove}>Delete</button>
      </div>
    </form>
    <datalist id="units">{#each COMMON_UNITS.filter(Boolean) as u (u)}<option value={u}></option>{/each}</datalist>
  {/if}
</Sheet>

{#if item}<MoveSheet bind:open={moveOpen} items={[item]} />{/if}
