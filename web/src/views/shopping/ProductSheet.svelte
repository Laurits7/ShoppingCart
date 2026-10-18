<script lang="ts">
  // Brands of a catalogue item (SPEC §6.4b). With an item: choose which one it must be (or
  // any). Always: mark the preferred one (everywhere or at this shop), add one with a photo
  // or from Open Food Facts, and give the item itself a picture.
  import type { Item } from '../../lib/api/types/Item'
  import type { Product } from '../../lib/api/types/Product'
  import Icon from '../../lib/components/Icon.svelte'
  import PhotoPicker from '../../lib/components/PhotoPicker.svelte'
  import Sheet from '../../lib/components/Sheet.svelte'
  import Thumb from '../../lib/components/Thumb.svelte'
  import { cleanBarcode } from '../../lib/barcode'
  import { label, type LookupMode } from '../../lib/products'
  import { shopping } from '../../lib/shopping.svelte'
  import { toast } from '../../lib/toast.svelte'
  import LookupSheet from './LookupSheet.svelte'

  let {
    open = $bindable(false),
    catalogId,
    item = null,
    shopId = null,
  }: { open: boolean; catalogId: string; item?: Item | null; shopId?: string | null } = $props()

  const entry = $derived(shopping.catalog.find((c) => c.id === catalogId))
  const products = $derived(shopping.productsOf(catalogId))
  const shopName = $derived(shopping.shop(shopId)?.name)
  const prefAt = (s: string | null) => shopping.preferences.find((p) => p.catalog_item_id === catalogId && p.shop_id === s)?.product_id

  let adding = $state(false)
  let brand = $state('')
  let name = $state('')
  let code = $state('')
  let image = $state<string | null>(null)
  let lookupOpen = $state(false)
  let lookupMode = $state<LookupMode>({ kind: 'product' })

  function choose(p: Product | null) {
    if (!item) return
    shopping.pin(item, p?.id ?? null)
    open = false
  }

  function prefer(p: Product, at: string | null) {
    shopping.setPreferred(catalogId, prefAt(at) === p.id ? null : p.id, at)
  }

  async function add(e: Event) {
    e.preventDefault()
    if (!brand.trim() && !name.trim()) return toast('Give a brand or a name', 'error')
    const barcode = code.trim() ? cleanBarcode(code) : null
    if (code.trim() && !barcode) return toast('A barcode is 8 to 14 digits', 'error')
    const p = await shopping.createProduct(catalogId, { brand: brand.trim(), name: name.trim(), barcode, image_id: image })
    if (!p) return
    brand = name = code = ''
    image = null
    adding = false
    if (item) choose(p)
  }

  function find(mode: LookupMode) {
    lookupMode = mode
    lookupOpen = true
  }

  function remove(p: Product) {
    if (!confirm(`Delete ${label(p.brand, p.name)}? Items of this brand become "any brand".`)) return
    shopping.deleteProduct(p)
  }
</script>

<Sheet bind:open title={item ? `Which ${entry?.name ?? item.name}?` : `Brands of ${entry?.name ?? ''}`}>
  {#if item}
    <button class="choice" class:on={!item.product_id} onclick={() => choose(null)}>
      <span class="radio"><Icon name={item.product_id ? 'circle' : 'checkCircle'} size={22} /></span>
      <span class="text"><strong>Any brand</strong><span class="muted small">Whatever is there{shopping.preferredOf(catalogId, shopId) ? ', preferably the starred one' : ''}</span></span>
    </button>
  {/if}

  <ul>
    {#each products as p (p.id)}
      {@const everywhere = prefAt(null) === p.id}
      {@const here = shopId ? prefAt(shopId) === p.id : false}
      <li>
        {#if item}
          <button class="choice" class:on={item.product_id === p.id} onclick={() => choose(p)}>
            <span class="radio"><Icon name={item.product_id === p.id ? 'checkCircle' : 'circle'} size={22} /></span>
            {#if p.image_id}<Thumb id={p.image_id} size={44} />{/if}
            <span class="text"><strong>{label(p.brand, p.name)}</strong>{#if p.barcode}<span class="muted small">{p.barcode}</span>{/if}</span>
          </button>
        {:else}
          <span class="choice static">
            {#if p.image_id}<Thumb id={p.image_id} size={44} />{:else}<span class="noimg"><Icon name="image" size={18} /></span>{/if}
            <span class="text"><strong>{label(p.brand, p.name)}</strong>{#if p.barcode}<span class="muted small">{p.barcode}</span>{/if}</span>
          </span>
        {/if}
        <div class="tools">
          <button class="chip" class:on={everywhere} aria-pressed={everywhere} onclick={() => prefer(p, null)} title="Preferred everywhere">
            <Icon name="star" size={14} /> Preferred
          </button>
          {#if shopId}
            <button class="chip" class:on={here} aria-pressed={here} onclick={() => prefer(p, shopId)} title="Preferred at {shopName}">
              <Icon name="star" size={14} /> At {shopName}
            </button>
          {/if}
          <PhotoPicker label="Photo" onpicked={(id) => shopping.patchProduct(p, { image_id: id })} />
          <button class="btn small" onclick={() => find({ kind: 'product-picture', productId: p.id })}><Icon name="search" size={14} /> Picture</button>
          <button class="icon-btn" aria-label="Delete {label(p.brand, p.name)}" onclick={() => remove(p)}><Icon name="trash" size={18} /></button>
        </div>
      </li>
    {:else}
      <li class="muted small">No brands yet. Add the ones you buy, with a photo, or find them on Open Food Facts.</li>
    {/each}
  </ul>

  {#if adding}
    <form class="fields" onsubmit={add}>
      <label class="field"><span>Brand</span><input bind:value={brand} placeholder="Alma" maxlength="200" /></label>
      <label class="field"><span>Product</span><input bind:value={name} placeholder="Piim 2.5% 1 l" maxlength="200" /></label>
      <label class="field"><span>Barcode (optional)</span><input bind:value={code} inputmode="numeric" maxlength="20" /></label>
      <div class="field">
        <span>Picture</span>
        <div class="pic">{#if image}<Thumb id={image} size={44} />{/if}<PhotoPicker onpicked={(id) => (image = id)} /></div>
      </div>
      <div class="actions wide">
        <button class="btn primary" type="submit">Add brand</button>
        <button class="btn" type="button" onclick={() => (adding = false)}>Cancel</button>
      </div>
    </form>
  {:else}
    <div class="actions">
      <button class="btn" onclick={() => (adding = true)}><Icon name="plus" size={16} /> Add a brand</button>
      <button class="btn" onclick={() => find({ kind: 'product' })}><Icon name="search" size={16} /> Find on Open Food Facts</button>
    </div>
  {/if}

  {#if entry}
    <section class="own">
      <h3>Picture of {entry.name} (any brand)</h3>
      <div class="pic">
        {#if entry.image_id}<Thumb id={entry.image_id} size={56} />{/if}
        <PhotoPicker onpicked={(id) => shopping.patchCatalog(entry, { image_id: id })} />
        <button class="btn small" onclick={() => find({ kind: 'catalog-picture' })}><Icon name="search" size={14} /> Find</button>
        {#if entry.image_id}<button class="btn small" onclick={() => shopping.patchCatalog(entry, { image_id: null })}>Remove</button>{/if}
      </div>
    </section>
  {/if}
</Sheet>

<LookupSheet
  bind:open={lookupOpen}
  {catalogId}
  query={entry?.name ?? item?.name ?? ''}
  mode={lookupMode}
  ondone={(p) => {
    if (p && item && lookupMode.kind === 'product') choose(p)
  }}
/>

<style>
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  li {
    border-top: 1px solid var(--border);
    padding: 6px 0;
  }
  .choice {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: 48px;
    padding: 4px 6px;
    border-radius: var(--radius-sm);
    text-align: left;
  }
  button.choice:hover {
    background: var(--surface-2);
  }
  .choice.on .radio {
    color: var(--accent);
  }
  .radio {
    display: grid;
    color: var(--muted);
  }
  .text {
    display: flex;
    flex-direction: column;
    min-width: 0;
    overflow-wrap: anywhere;
  }
  .small {
    font-size: 13px;
  }
  .tools {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    padding: 4px 0 2px 38px;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    min-height: 32px;
    padding: 2px 10px;
    border: 1px solid var(--border);
    border-radius: 999px;
    font-size: 13px;
    color: var(--muted);
  }
  .chip.on {
    color: var(--star);
    background: var(--star-soft);
    border-color: var(--star);
  }
  .noimg {
    display: grid;
    place-items: center;
    width: 44px;
    height: 44px;
    border-radius: 6px;
    background: var(--surface-2);
    color: var(--muted);
  }
  .pic {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }
  .own {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding-top: 8px;
    border-top: 1px solid var(--border);
  }
  h3 {
    font-size: 14px;
    margin: 0;
  }
</style>
