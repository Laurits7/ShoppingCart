<script lang="ts">
  // Add or edit a wish.
  import type { WishItem } from '../../lib/api/types/WishItem'
  import type { Wishlist } from '../../lib/api/types/Wishlist'
  import Sheet from '../../lib/components/Sheet.svelte'
  import { toast } from '../../lib/toast.svelte'
  import { wishes, type WishInput } from '../../lib/wishes.svelte'

  let { open = $bindable(false), wishlist, item = null }: { open: boolean; wishlist: Wishlist; item?: WishItem | null } = $props()
  let f = $state({ title: '', url: '', price: '', currency: '', note: '', priority: 0, section: '' })

  $effect(() => {
    if (open) {
      f = item
        ? { title: item.title, url: item.url ?? '', price: item.price === null ? '' : String(item.price), currency: item.currency ?? wishes.currency, note: item.note, priority: Number(item.priority), section: item.section_id ?? '' }
        : { title: '', url: '', price: '', currency: wishes.currency, note: '', priority: 0, section: '' }
    }
  })

  const sections = $derived(wishes.sectionsOf(wishlist.id))
  const others = $derived(wishes.wishlists.filter((w) => w.id !== wishlist.id && w.role !== 'viewer'))

  function save(e: Event) {
    e.preventDefault()
    const title = f.title.trim()
    if (!title) return
    let url = f.url.trim() || null
    if (url && !/^https?:\/\//i.test(url)) url = `https://${url}`
    const price = f.price.trim() ? Number(f.price.trim().replace(',', '.')) : null
    if (price !== null && !(price >= 0)) return toast('The price must be a number', 'error')
    const currency = /^[A-Za-z]{3}$/.test(f.currency.trim()) ? f.currency.trim().toUpperCase() : wishes.currency
    const w: WishInput = { title, url, price, currency: price === null ? null : currency, note: f.note.trim(), priority: f.priority, section_id: f.section || null }
    if (item) wishes.editItem(item, w)
    else wishes.addItem(wishlist.id, w)
    open = false
  }
</script>

<Sheet bind:open title={item ? 'Edit wish' : 'Add a wish'}>
  <form class="fields" onsubmit={save}>
    <label class="field wide"><span>What</span><input bind:value={f.title} maxlength="200" required placeholder="A good book about birds" /></label>
    <label class="field wide"><span>Link (optional)</span><input bind:value={f.url} inputmode="url" maxlength="2000" placeholder="https://…" /></label>
    <label class="field"><span>Price</span><input bind:value={f.price} inputmode="decimal" placeholder="e.g. 25" /></label>
    <label class="field"><span>Currency</span><input bind:value={f.currency} maxlength="3" /></label>
    <label class="field">
      <span>How much</span>
      <select bind:value={f.priority}>
        <option value={0}>Would be nice</option>
        <option value={1}>★ Would like a lot</option>
        <option value={2}>★★ Most wanted</option>
      </select>
    </label>
    <label class="field">
      <span>Section</span>
      <select bind:value={f.section}>
        <option value="">None</option>
        {#each sections as s (s.id)}<option value={s.id}>{s.name}</option>{/each}
      </select>
    </label>
    <label class="field wide"><span>Note</span><textarea bind:value={f.note} maxlength="1000" placeholder="Size, colour, where to find it…"></textarea></label>
    <div class="actions wide">
      <button class="btn primary" type="submit">{item ? 'Save' : 'Add'}</button>
      {#if item}
        <button class="btn" type="button" onclick={() => (wishes.editItem(item, { received: !item.received_at }), (open = false))}>
          {item.received_at ? 'Not received' : 'Received'}
        </button>
        {#if others.length}
          <select
            class="copy"
            aria-label="Copy to another wishlist"
            onchange={(e) => {
              const to = others.find((w) => w.id === e.currentTarget.value)
              if (to && item) wishes.copyItem(item, to)
              e.currentTarget.value = ''
            }}
          >
            <option value="">Copy to…</option>
            {#each others as w (w.id)}<option value={w.id}>{w.title}</option>{/each}
          </select>
        {/if}
        <span class="spacer"></span>
        <button class="btn danger" type="button" onclick={() => (wishes.removeItem(item), (open = false))}>Delete</button>
      {/if}
    </div>
  </form>
</Sheet>

<style>
  .copy {
    width: auto;
  }
</style>
