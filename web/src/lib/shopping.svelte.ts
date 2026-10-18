// Client-side copy of the Shopping tab: shops, lists, categories, items, staples and the
// item catalogue. Changes are applied here first (instant UI) and then sent; the server's
// answer replaces the local guess. If the server refuses, the store reloads. (The persistent
// outbox for flaky connections comes in Phase 4.)

import { api, message } from './api/client'
import type { AddedItem } from './api/types/AddedItem'
import type { CatalogItem } from './api/types/CatalogItem'
import type { Category } from './api/types/Category'
import type { Finished } from './api/types/Finished'
import type { Item } from './api/types/Item'
import type { ListBundle } from './api/types/ListBundle'
import type { Shop } from './api/types/Shop'
import type { ShopBundle } from './api/types/ShopBundle'
import type { ShoppingData } from './api/types/ShoppingData'
import type { ShoppingList } from './api/types/ShoppingList'
import type { Staple } from './api/types/Staple'
import type { Stepped } from './api/types/Stepped'
import { combine, formatAmount, step as stepAmount } from './amount'
import { frequent as rankFrequent, normalizeName, rank, type Usage } from './catalog'
import { neighbours } from './dndmath'
import { keyBetween } from './order'
import { session } from './session.svelte'
import { toast } from './toast.svelte'
import { ulid } from './ulid'

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

export type NewItem = { name: string; amount: number | null; unit: string | null; categoryId?: string | null }

class ShoppingStore {
  loaded = $state(false)
  shops = $state<Shop[]>([])
  lists = $state<ShoppingList[]>([])
  categories = $state<Category[]>([])
  items = $state<Item[]>([])
  staples = $state<Staple[]>([])
  catalog = $state<CatalogItem[]>([])
  /** Requests in flight per item: an older answer must not overwrite a newer local change. */
  #pending = new Map<string, number>()

  /** Forget everything (signed out). */
  reset() {
    this.loaded = false
    this.shops = []
    this.lists = []
    this.categories = []
    this.items = []
    this.staples = []
    this.catalog = []
  }

  async load() {
    const d = await api.get<ShoppingData>('/shopping')
    this.shops = d.shops
    this.lists = d.lists
    this.categories = d.categories
    this.items = d.items
    this.staples = d.staples
    this.catalog = d.catalog
    this.loaded = true
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

  /** Send a change to one item; its answer is applied only if no newer change is on the way. */
  async #sendItem<T>(id: string, req: () => Promise<T>, apply: (res: T) => void) {
    this.#pending.set(id, (this.#pending.get(id) ?? 0) + 1)
    const res = await this.#send(req)
    const left = (this.#pending.get(id) ?? 1) - 1
    if (left) this.#pending.set(id, left)
    else this.#pending.delete(id)
    if (res !== undefined && !left) apply(res)
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
  listsOf(shopId: string, archived = false) {
    return this.lists.filter((l) => l.shop_id === shopId && !!l.archived_at === archived).sort(byPosition)
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
  staplesOf(shopId: string) {
    return this.staples.filter((s) => s.shop_id === shopId).sort(byPosition)
  }
  known(name: string) {
    const n = normalizeName(name)
    return this.catalog.find((c) => normalizeName(c.name) === n)
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
    const body = { id, name: input.name, amount, unit: unit ?? '', step, category_id: categoryId }
    const norm = normalizeName(input.name)
    const dup = this.itemsOf(listId)
      .filter((i) => !i.checked_at && normalizeName(i.name) === norm)
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
      c = { id: ulid(), name, category_name: null, amount, unit, step: null, use_count: 0, last_used_at: null, shop_id: null, hidden: false, shops: [], created_at: ts, updated_at: ts, rev: 0 }
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
    this.#sendItem(
      merged?.item.id ?? id,
      () => api.post<AddedItem>(`/lists/${listId}/items`, body),
      (res) => this.#applyAdded(id, res),
    )
  }

  tick(item: Item, checked: boolean) {
    item.checked_at = checked ? now() : null
    item.checked_by = checked ? (session.me?.id ?? null) : null
    this.#sendItem(item.id, () => api.patch<Item>(`/items/${item.id}`, { checked }), (res) => upsert(this.items, res))
  }

  step(item: Item, up: boolean) {
    const next = stepAmount(item.amount, item.unit, item.step, up)
    if (next === null) {
      this.remove(item)
      return
    }
    item.amount = next
    this.#sendItem(item.id, () => api.post<Stepped>(`/items/${item.id}/step`, { by: up ? 1 : -1 }), (res) => upsert(this.items, res.item))
  }

  editItem(item: Item, patch: { name?: string; amount?: number; unit?: string | null; step?: number | null; note?: string; category_id?: string | null }) {
    Object.assign(item, patch)
    if ('category_id' in patch) {
      const c = this.known(item.name)
      if (c) c.category_name = this.categories.find((x) => x.id === patch.category_id)?.name ?? null
    }
    const body = { ...patch, ...('unit' in patch ? { unit: patch.unit ?? '' } : {}) }
    this.#sendItem(item.id, () => api.patch<Item>(`/items/${item.id}`, body), (res) => upsert(this.items, res))
  }

  remove(item: Item) {
    removeWhere(this.items, (i) => i.id === item.id)
    this.#send(() => api.del(`/items/${item.id}`))
    toast(`Removed ${item.name}`, 'info', { label: 'Undo', run: () => this.restore([item]) })
  }

  async restore(rows: Item[]) {
    for (const r of rows) upsert(this.items, r)
    const res = await this.#send(() => api.post<Item[]>('/items/restore', { ids: rows.map((r) => r.id) }))
    if (res) for (const r of res) upsert(this.items, r)
  }

  async finish(listId: string) {
    const done = this.itemsOf(listId).filter((i) => i.checked_at)
    if (!done.length) return
    removeWhere(this.items, (i) => i.list_id === listId && !!i.checked_at)
    toast(`Finished: ${done.length} item${done.length === 1 ? '' : 's'} cleared`, 'info', { label: 'Undo', run: () => this.restore(done) })
    await this.#send(() => api.post<Finished>(`/lists/${listId}/finish`))
  }

  // ---- staples ---------------------------------------------------------------------------

  addStaples(listId: string, staples: Staple[]) {
    const sent = staples.map((s) => {
      const r = this.#applyAdd(listId, { name: s.name, amount: s.amount, unit: s.unit, categoryId: this.#categoryNamed(listId, s.category_name) })
      return { staple_id: s.id, id: r.id }
    })
    if (staples.length) toast(`Added ${staples.length} staple${staples.length === 1 ? '' : 's'}`)
    this.#send(() => api.post<AddedItem[]>(`/lists/${listId}/staples`, { staples: sent })).then((res) => res?.forEach((r, i) => this.#applyAdded(sent[i].id, r)))
  }

  /** The list's category of that name: undefined (let the catalogue decide) if none given. */
  #categoryNamed(listId: string, name: string | null): string | null | undefined {
    if (!name) return undefined
    const want = normalizeName(name)
    return this.categoriesOf(listId).find((c) => normalizeName(c.name) === want)?.id
  }

  async createStaple(shopId: string, s: { name: string; amount: number; unit: string | null; category_name: string | null }) {
    const id = ulid()
    const ts = now()
    this.staples.push({ id, shop_id: shopId, catalog_item_id: null, ...s, position: keyBetween(lastKey(this.staplesOf(shopId)), null), created_at: ts, updated_at: ts, rev: 0 })
    const res = await this.#send(() => api.post<Staple>(`/shops/${shopId}/staples`, { id, ...s, unit: s.unit ?? '' }))
    if (res) upsert(this.staples, res)
  }

  async patchStaple(s: Staple, patch: { name?: string; amount?: number; unit?: string | null; category_name?: string | null }) {
    Object.assign(s, patch)
    const res = await this.#send(() => api.patch<Staple>(`/staples/${s.id}`, patch))
    if (res) upsert(this.staples, res)
  }

  deleteStaple(s: Staple) {
    removeWhere(this.staples, (x) => x.id === s.id)
    this.#send(() => api.del(`/staples/${s.id}`))
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
    return res.shop.default_list_id ?? undefined
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
    removeWhere(this.staples, (s) => s.shop_id === shop.id)
    removeWhere(this.shops, (s) => s.id === shop.id)
    for (const c of this.catalog) removeWhere(c.shops, (u) => u.shop_id === shop.id)
    await this.#send(() => api.del(`/shops/${shop.id}`))
  }

  async createList(shopId: string, name: string): Promise<string | undefined> {
    const res = await this.#send(() => api.post<ListBundle>('/lists', { id: ulid(), shop_id: shopId, name }))
    if (!res) return undefined
    this.#applyBundle({ lists: [res.list], categories: res.categories })
    return res.list.id
  }

  async patchList(list: ShoppingList, patch: { name?: string; archived?: boolean; is_default?: boolean }) {
    const res = await this.#send(() => api.patch<ShoppingList>(`/lists/${list.id}`, patch))
    if (!res) return
    if (patch.is_default) {
      for (const l of this.lists) if (l.shop_id === res.shop_id) l.is_default = false
      const shop = this.shop(res.shop_id)
      if (shop) shop.default_list_id = res.id
    }
    upsert(this.lists, res)
  }

  async deleteList(list: ShoppingList) {
    removeWhere(this.items, (i) => i.list_id === list.id)
    removeWhere(this.categories, (c) => c.list_id === list.id)
    removeWhere(this.lists, (l) => l.id === list.id)
    await this.#send(() => api.del(`/lists/${list.id}`))
  }

  async createCategory(listId: string, name: string) {
    const id = ulid()
    const ts = now()
    this.categories.push({ id, list_id: listId, name, position: keyBetween(lastKey(this.categoriesOf(listId)), null), created_at: ts, updated_at: ts, rev: 0 })
    const res = await this.#send(() => api.post<Category>(`/lists/${listId}/categories`, { id, name }))
    if (res) upsert(this.categories, res)
  }

  async renameCategory(c: Category, name: string) {
    c.name = name
    const res = await this.#send(() => api.patch<Category>(`/categories/${c.id}`, { name }))
    if (res) upsert(this.categories, res)
  }

  async deleteCategory(c: Category) {
    for (const i of this.items) if (i.category_id === c.id) i.category_id = null
    removeWhere(this.categories, (x) => x.id === c.id)
    await this.#send(() => api.del(`/categories/${c.id}`))
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
    this.#sendItem(item.id, () => api.post<Item>(`/items/${item.id}/move`, body), (res) => upsert(this.items, res))
  }

  /** Move items to the end of another list, keeping a category of the same name. */
  async moveItems(items: Item[], listId: string) {
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
    const res = await this.#send(() => api.post<Item[]>('/items/move', { ids: items.map((i) => i.id), list_id: listId }))
    if (res) for (const r of res) upsert(this.items, r)
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
    return this.#move(this.categories, c, this.categoriesOf(c.list_id), index, `/categories/${c.id}/move`)
  }
  moveList(l: ShoppingList, index: number) {
    return this.#move(this.lists, l, this.listsOf(l.shop_id), index, `/lists/${l.id}/move`)
  }
  moveShop(s: Shop, index: number) {
    return this.#move(this.shops, s, this.sortedShops, index, `/shops/${s.id}/move`)
  }
  moveStaple(s: Staple, index: number) {
    return this.#move(this.staples, s, this.staplesOf(s.shop_id), index, `/staples/${s.id}/move`)
  }

  // ---- catalogue management ------------------------------------------------------------------

  async reloadCatalog() {
    this.catalog = await api.get<CatalogItem[]>('/catalog')
  }

  async patchCatalog(c: CatalogItem, patch: { name?: string; category_name?: string | null; amount?: number; unit?: string | null; step?: number | null; hidden?: boolean }) {
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
