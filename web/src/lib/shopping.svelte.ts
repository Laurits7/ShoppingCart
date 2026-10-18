// Client-side copy of the Shopping tab: shops, lists, templates, the inbox, categories, items, the item
// catalogue and products. Changes are applied here first (instant UI). Item and category
// changes go through the outbox (sent in order, kept across a dead zone); shop, list,
// catalogue and product changes need the connection. Other devices' changes arrive by
// `/sync` when the event stream says something changed (lib/live.ts).

import { api, ApiError, message } from './api/client'
import type { AddedItem } from './api/types/AddedItem'
import type { CatalogItem } from './api/types/CatalogItem'
import type { Category } from './api/types/Category'
import type { Finished } from './api/types/Finished'
import type { Image } from './api/types/Image'
import type { Item } from './api/types/Item'
import type { ListBundle } from './api/types/ListBundle'
import type { LookupResult } from './api/types/LookupResult'
import type { Product } from './api/types/Product'
import type { ProductPreference } from './api/types/ProductPreference'
import type { Shop } from './api/types/Shop'
import type { ShopBundle } from './api/types/ShopBundle'
import type { ShoppingData } from './api/types/ShoppingData'
import { IdbStore, MemoryStore, Outbox, type Entry } from './outbox.svelte'
import { saveSnapshot } from './snapshot'
import type { ShoppingList } from './api/types/ShoppingList'
import { combine, formatAmount, step as stepAmount } from './amount'
import { frequent as rankFrequent, normalizeName, rank, type Usage } from './catalog'
import { neighbours } from './dndmath'
import { keyBetween } from './order'
import { label as productLabel, picture, preferred, type PictureFrom } from './products'
import { session } from './session.svelte'
import { toast } from './toast.svelte'
import { ulid } from './ulid'
import { wishes } from './wishes.svelte'

type Positioned = { id: string; position: string }
export const byPosition = (a: Positioned, b: Positioned) =>
  a.position < b.position ? -1 : a.position > b.position ? 1 : a.id < b.id ? -1 : a.id > b.id ? 1 : 0

const now = () => new Date().toISOString()

/** Update a row in place (components may hold it) or add it. */
function upsert<T extends { id: string }>(arr: T[], row: T) {
  const old = arr.find((x) => x.id === row.id)
  if (old) Object.assign(old, row)
  else arr.push(row)
}

function removeWhere<T>(arr: T[], pred: (x: T) => boolean) {
  for (let i = arr.length - 1; i >= 0; i--) if (pred(arr[i])) arr.splice(i, 1)
}

const lastKey = (rows: Positioned[]) => rows.reduce<string>((m, r) => (r.position > m ? r.position : m), '')

/** What to add: a name with an optional amount/unit (null = the catalogue's usual one) and
 * category (undefined = the catalogue's usual one, null = Other). */
/** A category of a list with its open items ("Other" has id 'other'). */
export type Section = { id: string; name: string; items: Item[] }

export type NewItem = { name: string; amount: number | null; unit: string | null; categoryId?: string | null; productId?: string | null }

/** What an item shows: its picture (and where it comes from) and its brand line. */
export type Look = { image: string | null; from: PictureFrom | null; brand: string | null }

class ShoppingStore {
  loaded = $state(false)
  shops = $state<Shop[]>([])
  lists = $state<ShoppingList[]>([])
  categories = $state<Category[]>([])
  items = $state<Item[]>([])
  catalog = $state<CatalogItem[]>([])
  products = $state<Product[]>([])
  preferences = $state<ProductPreference[]>([])
  images = $state<Image[]>([])
  /** The rev of the last sync. */
  rev = 0
  outbox = new Outbox(typeof indexedDB === 'undefined' ? new MemoryStore() : new IdbStore(), (e) => api.send(e.method, e.path, e.body))
  #syncing: Promise<void> | null = null
  #again = false

  constructor() {
    this.outbox.onrefused = ({ error }) => {
      toast(error.message, 'error')
      this.load().catch(() => {})
    }
    // Once everything waiting is through, catch up with what others did meanwhile.
    this.outbox.ondrained = () => void this.sync()
  }

  /** Forget everything (signed out); `queue` also drops changes not yet sent. */
  reset(queue = false) {
    if (queue) this.outbox.clear()
    wishes.reset()
    this.rev = 0
    this.loaded = false
    this.shops = []
    this.lists = []
    this.categories = []
    this.items = []
    this.catalog = []
    this.products = []
    this.preferences = []
    this.images = []
  }

  async load() {
    this.apply(await api.get<ShoppingData>('/sync?since=0'))
  }

  /** Take a full copy of the data (from the server, or the saved copy when offline). */
  apply(d: ShoppingData) {
    // Changes still waiting in the outbox stay as they are locally.
    const waiting = this.outbox.keys()
    const keep = <T extends { id: string }>(fresh: T[], old: T[]) => [...fresh.filter((r) => !waiting.has(r.id)), ...old.filter((r) => waiting.has(r.id))]
    this.rev = d.rev
    this.shops = d.shops
    this.lists = d.lists
    this.categories = keep(d.categories, this.categories)
    this.items = keep(d.items, this.items)
    wishes.apply(d, true, waiting)
    this.catalog = d.catalog
    this.products = d.products
    this.preferences = d.preferences
    this.images = d.images
    this.loaded = true
    this.#save()
  }

  /** Catch up with changes made elsewhere since the last sync. */
  sync(): Promise<void> {
    if (!this.loaded) return Promise.resolve()
    if (this.#syncing) {
      this.#again = true
      return this.#syncing
    }
    this.#syncing = (async () => {
      try {
        do {
          this.#again = false
          await this.#syncOnce()
        } while (this.#again)
      } catch {
        /* offline: the next nudge or reconnect tries again */
      } finally {
        this.#syncing = null
      }
    })()
    return this.#syncing
  }

  async #syncOnce() {
    const d = await api.get<ShoppingData>(`/sync?since=${this.rev}`)
    const known = new Set(this.lists.map((l) => l.id))
    // A list newly shared with me: its older items aren't in the delta, so load everything.
    if (d.visible_lists.some((id) => !known.has(id)) || wishes.hasNew(d)) return this.load()
    const waiting = this.outbox.keys()
    wishes.apply(d, false, waiting)
    const merge = <T extends { id: string }>(arr: T[], rows: T[]) => {
      for (const r of rows) if (!waiting.has(r.id)) upsert(arr, r)
    }
    merge(this.shops, d.shops)
    merge(this.lists, d.lists)
    merge(this.categories, d.categories)
    merge(this.items, d.items)
    merge(this.catalog, d.catalog)
    merge(this.products, d.products)
    this.preferences = d.preferences
    this.images = d.images
    const gone: Record<string, Set<string>> = {}
    for (const x of d.deleted) (gone[x.kind] ??= new Set()).add(x.id)
    const drop = <T extends { id: string }>(arr: T[], kind: string) => gone[kind] && removeWhere(arr, (r) => gone[kind].has(r.id) && !waiting.has(r.id))
    drop(this.shops, 'shop')
    drop(this.lists, 'list')
    drop(this.categories, 'category')
    drop(this.items, 'item')
    drop(this.catalog, 'catalog')
    drop(this.products, 'product')
    // Lists I can no longer see (unshared, left, deleted) go, with everything on them.
    const visible = new Set(d.visible_lists)
    const lost = new Set(this.lists.filter((l) => !visible.has(l.id)).map((l) => l.id))
    if (lost.size) {
      removeWhere(this.items, (i) => lost.has(i.list_id))
      removeWhere(this.categories, (c) => lost.has(c.list_id))
      removeWhere(this.lists, (l) => lost.has(l.id))
    }
    this.rev = d.rev
    this.#save()
  }

  #saveTimer: ReturnType<typeof setTimeout> | undefined
  /** Keep a copy on the device, a moment after the last sync (it's what opens offline). */
  #save() {
    clearTimeout(this.#saveTimer)
    this.#saveTimer = setTimeout(() => {
      const me = session.me
      if (!me || !this.loaded) return
      const data: ShoppingData = $state.snapshot({
        rev: this.rev,
        shops: this.shops,
        lists: this.lists,
        categories: this.categories,
        items: this.items,
        catalog: this.catalog,
        products: this.products,
        preferences: this.preferences,
        images: this.images,
        persons: wishes.persons,
        wishlists: wishes.wishlists,
        wish_sections: wishes.sections,
        wish_items: wishes.items,
        claims: wishes.claims,
        deleted: [],
        visible_lists: this.lists.map((l) => l.id),
        visible_wishlists: wishes.wishlists.map((w) => w.id),
      })
      saveSnapshot({ me: $state.snapshot(me), data, saved_at: new Date().toISOString() })
    }, 1500)
  }

  /** Queue a change; `apply` gets the server's answer unless a newer change to the same
   * thing is still waiting. */
  queue<T>(e: Omit<Entry, 'user'>, apply?: (res: T) => void) {
    this.#queue(e, apply)
  }

  #queue<T>(e: Omit<Entry, 'user'>, apply?: (res: T) => void) {
    this.outbox.add({ ...e, user: session.me?.id ?? '' }).then((res) => {
      if (res !== undefined && apply && !(e.key && this.outbox.keys().has(e.key))) apply(res as T)
    })
  }

  /** Send a change; on failure say why and resync with the server. */
  async #send<T>(req: () => Promise<T>): Promise<T | undefined> {
    try {
      return await req()
    } catch (e) {
      toast(message(e), 'error')
      this.load().catch(() => {})
      return undefined
    }
  }


  // ---- queries -----------------------------------------------------------------------

  get sortedShops() {
    return [...this.shops].sort(byPosition)
  }
  shop(id: string | null | undefined) {
    return this.shops.find((s) => s.id === id)
  }
  list(id: string | null | undefined) {
    return this.lists.find((l) => l.id === id)
  }
  /** A shop's shopping lists (not templates). */
  listsOf(shopId: string, archived = false) {
    return this.lists.filter((l) => l.kind === 'list' && l.shop_id === shopId && !!l.archived_at === archived).sort(byPosition)
  }
  /** A shop's templates: usual items to start lists from. */
  templatesOf(shopId: string) {
    return this.lists.filter((l) => l.kind === 'template' && l.shop_id === shopId && !l.archived_at).sort(byPosition)
  }
  /** Templates you can see, also shared ones (whatever their shop). */
  get templates() {
    return this.lists.filter((l) => l.kind === 'template' && !l.archived_at)
  }
  /** Where items without a list arrive (Mealie, the API), if it exists yet. */
  get inbox() {
    return this.lists.find((l) => l.kind === 'inbox')
  }
  categoriesOf(listId: string) {
    return this.categories.filter((c) => c.list_id === listId).sort(byPosition)
  }
  itemsOf(listId: string) {
    return this.items.filter((i) => i.list_id === listId)
  }
  openCount(listId: string) {
    return this.items.reduce((n, i) => n + (i.list_id === listId && !i.checked_at ? 1 : 0), 0)
  }
  /** Open items grouped by category in the list's order, Other last; empty ones only with
   * `withEmpty` (as drop targets while dragging). */
  sections(listId: string, withEmpty = false): Section[] {
    const open = this.itemsOf(listId)
      .filter((i) => !i.checked_at)
      .sort(byPosition)
    const cats = this.categoriesOf(listId)
    const known = new Set(cats.map((c) => c.id))
    const out: Section[] = cats.map((c) => ({ id: c.id, name: c.name, items: open.filter((i) => i.category_id === c.id) }))
    out.push({ id: 'other', name: 'Other', items: open.filter((i) => !i.category_id || !known.has(i.category_id)) })
    return withEmpty ? out : out.filter((s) => s.items.length)
  }
  /** Items in the cart, most recently ticked first. */
  cartOf(listId: string): Item[] {
    return this.itemsOf(listId)
      .filter((i) => i.checked_at)
      .sort((a, b) => (b.checked_at! < a.checked_at! ? -1 : b.checked_at! > a.checked_at! ? 1 : 0))
  }
  known(name: string) {
    const n = normalizeName(name)
    return this.catalog.find((c) => normalizeName(c.name) === n)
  }

  // ---- products and pictures ------------------------------------------------------------

  productsOf(catalogId: string | null | undefined) {
    return this.products.filter((p) => p.catalog_item_id === catalogId).sort(byPosition)
  }
  product(id: string | null | undefined) {
    return this.products.find((p) => p.id === id)
  }
  /** The catalogue entry an item belongs to (by link, else by name). */
  entryOf(item: { catalog_item_id: string | null; name: string }) {
    return this.catalog.find((c) => c.id === item.catalog_item_id) ?? this.known(item.name)
  }
  /** The preferred product of a catalogue item at a shop (or everywhere). */
  preferredOf(catalogId: string, shopId: string | null) {
    const at = (s: string | null) => this.preferences.find((p) => p.catalog_item_id === catalogId && p.shop_id === s)?.product_id
    return preferred(shopId ? at(shopId) : undefined, at(null))
  }
  /** The picture and brand line an item shows (SPEC §6.4b). */
  look(item: Item): Look {
    const entry = this.entryOf(item)
    const shopId = this.list(item.list_id)?.shop_id ?? null
    const pinned = item.product_id ? this.product(item.product_id) : undefined
    const prefId = !item.product_id && entry ? this.preferredOf(entry.id, shopId) : null
    const pref = prefId ? this.product(prefId) : undefined
    const pic = picture(item.product_id ? (pinned?.image_id ?? null) : undefined, pref ? pref.image_id : undefined, entry?.image_id)
    const brand = pinned ? productLabel(pinned.brand, pinned.name) : pref ? `usually ${productLabel(pref.brand, pref.name)}` : null
    return { image: pic?.image ?? null, from: pic?.from ?? null, brand }
  }
  /** Credit for a picture from Open Food Facts (CC BY-SA), if it is one. */
  credit(imageId: string): string | null {
    const img = this.images.find((i) => i.id === imageId)
    return img?.source === 'openfoodfacts' ? (img.source_ref ?? 'https://world.openfoodfacts.org') : null
  }

  async uploadImage(file: Blob): Promise<Image | undefined> {
    const img = await this.#send(() => api.upload<Image>('/images', file))
    if (img) upsert(this.images, img)
    return img
  }

  async createProduct(catalogId: string, p: { brand: string; name: string; barcode: string | null; image_id: string | null }) {
    const res = await this.#send(() => api.post<Product>(`/catalog/${catalogId}/products`, { id: ulid(), ...p }))
    if (res) upsert(this.products, res)
    return res
  }

  async patchProduct(p: Product, patch: { brand?: string; name?: string; barcode?: string | null; image_id?: string | null }) {
    const res = await this.#send(() => api.patch<Product>(`/products/${p.id}`, patch))
    if (res) upsert(this.products, res)
    return res
  }

  async deleteProduct(p: Product) {
    removeWhere(this.products, (x) => x.id === p.id)
    removeWhere(this.preferences, (x) => x.product_id === p.id)
    for (const i of this.items) if (i.product_id === p.id) i.product_id = null
    await this.#send(() => api.del(`/products/${p.id}`))
  }

  async setPreferred(catalogId: string, productId: string | null, shopId: string | null) {
    const res = await this.#send(() => api.put<ProductPreference[]>(`/catalog/${catalogId}/preferred`, { product_id: productId, shop_id: shopId }))
    if (!res) return
    removeWhere(this.preferences, (p) => p.catalog_item_id === catalogId)
    this.preferences.push(...res)
  }

  /** Pin an item to a product, or make it generic again (null). */
  pin(item: Item, productId: string | null) {
    item.product_id = productId
    this.#queue<Item>({ method: 'PATCH', path: `/items/${item.id}`, body: { product_id: productId }, key: item.id }, (res) => upsert(this.items, res))
  }

  lookup(q: string) {
    return api.get<LookupResult[]>(`/lookup?q=${encodeURIComponent(q)}`)
  }
  lookupBarcode(code: string) {
    return api.get<LookupResult>(`/lookup/barcode/${encodeURIComponent(code)}`)
  }
  /** Use a lookup result: a new product of the catalogue item (default), its picture for an
   * existing product, or (pictureOnly) for the catalogue item itself. */
  async importLookup(code: string, catalogId: string, opts: { productId?: string; pictureOnly?: boolean } = {}) {
    const res = await this.#send(() =>
      api.post<Product | null>(`/lookup/${code}/import`, { catalog_item_id: catalogId, product_id: opts.productId, picture_only: !!opts.pictureOnly }),
    )
    if (res === undefined) return undefined
    if (res) upsert(this.products, res)
    // The new picture's credit and (for pictureOnly) the catalogue item's picture.
    await this.load().catch(() => {})
    return res
  }

  /** Add what a barcode is: your product with that barcode, or one found on Open Food Facts
   * (which becomes a product of a new or existing catalogue item). */
  async addScanned(listId: string, code: string) {
    const mine = this.products.find((p) => p.barcode === code)
    const entry = mine && this.catalog.find((c) => c.id === mine.catalog_item_id)
    if (mine && entry) {
      this.add(listId, { name: entry.name, amount: null, unit: null, productId: mine.id })
      toast(`Added ${entry.name} · ${productLabel(mine.brand, mine.name)}`)
      return
    }
    let found: LookupResult
    try {
      found = await this.lookupBarcode(code)
    } catch (e) {
      toast(e instanceof ApiError && e.status === 404 ? `${code} isn't on Open Food Facts; add it by name instead` : message(e), 'error')
      return
    }
    const name = found.name || found.brand
    const added = await this.#send(() => api.post<AddedItem>(`/lists/${listId}/items`, { id: ulid(), name }))
    if (!added) return
    upsert(this.items, added.item)
    const catalogId = added.item.catalog_item_id
    const product = catalogId ? await this.importLookup(code, catalogId) : null
    const item = this.items.find((i) => i.id === added.item.id)
    if (product && item) this.pin(item, product.id)
    toast(`Added ${name}`)
  }

  #usage(c: CatalogItem, shopId: string | null, t: number): Usage {
    return {
      useCount: c.use_count,
      daysSinceUse: c.last_used_at ? (t - Date.parse(c.last_used_at)) / 86_400_000 : 365,
      shopCount: (shopId && c.shops.find((s) => s.shop_id === shopId)?.use_count) || 0,
    }
  }

  /** Catalogue entries matching what's typed, best first. */
  suggest(query: string, shopId: string | null, limit = 6): CatalogItem[] {
    const t = Date.now()
    const entries = this.catalog.filter((c) => !c.hidden)
    const order = rank(
      entries.map((c) => [normalizeName(c.name), this.#usage(c, shopId, t)]),
      query,
      limit,
    )
    return order.map((i) => entries[i])
  }

  /** What's usually bought at the list's shop and isn't open on the list. */
  frequentFor(listId: string, limit = 8): CatalogItem[] {
    const shopId = this.list(listId)?.shop_id ?? null
    const open = new Set(this.itemsOf(listId).filter((i) => !i.checked_at).map((i) => normalizeName(i.name)))
    const t = Date.now()
    const entries = this.catalog.filter((c) => !c.hidden && !open.has(normalizeName(c.name)))
    const order = rankFrequent(
      entries.map((c) => [normalizeName(c.name), this.#usage(c, shopId, t)]),
      limit,
    )
    return order.map((i) => entries[i])
  }

  // ---- items ---------------------------------------------------------------------------

  /** Apply an add locally the way the server will (catalogue defaults, duplicate guard).
   * Returns the new item's id, the request body and, when merged, the item and its amount before. */
  #applyAdd(listId: string, input: NewItem) {
    const list = this.list(listId)
    const known = this.known(input.name)
    let amount: number
    let unit: string | null
    let step: number | null = null
    if (input.amount !== null) {
      amount = input.amount
      unit = input.unit
    } else if (known && input.unit === null) {
      amount = known.amount
      unit = known.unit
      step = known.step
    } else {
      amount = 1
      unit = input.unit
    }
    let categoryId: string | null = null
    if (input.categoryId !== undefined) categoryId = input.categoryId
    else if (known?.category_name) {
      const want = normalizeName(known.category_name)
      categoryId = this.categoriesOf(listId).find((c) => normalizeName(c.name) === want)?.id ?? null
    }
    const id = ulid()
    const productId = input.productId ?? null
    const body = { id, name: input.name, amount, unit: unit ?? '', step, category_id: categoryId, product_id: productId }
    const norm = normalizeName(input.name)
    // The same thing of another brand gets a row of its own.
    const dup = this.itemsOf(listId)
      .filter((i) => !i.checked_at && normalizeName(i.name) === norm && (i.product_id ?? null) === productId)
      .sort(byPosition)
      .find((i) => combine(i.amount, i.unit, amount, unit) !== null)
    const ts = now()
    let merged: { item: Item; before: number } | null = null
    if (dup) {
      merged = { item: dup, before: dup.amount }
      dup.amount = combine(dup.amount, dup.unit, amount, unit)!
      dup.updated_at = ts
    } else {
      this.items.push({
        id,
        list_id: listId,
        category_id: categoryId,
        catalog_item_id: known?.id ?? null,
        product_id: productId,
        name: input.name,
        amount,
        unit,
        step,
        note: '',
        position: keyBetween(lastKey(this.itemsOf(listId)), null),
        checked_at: null,
        checked_by: null,
        added_by: session.me?.id ?? '',
        source: 'manual',
        external_ref: null,
        created_at: ts,
        updated_at: ts,
        rev: 0,
      })
    }
    // The catalogue learns at once, so suggestions and chips follow.
    const catName = this.categories.find((c) => c.id === (dup ? dup.category_id : categoryId))?.name ?? null
    this.#learn(input.name, amount, unit, catName, list?.shop_id ?? null)
    return { id, body, merged }
  }

  #learn(name: string, amount: number, unit: string | null, categoryName: string | null, shopId: string | null) {
    const ts = now()
    let c = this.known(name)
    if (!c) {
      // Provisional until the server's id arrives with the item (or the next load).
      c = { id: ulid(), name, category_name: null, amount, unit, step: null, use_count: 0, last_used_at: null, shop_id: null, hidden: false, image_id: null, shops: [], created_at: ts, updated_at: ts, rev: 0 }
      this.catalog.push(c)
      c = this.catalog[this.catalog.length - 1]
    }
    c.use_count += 1
    c.last_used_at = ts
    c.amount = amount
    c.unit = unit
    if (categoryName) c.category_name = categoryName
    if (shopId) {
      c.shop_id = shopId
      const u = c.shops.find((s) => s.shop_id === shopId)
      if (u) {
        u.use_count += 1
        u.last_used_at = ts
      } else c.shops.push({ shop_id: shopId, use_count: 1, last_used_at: ts })
    }
  }

  /** Put the server's answer to an add in place of the local guess. */
  #applyAdded(localId: string, res: AddedItem) {
    if (res.item.id !== localId) removeWhere(this.items, (i) => i.id === localId)
    upsert(this.items, res.item)
    if (res.item.catalog_item_id) {
      const c = this.known(res.item.name)
      if (c && c.id !== res.item.catalog_item_id && c.rev === 0) c.id = res.item.catalog_item_id
    }
  }

  #mergedToast(merged: { item: Item; before: number }) {
    const it = merged.item
    toast(`${it.name}: ${formatAmount(merged.before, it.unit)} → ${formatAmount(it.amount, it.unit)}`, 'info', {
      label: 'Undo',
      run: () => this.editItem(it, { amount: merged.before }),
    })
  }

  add(listId: string, input: NewItem) {
    const { id, body, merged } = this.#applyAdd(listId, input)
    if (merged) this.#mergedToast(merged)
    this.#queue<AddedItem>({ method: 'POST', path: `/lists/${listId}/items`, body, key: merged?.item.id ?? id }, (res) => this.#applyAdded(id, res))
  }

  tick(item: Item, checked: boolean) {
    item.checked_at = checked ? now() : null
    item.checked_by = checked ? (session.me?.id ?? null) : null
    this.#queue<Item>({ method: 'PATCH', path: `/items/${item.id}`, body: { checked }, key: item.id }, (res) => upsert(this.items, res))
  }

  step(item: Item, up: boolean) {
    const next = stepAmount(item.amount, item.unit, item.step, up)
    if (next === null) {
      this.remove(item)
      return
    }
    item.amount = next
    // The new amount rather than "one more", so a retried request can't count twice.
    this.#queue<Item>({ method: 'PATCH', path: `/items/${item.id}`, body: { amount: next }, key: item.id }, (res) => upsert(this.items, res))
  }

  editItem(item: Item, patch: { name?: string; amount?: number; unit?: string | null; step?: number | null; note?: string; category_id?: string | null }) {
    Object.assign(item, patch)
    if ('category_id' in patch) {
      const c = this.known(item.name)
      if (c) c.category_name = this.categories.find((x) => x.id === patch.category_id)?.name ?? null
    }
    const body = { ...patch, ...('unit' in patch ? { unit: patch.unit ?? '' } : {}) }
    this.#queue<Item>({ method: 'PATCH', path: `/items/${item.id}`, body, key: item.id }, (res) => upsert(this.items, res))
  }

  remove(item: Item) {
    removeWhere(this.items, (i) => i.id === item.id)
    this.#queue({ method: 'DELETE', path: `/items/${item.id}`, key: item.id })
    toast(`Removed ${item.name}`, 'info', { label: 'Undo', run: () => this.restore([item]) })
  }

  restore(rows: Item[]) {
    for (const r of rows) upsert(this.items, r)
    this.#queue<Item[]>({ method: 'POST', path: '/items/restore', body: { ids: rows.map((r) => r.id) } }, (res) => {
      for (const r of res) upsert(this.items, r)
    })
  }

  finish(listId: string) {
    const done = this.itemsOf(listId).filter((i) => i.checked_at)
    if (!done.length) return
    removeWhere(this.items, (i) => i.list_id === listId && !!i.checked_at)
    toast(`Finished: ${done.length} item${done.length === 1 ? '' : 's'} cleared`, 'info', { label: 'Undo', run: () => this.restore(done) })
    this.#queue<Finished>({ method: 'POST', path: `/lists/${listId}/finish`, body: {} })
  }

  // ---- templates and the inbox --------------------------------------------------------------

  /** Add a template's items (all, or some) to a list; ones already open get the amount added. */
  addFromTemplate(listId: string, template: ShoppingList, itemIds?: string[]) {
    const from = this.itemsOf(template.id).filter((i) => !itemIds || itemIds.includes(i.id)).sort(byPosition)
    const sent = from.map((t) => {
      const cat = this.categories.find((c) => c.id === t.category_id)?.name ?? null
      const r = this.#applyAdd(listId, { name: t.name, amount: t.amount, unit: t.unit, categoryId: this.#categoryNamed(listId, cat), productId: t.product_id })
      return { item_id: t.id, id: r.id }
    })
    if (sent.length) toast(`Added ${sent.length} item${sent.length === 1 ? '' : 's'} from ${template.name}`)
    this.#queue<AddedItem[]>({ method: 'POST', path: `/lists/${listId}/from-template`, body: { template_id: template.id, items: sent } }, (res) =>
      res.forEach((r, i) => this.#applyAdded(sent[i].id, r)),
    )
  }

  /** The list's category of that name: undefined (let the catalogue decide) if none given. */
  #categoryNamed(listId: string, name: string | null): string | null | undefined {
    if (!name) return undefined
    const want = normalizeName(name)
    return this.categoriesOf(listId).find((c) => normalizeName(c.name) === want)?.id
  }

  /** Put an item into a template too (e.g. from a list), in its category of the same name. */
  addToTemplate(item: Item, template: ShoppingList) {
    const cat = this.categories.find((c) => c.id === item.category_id)?.name ?? null
    this.add(template.id, { name: item.name, amount: item.amount, unit: item.unit, categoryId: this.#categoryNamed(template.id, cat) ?? null, productId: item.product_id })
    toast(`${item.name} is now in ${template.name}`)
  }

  /** Add to the inbox (created on first use). */
  async addToInbox(text: string) {
    const res = await this.#send(() => api.post<AddedItem>('/inbox/items', { id: ulid(), text }))
    if (!res) return
    if (!this.inbox) await this.load()
    else upsert(this.items, res.item)
  }

  // ---- shops, lists, categories ------------------------------------------------------------

  #applyBundle(b: { lists: ShoppingList[]; categories: Category[] }) {
    for (const l of b.lists) upsert(this.lists, l)
    for (const c of b.categories) upsert(this.categories, c)
  }

  /** Create a shop with its default list; returns the list's id. */
  async createShop(name: string, color: string | null = null): Promise<string | undefined> {
    const res = await this.#send(() => api.post<ShopBundle>('/shops', { id: ulid(), list_id: ulid(), name, color }))
    if (!res) return undefined
    upsert(this.shops, res.shop)
    this.#applyBundle(res)
    return res.lists[0]?.id
  }

  async patchShop(shop: Shop, patch: { name?: string; color?: string | null }) {
    Object.assign(shop, patch)
    const res = await this.#send(() => api.patch<Shop>(`/shops/${shop.id}`, patch))
    if (res) upsert(this.shops, res)
  }

  async deleteShop(shop: Shop) {
    const lists = new Set(this.lists.filter((l) => l.shop_id === shop.id).map((l) => l.id))
    removeWhere(this.items, (i) => lists.has(i.list_id))
    removeWhere(this.categories, (c) => lists.has(c.list_id))
    removeWhere(this.lists, (l) => lists.has(l.id))
    removeWhere(this.shops, (s) => s.id === shop.id)
    for (const c of this.catalog) removeWhere(c.shops, (u) => u.shop_id === shop.id)
    await this.#send(() => api.del(`/shops/${shop.id}`))
  }

  /** A new list (empty, or a copy of a template) or a new template; returns its id. */
  async createList(shopId: string, name: string, opts: { kind?: 'list' | 'template'; templateId?: string | null } = {}): Promise<string | undefined> {
    const res = await this.#send(() => api.post<ListBundle>('/lists', { id: ulid(), shop_id: shopId, name, kind: opts.kind ?? 'list', template_id: opts.templateId ?? null }))
    if (!res) return undefined
    this.#applyBundle({ lists: [res.list], categories: res.categories })
    return res.list.id
  }

  async patchList(list: ShoppingList, patch: { name?: string; archived?: boolean }) {
    const res = await this.#send(() => api.patch<ShoppingList>(`/lists/${list.id}`, patch))
    if (res) upsert(this.lists, res)
  }

  async deleteList(list: ShoppingList) {
    removeWhere(this.items, (i) => i.list_id === list.id)
    removeWhere(this.categories, (c) => c.list_id === list.id)
    removeWhere(this.lists, (l) => l.id === list.id)
    await this.#send(() => api.del(`/lists/${list.id}`))
  }

  createCategory(listId: string, name: string) {
    const id = ulid()
    const ts = now()
    this.categories.push({ id, list_id: listId, name, position: keyBetween(lastKey(this.categoriesOf(listId)), null), created_at: ts, updated_at: ts, rev: 0 })
    this.#queue<Category>({ method: 'POST', path: `/lists/${listId}/categories`, body: { id, name }, key: id }, (res) => upsert(this.categories, res))
  }

  renameCategory(c: Category, name: string) {
    c.name = name
    this.#queue<Category>({ method: 'PATCH', path: `/categories/${c.id}`, body: { name }, key: c.id }, (res) => upsert(this.categories, res))
  }

  deleteCategory(c: Category) {
    for (const i of this.items) if (i.category_id === c.id) i.category_id = null
    removeWhere(this.categories, (x) => x.id === c.id)
    this.#queue({ method: 'DELETE', path: `/categories/${c.id}`, key: c.id })
  }

  // ---- moving (drag and drop) -----------------------------------------------------------------

  /** Put `row` at `index` among `siblings` (in display order, without `row`) locally; returns
   * the neighbours to name to the server. */
  #slot(row: Positioned, siblings: Positioned[], index: number) {
    const lo = siblings[index - 1]?.position ?? ''
    const hi = siblings[index]?.position ?? null
    row.position = hi !== null && lo >= hi ? keyBetween(lo, null) : keyBetween(lo, hi)
    return neighbours(
      siblings.map((r) => r.id),
      index,
    )
  }

  /** Open items of a list in one category (null = Other), in order. */
  #inCategory(listId: string, categoryId: string | null) {
    const known = new Set(this.categoriesOf(listId).map((c) => c.id))
    return this.itemsOf(listId)
      .filter((i) => !i.checked_at && (categoryId === null ? !i.category_id || !known.has(i.category_id) : i.category_id === categoryId))
      .sort(byPosition)
  }

  /** Move an item to `index` within a category (null = Other) of a list, maybe another list. */
  moveItem(item: Item, listId: string, categoryId: string | null, index: number) {
    const siblings = this.#inCategory(listId, categoryId).filter((i) => i.id !== item.id)
    const otherList = listId !== item.list_id
    const otherCategory = otherList || categoryId !== (item.category_id ?? null)
    const place = this.#slot(item, siblings, index)
    item.list_id = listId
    item.category_id = categoryId
    if (otherCategory && !otherList) {
      const c = this.known(item.name)
      if (c) c.category_name = this.categories.find((x) => x.id === categoryId)?.name ?? c.category_name
    }
    const body = { ...place, category_id: categoryId, ...(otherList ? { list_id: listId } : {}) }
    this.#queue<Item>({ method: 'POST', path: `/items/${item.id}/move`, body, key: item.id }, (res) => upsert(this.items, res))
  }

  /** Move items to the end of another list, keeping a category of the same name. */
  moveItems(items: Item[], listId: string) {
    const targetCats = this.categoriesOf(listId)
    let last = lastKey(this.itemsOf(listId))
    for (const it of items) {
      const name = this.categories.find((c) => c.id === it.category_id)?.name
      const want = name ? normalizeName(name) : null
      it.category_id = (want && targetCats.find((c) => normalizeName(c.name) === want)?.id) || null
      it.list_id = listId
      last = keyBetween(last, null)
      it.position = last
    }
    const list = this.list(listId)
    toast(`Moved ${items.length} item${items.length === 1 ? '' : 's'} to ${list?.name ?? 'the list'}`)
    this.#queue<Item[]>({ method: 'POST', path: '/items/move', body: { ids: items.map((i) => i.id), list_id: listId } }, (res) => {
      for (const r of res) upsert(this.items, r)
    })
  }

  async #move<T extends Positioned>(arr: T[], row: T, siblings: T[], index: number, url: string) {
    const place = this.#slot(
      row,
      siblings.filter((r) => r.id !== row.id),
      index,
    )
    const res = await this.#send(() => api.post<T>(url, place))
    if (res) upsert(arr, res)
  }

  moveCategory(c: Category, index: number) {
    const place = this.#slot(
      c,
      this.categoriesOf(c.list_id).filter((x) => x.id !== c.id),
      index,
    )
    this.#queue<Category>({ method: 'POST', path: `/categories/${c.id}/move`, body: place, key: c.id }, (res) => upsert(this.categories, res))
  }
  moveList(l: ShoppingList, index: number) {
    const siblings = l.kind === 'template' ? this.templatesOf(l.shop_id ?? '') : this.listsOf(l.shop_id ?? '')
    return this.#move(this.lists, l, siblings, index, `/lists/${l.id}/move`)
  }
  moveShop(s: Shop, index: number) {
    return this.#move(this.shops, s, this.sortedShops, index, `/shops/${s.id}/move`)
  }

  // ---- catalogue management ------------------------------------------------------------------

  async reloadCatalog() {
    this.catalog = await api.get<CatalogItem[]>('/catalog')
  }

  async patchCatalog(c: CatalogItem, patch: { name?: string; category_name?: string | null; amount?: number; unit?: string | null; step?: number | null; hidden?: boolean; image_id?: string | null }) {
    const body = { ...patch, ...('unit' in patch ? { unit: patch.unit ?? '' } : {}) }
    const res = await this.#send(() => api.patch<CatalogItem>(`/catalog/${c.id}`, body))
    if (res) upsert(this.catalog, res)
    return !!res
  }

  async deleteCatalog(c: CatalogItem) {
    removeWhere(this.catalog, (x) => x.id === c.id)
    for (const i of this.items) if (i.catalog_item_id === c.id) i.catalog_item_id = null
    await this.#send(() => api.del(`/catalog/${c.id}`))
  }

  async mergeCatalog(into: CatalogItem, from: CatalogItem[]) {
    const res = await this.#send(() => api.post<CatalogItem>('/catalog/merge', { into: into.id, from: from.map((f) => f.id) }))
    if (!res) return
    const gone = new Set(from.map((f) => f.id))
    removeWhere(this.catalog, (x) => gone.has(x.id))
    upsert(this.catalog, res)
    for (const i of this.items) if (i.catalog_item_id && gone.has(i.catalog_item_id)) i.catalog_item_id = res.id
  }
}

export const shopping = new ShoppingStore()
