<script lang="ts">
  import { COMMON_UNITS, formatAmount, normalizeUnit } from '../../lib/amount'
  import { message } from '../../lib/api/client'
  import type { CatalogItem } from '../../lib/api/types/CatalogItem'
  import { normalizeName } from '../../lib/catalog'
  import Icon from '../../lib/components/Icon.svelte'
  import Sheet from '../../lib/components/Sheet.svelte'
  import Thumb from '../../lib/components/Thumb.svelte'
  import ProductSheet from '../shopping/ProductSheet.svelte'
  import { shopping } from '../../lib/shopping.svelte'
  import { toast } from '../../lib/toast.svelte'

  let filter = $state('')
  let error = $state('')
  let selecting = $state(false)
  let selected = $state<string[]>([])
  let mergeOpen = $state(false)
  let mergeInto = $state('')
  let editing = $state<CatalogItem | null>(null)
  let editOpen = $state(false)
  let brandsOpen = $state(false)
  let form = $state({ name: '', category: '', amount: '', unit: '', step: '', hidden: false })

  // Always start from the server's copy: suggestions while shopping may have added
  // provisional entries.
  shopping.reloadCatalog().catch((e) => (error = message(e)))

  const shown = $derived.by(() => {
    const q = normalizeName(filter)
    return shopping.catalog.filter((c) => !q || normalizeName(c.name).includes(q)).sort((a, b) => normalizeName(a.name).localeCompare(normalizeName(b.name)))
  })
  const categoryNames = $derived([...new Set(shopping.categories.map((c) => c.name))].sort())
  const picked = $derived(shopping.catalog.filter((c) => selected.includes(c.id)))

  function edit(c: CatalogItem) {
    editing = c
    form = { name: c.name, category: c.category_name ?? '', amount: String(c.amount), unit: c.unit ?? '', step: c.step == null ? '' : String(c.step), hidden: c.hidden }
    editOpen = true
  }

  const num = (s: string) => Number(s.trim().replace(',', '.'))

  async function save(e: Event) {
    e.preventDefault()
    const c = editing
    if (!c) return
    const amount = num(form.amount)
    const step = form.step.trim() ? num(form.step) : null
    if (!(amount > 0) || (step !== null && !(step > 0))) return toast('Amounts must be more than 0', 'error')
    const ok = await shopping.patchCatalog(c, {
      name: form.name.trim(),
      category_name: form.category.trim() || null,
      amount,
      unit: normalizeUnit(form.unit),
      step,
      hidden: form.hidden,
    })
    if (ok) editOpen = false
  }

  async function del(c: CatalogItem) {
    if (!confirm(`Forget "${c.name}"? Items on your lists stay.`)) return
    editOpen = false
    await shopping.deleteCatalog(c)
  }

  function startMerge() {
    // Keep the most used one by default.
    mergeInto = [...picked].sort((a, b) => b.use_count - a.use_count)[0]?.id ?? ''
    mergeOpen = true
  }

  async function merge() {
    const into = picked.find((c) => c.id === mergeInto)
    if (!into) return
    await shopping.mergeCatalog(
      into,
      picked.filter((c) => c.id !== into.id),
    )
    mergeOpen = false
    selecting = false
    selected = []
  }
</script>

<div class="head">
  <a class="back" href="/settings"><Icon name="chevronRight" size={18} /> Settings</a>
  <h1>Item catalogue</h1>
  <p class="muted">
    Everything you've added, remembered for suggestions. Set the category an item goes into, its usual amount and unit, merge duplicates and hide what you
    don't buy any more.
  </p>
</div>

{#if error}
  <p class="card pad">Could not load the catalogue: {error}</p>
{:else}
  <div class="bar">
    <input type="search" bind:value={filter} placeholder="Search {shopping.catalog.length} items" aria-label="Search the catalogue" />
    {#if selecting}
      <button class="btn primary" disabled={selected.length < 2} onclick={startMerge}>Merge {selected.length || ''}</button>
      <button class="btn" onclick={() => ((selecting = false), (selected = []))}>Cancel</button>
    {:else}
      <button class="btn" onclick={() => (selecting = true)}>Merge…</button>
    {/if}
  </div>
  {#if selecting}<p class="muted small">Pick the entries that are the same thing, e.g. Tomato and Tomatoes.</p>{/if}

  {#if !shown.length}
    <p class="card pad muted">{shopping.catalog.length ? 'Nothing matches.' : 'Nothing yet: everything you add to a list shows up here.'}</p>
  {:else}
    <ul class="card">
      {#each shown as c (c.id)}
        <li class:hidden={c.hidden}>
          {#if selecting}
            <label class="entry">
              <input type="checkbox" value={c.id} bind:group={selected} />
              <span class="name">{c.name}</span>
              <span class="muted small">{c.use_count}×</span>
            </label>
          {:else}
            <button class="entry" onclick={() => edit(c)}>
              {#if c.image_id}<Thumb id={c.image_id} size={32} />{/if}
              <span class="name">{c.name}{#if c.hidden}<span class="muted small"> · hidden</span>{/if}</span>
              {#if shopping.productsOf(c.id).length}<span class="badge">{shopping.productsOf(c.id).length} brand{shopping.productsOf(c.id).length === 1 ? '' : 's'}</span>{/if}
              <span class="muted small">{c.category_name ?? 'Other'} · {formatAmount(c.amount, c.unit)} · {c.use_count}×</span>
            </button>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
{/if}

<Sheet bind:open={editOpen} title={editing?.name ?? 'Item'}>
  {#if editing}
    <form class="fields" onsubmit={save}>
      <label class="field wide"><span>Name</span><input bind:value={form.name} maxlength="200" required /></label>
      <label class="field wide"><span>Category it goes into</span><input bind:value={form.category} list="catalog-categories" placeholder="Other" maxlength="200" /></label>
      <label class="field"><span>Usual amount</span><input bind:value={form.amount} inputmode="decimal" required /></label>
      <label class="field"><span>Unit</span><input bind:value={form.unit} list="catalog-units" placeholder="pieces" maxlength="16" /></label>
      <label class="field"><span>−/+ step</span><input bind:value={form.step} inputmode="decimal" placeholder="from the unit" /></label>
      <label class="check"><input type="checkbox" bind:checked={form.hidden} /> Hide from suggestions</label>
      <div class="actions wide">
        <button class="btn primary" type="submit">Save</button>
        <button class="btn" type="button" onclick={() => ((editOpen = false), (brandsOpen = true))}>Brands and picture…</button>
        <span class="spacer"></span>
        <button class="btn danger" type="button" onclick={() => editing && del(editing)}><Icon name="trash" size={16} /> Forget</button>
      </div>
    </form>
    <datalist id="catalog-categories">{#each categoryNames as n (n)}<option value={n}></option>{/each}</datalist>
    <datalist id="catalog-units">{#each COMMON_UNITS.filter(Boolean) as u (u)}<option value={u}></option>{/each}</datalist>
  {/if}
</Sheet>

{#if editing}<ProductSheet bind:open={brandsOpen} catalogId={editing.id} />{/if}

<Sheet bind:open={mergeOpen} title="Merge {picked.length} entries">
  <p class="muted small">Their use counts add up and items on your lists point to the one you keep. Keep:</p>
  <div class="choices">
    {#each picked as c (c.id)}
      <label><input type="radio" name="merge-into" value={c.id} bind:group={mergeInto} /> {c.name} <span class="muted small">({c.use_count}×)</span></label>
    {/each}
  </div>
  <div class="actions"><button class="btn primary" onclick={merge}>Merge</button></div>
</Sheet>

<style>
  .head {
    margin-bottom: 16px;
  }
  .head p {
    margin: 6px 0 0;
  }
  .back {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    font-size: 14px;
    font-weight: 600;
    text-decoration: none;
    margin-bottom: 6px;
  }
  .back :global(svg) {
    transform: rotate(180deg);
  }
  .bar {
    display: flex;
    gap: 8px;
    margin-bottom: 8px;
  }
  .pad {
    padding: 20px;
    margin: 0;
  }
  .small {
    font-size: 13px;
  }
  p.small {
    margin: 0 0 8px;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    overflow: hidden;
  }
  li + li {
    border-top: 1px solid var(--border);
  }
  .entry {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
    min-height: 50px;
    padding: 8px 14px;
    text-align: left;
    cursor: pointer;
  }
  .entry input {
    width: 20px;
    min-height: 20px;
    accent-color: var(--accent);
  }
  .name {
    flex: 1;
    min-width: 40%;
  }
  li.hidden .name {
    color: var(--muted);
  }
  .check {
    display: flex;
    align-items: center;
    gap: 8px;
    align-self: end;
    min-height: 42px;
  }
  .check input {
    width: 20px;
    min-height: 20px;
    accent-color: var(--accent);
  }
  .choices {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .choices label {
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 40px;
  }
  .choices input {
    width: 20px;
    min-height: 20px;
    accent-color: var(--accent);
  }
</style>
