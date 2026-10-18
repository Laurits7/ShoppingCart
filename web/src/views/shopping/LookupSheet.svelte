<script lang="ts">
  // Find a product or picture on Open Food Facts (by name or barcode) and use it. The
  // server does the looking up and fetches the pictures, so the browser only talks to us.
  import { message } from '../../lib/api/client'
  import type { LookupResult } from '../../lib/api/types/LookupResult'
  import type { Product } from '../../lib/api/types/Product'
  import Icon from '../../lib/components/Icon.svelte'
  import ScanSheet from '../../lib/components/ScanSheet.svelte'
  import Sheet from '../../lib/components/Sheet.svelte'
  import { label, type LookupMode } from '../../lib/products'
  import { shopping } from '../../lib/shopping.svelte'

  let {
    open = $bindable(false),
    catalogId,
    query,
    mode,
    ondone,
  }: { open: boolean; catalogId: string; query: string; mode: LookupMode; ondone?: (p: Product | null) => void } = $props()

  let q = $state('')
  let results = $state<LookupResult[]>([])
  let busy = $state(false)
  let using = $state<string | null>(null)
  let error = $state('')
  let searched = $state(false)
  let scanOpen = $state(false)

  $effect(() => {
    if (open) {
      q = query
      results = []
      error = ''
      searched = false
    }
  })

  async function run(get: () => Promise<LookupResult[]>) {
    busy = true
    error = ''
    try {
      results = await get()
    } catch (e) {
      results = []
      error = message(e)
    }
    busy = false
    searched = true
  }

  const search = (e?: Event) => {
    e?.preventDefault()
    if (q.trim()) run(() => shopping.lookup(q.trim()))
  }
  const byBarcode = (code: string) => run(async () => [await shopping.lookupBarcode(code)])

  async function use(r: LookupResult) {
    using = r.code
    const res = await shopping.importLookup(r.code, catalogId, {
      productId: mode.kind === 'product-picture' ? mode.productId : undefined,
      pictureOnly: mode.kind === 'catalog-picture',
    })
    using = null
    if (res === undefined) return
    open = false
    ondone?.(res)
  }
</script>

<Sheet bind:open title={mode.kind === 'product' ? 'Find a product' : 'Find a picture'}>
  <form class="row" onsubmit={search}>
    <input type="search" bind:value={q} placeholder="Product name, e.g. alma piim" aria-label="Search Open Food Facts" maxlength="100" />
    <button class="btn" type="submit" disabled={busy}><Icon name="search" size={16} /> Search</button>
    <button class="btn" type="button" aria-label="Scan or type a barcode" title="Barcode" onclick={() => (scanOpen = true)}><Icon name="barcode" size={18} /></button>
  </form>
  {#if busy}
    <p class="muted" aria-busy="true">Looking on Open Food Facts…</p>
  {:else if error}
    <p class="error" role="alert">{error === 'Not Found' ? 'Not found on Open Food Facts.' : error}</p>
  {:else if searched && !results.length}
    <p class="muted">Nothing found. Try fewer or other words, or take a photo yourself.</p>
  {/if}
  {#if results.length}
    <ul>
      {#each results as r (r.code)}
        <li>
          {#if r.has_picture}
            <img src="/api/v1/lookup/picture/{r.code}" alt="" width="56" height="56" loading="lazy" />
          {:else}
            <span class="noimg"><Icon name="image" size={20} /></span>
          {/if}
          <span class="text">
            <strong>{label(r.brand, r.name) || r.code}</strong>
            <span class="muted small">{r.quantity}{r.quantity ? ' · ' : ''}{r.code}</span>
          </span>
          <button class="btn small primary" disabled={using !== null || (mode.kind !== 'product' && !r.has_picture)} onclick={() => use(r)}>
            {using === r.code ? 'Saving…' : 'Use'}
          </button>
        </li>
      {/each}
    </ul>
    <p class="muted small">
      From <a href="https://world.openfoodfacts.org" target="_blank" rel="noopener noreferrer">Open Food Facts</a>, a free database anyone can add to.
      Pictures are CC BY-SA.
    </p>
  {/if}
</Sheet>

<ScanSheet bind:open={scanOpen} title="Look up a barcode" onfound={byBarcode} />

<style>
  .row {
    display: flex;
    gap: 6px;
  }
  p {
    margin: 0;
  }
  .small {
    font-size: 13px;
  }
  .error {
    color: var(--danger);
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  li {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 0;
    border-top: 1px solid var(--border);
  }
  img,
  .noimg {
    width: 56px;
    height: 56px;
    flex: none;
    object-fit: contain;
    border-radius: 6px;
    background: var(--surface-2);
  }
  .noimg {
    display: grid;
    place-items: center;
    color: var(--muted);
  }
  .text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    overflow-wrap: anywhere;
  }
</style>
