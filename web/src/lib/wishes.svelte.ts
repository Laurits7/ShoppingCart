// Client-side copy of the Wishlist tab: persons, wishlists, sections, wish items and claims.
// It arrives with the same /sync as the Shopping tab (lib/shopping.svelte.ts calls `apply`),
// and item, section and claim changes go through the same outbox. What the server sends is
// already filtered: no gift list about me, no claims on my own wishlists.

import { api, message } from './api/client'
import type { Claim } from './api/types/Claim'
import type { Person } from './api/types/Person'
import type { ShoppingData } from './api/types/ShoppingData'
import type { WishItem } from './api/types/WishItem'
import type { WishSection } from './api/types/WishSection'
import type { Wishlist } from './api/types/Wishlist'
import { neighbours } from './dndmath'
import { keyBetween } from './order'
import { session } from './session.svelte'
import { byPosition, shopping } from './shopping.svelte'
import { toast } from './toast.svelte'
import { ulid } from './ulid'

const now = () => new Date().toISOString()

function upsert<T extends { id: string }>(arr: T[], row: T) {
  const old = arr.find((x) => x.id === row.id)
  if (old) Object.assign(old, row)
  else arr.push(row)
}

function removeWhere<T>(arr: T[], pred: (x: T) => boolean) {
  for (let i = arr.length - 1; i >= 0; i--) if (pred(arr[i])) arr.splice(i, 1)
}

const lastKey = (rows: { position: string }[]) => rows.reduce<string>((m, r) => (r.position > m ? r.position : m), '')

export type WishInput = { title: string; url: string | null; price: number | null; currency: string | null; note: string; priority: number; section_id: string | null }

class WishStore {
  persons = $state<Person[]>([])
  wishlists = $state<Wishlist[]>([])
  sections = $state<WishSection[]>([])
  items = $state<WishItem[]>([])
  claims = $state<Claim[]>([])

  reset() {
    this.persons = []
    this.wishlists = []
    this.sections = []
    this.items = []
    this.claims = []
  }

  /** Take what /sync sent: everything (`full`), or what changed since the last sync. */
  apply(d: ShoppingData, full: boolean, waiting: Set<string>) {
    if (full) {
      const keep = <T extends { id: string }>(fresh: T[], old: T[]) => [...fresh.filter((r) => !waiting.has(r.id)), ...old.filter((r) => waiting.has(r.id))]
      this.persons = d.persons
      this.wishlists = d.wishlists
      this.sections = keep(d.wish_sections, this.sections)
      this.items = keep(d.wish_items, this.items)
      this.claims = keep(d.claims, this.claims)
      return
    }
    const merge = <T extends { id: string }>(arr: T[], rows: T[]) => {
      for (const r of rows) if (!waiting.has(r.id)) upsert(arr, r)
    }
    merge(this.persons, d.persons)
    merge(this.wishlists, d.wishlists)
    merge(this.sections, d.wish_sections)
    merge(this.items, d.wish_items)
    merge(this.claims, d.claims)
    const gone = (kind: string) => new Set(d.deleted.filter((x) => x.kind === kind).map((x) => x.id))
    const drop = <T extends { id: string }>(arr: T[], kind: string) => {
      const g = gone(kind)
      if (g.size) removeWhere(arr, (r) => g.has(r.id) && !waiting.has(r.id))
    }
    drop(this.persons, 'person')
    drop(this.wishlists, 'wishlist')
    drop(this.sections, 'wish_section')
    drop(this.items, 'wish_item')
    drop(this.claims, 'claim')
    // Wishlists no longer visible (unshared, or about me now) go with everything on them.
    const visible = new Set(d.visible_wishlists)
    const lost = new Set(this.wishlists.filter((w) => !visible.has(w.id)).map((w) => w.id))
    if (lost.size) {
      const lostItems = new Set(this.items.filter((i) => lost.has(i.wishlist_id)).map((i) => i.id))
      removeWhere(this.claims, (c) => lostItems.has(c.wish_item_id))
      removeWhere(this.items, (i) => lost.has(i.wishlist_id))
      removeWhere(this.sections, (s) => lost.has(s.wishlist_id))
      removeWhere(this.wishlists, (w) => lost.has(w.id))
    }
  }

  /** Whether /sync shows a wishlist we don't have (its older items aren't in the delta). */
  hasNew(d: ShoppingData) {
    const known = new Set(this.wishlists.map((w) => w.id))
    return d.visible_wishlists.some((id) => !known.has(id))
  }

  // ---- queries -------------------------------------------------------------------------------

  wishlist(id: string | null | undefined) {
    return this.wishlists.find((w) => w.id === id)
  }
  get mine() {
    return this.wishlists.filter((w) => w.kind === 'own' && w.owner_id === session.me?.id).sort(byPosition)
  }
  get others() {
    return this.wishlists.filter((w) => w.kind === 'own' && w.owner_id !== session.me?.id).sort((a, b) => a.owner_name.localeCompare(b.owner_name))
  }
  /** Gift lists by recipient name. */
  get gifts(): [string, Wishlist[]][] {
    const by = new Map<string, Wishlist[]>()
    for (const w of this.wishlists.filter((w) => w.kind === 'gift')) {
      const k = w.recipient_name ?? '?'
      by.set(k, [...(by.get(k) ?? []), w])
    }
    for (const p of this.persons) if (!by.has(p.name)) by.set(p.name, [])
    return [...by.entries()].sort((a, b) => a[0].localeCompare(b[0]))
  }
  sectionsOf(wid: string) {
    return this.sections.filter((s) => s.wishlist_id === wid).sort(byPosition)
  }
  /** Items still wished for (not received), in order. */
  openOf(wid: string) {
    return this.items.filter((i) => i.wishlist_id === wid && !i.received_at).sort(byPosition)
  }
  receivedOf(wid: string) {
    return this.items.filter((i) => i.wishlist_id === wid && i.received_at).sort((a, b) => (b.received_at! > a.received_at! ? 1 : -1))
  }
  claimOf(itemId: string) {
    return this.claims.find((c) => c.wish_item_id === itemId)
  }

  // ---- changes -----------------------------------------------------------------------------

  async #direct<T>(req: () => Promise<T>): Promise<T | undefined> {
    try {
      return await req()
    } catch (e) {
      toast(message(e), 'error')
      return undefined
    }
  }

  async createPerson(name: string, userId: string | null) {
    const p = await this.#direct(() => api.post<Person>('/persons', { id: ulid(), name, user_id: userId }))
    if (p) upsert(this.persons, p)
    return p
  }
  async patchPerson(p: Person, patch: { name?: string; user_id?: string | null }) {
    const r = await this.#direct(() => api.patch<Person>(`/persons/${p.id}`, patch))
    if (r) {
      upsert(this.persons, r)
      for (const w of this.wishlists) if (w.recipient_person_id === r.id) w.recipient_name = r.name
    }
  }
  async deletePerson(p: Person) {
    try {
      await api.del(`/persons/${p.id}`)
      removeWhere(this.persons, (x) => x.id === p.id)
    } catch (e) {
      toast(message(e), 'error')
    }
  }

  async createWishlist(kind: 'own' | 'gift', title: string, recipient: string | null) {
    const w = await this.#direct(() => api.post<Wishlist>('/wishlists', { id: ulid(), kind, title, recipient_person_id: recipient }))
    if (w) upsert(this.wishlists, w)
    return w
  }
  async renameWishlist(w: Wishlist, title: string) {
    w.title = title
    const r = await this.#direct(() => api.patch<Wishlist>(`/wishlists/${w.id}`, { title }))
    if (r) upsert(this.wishlists, r)
  }
  async deleteWishlist(w: Wishlist) {
    removeWhere(this.items, (i) => i.wishlist_id === w.id)
    removeWhere(this.sections, (s) => s.wishlist_id === w.id)
    removeWhere(this.wishlists, (x) => x.id === w.id)
    await this.#direct(() => api.del(`/wishlists/${w.id}`))
  }
  moveWishlist(w: Wishlist, index: number) {
    const siblings = this.mine.filter((x) => x.id !== w.id)
    w.position = keyBetween(siblings[index - 1]?.position ?? '', siblings[index]?.position ?? null)
    this.#direct(() => api.post<Wishlist>(`/wishlists/${w.id}/move`, neighbours(siblings.map((s) => s.id), index))).then((r) => r && upsert(this.wishlists, r))
  }

  createSection(wid: string, name: string) {
    const id = ulid()
    const ts = now()
    this.sections.push({ id, wishlist_id: wid, name, position: keyBetween(lastKey(this.sectionsOf(wid)), null), created_at: ts, updated_at: ts, rev: 0 })
    shopping.queue<WishSection>({ method: 'POST', path: `/wishlists/${wid}/sections`, body: { id, name }, key: id }, (r) => upsert(this.sections, r))
  }
  renameSection(s: WishSection, name: string) {
    s.name = name
    shopping.queue<WishSection>({ method: 'PATCH', path: `/wish-sections/${s.id}`, body: { name }, key: s.id }, (r) => upsert(this.sections, r))
  }
  deleteSection(s: WishSection) {
    for (const i of this.items) if (i.section_id === s.id) i.section_id = null
    removeWhere(this.sections, (x) => x.id === s.id)
    shopping.queue({ method: 'DELETE', path: `/wish-sections/${s.id}`, key: s.id })
  }
  moveSection(s: WishSection, index: number) {
    const siblings = this.sectionsOf(s.wishlist_id).filter((x) => x.id !== s.id)
    s.position = keyBetween(siblings[index - 1]?.position ?? '', siblings[index]?.position ?? null)
    shopping.queue<WishSection>({ method: 'POST', path: `/wish-sections/${s.id}/move`, body: neighbours(siblings.map((x) => x.id), index), key: s.id }, (r) => upsert(this.sections, r))
  }

  addItem(wid: string, w: WishInput) {
    const id = ulid()
    const ts = now()
    const currency = w.price !== null ? (w.currency ?? this.currency) : null
    this.items.push({ id, wishlist_id: wid, ...w, currency, position: keyBetween(lastKey(this.items.filter((i) => i.wishlist_id === wid)), null), received_at: null, created_at: ts, updated_at: ts, rev: 0 })
    shopping.queue<WishItem>({ method: 'POST', path: `/wishlists/${wid}/items`, body: { id, ...w, currency }, key: id }, (r) => upsert(this.items, r))
  }
  editItem(i: WishItem, patch: Partial<WishInput> & { received?: boolean }) {
    const { received, ...fields } = patch
    Object.assign(i, fields)
    if (received !== undefined) i.received_at = received ? (i.received_at ?? now()) : null
    shopping.queue<WishItem>({ method: 'PATCH', path: `/wish-items/${i.id}`, body: patch, key: i.id }, (r) => upsert(this.items, r))
  }
  removeItem(i: WishItem) {
    removeWhere(this.items, (x) => x.id === i.id)
    shopping.queue({ method: 'DELETE', path: `/wish-items/${i.id}`, key: i.id })
    toast(`Deleted ${i.title}`, 'info', {
      label: 'Undo',
      run: () => {
        upsert(this.items, i)
        shopping.queue<WishItem[]>({ method: 'POST', path: '/wish-items/restore', body: { ids: [i.id] } }, (r) => r.forEach((x) => upsert(this.items, x)))
      },
    })
  }
  /** Move to `index` among the open items of a section (null = no section). */
  moveItem(i: WishItem, sectionId: string | null, index: number) {
    const siblings = this.openOf(i.wishlist_id).filter((x) => x.id !== i.id && (x.section_id ?? null) === sectionId)
    i.position = keyBetween(siblings[index - 1]?.position ?? '', siblings[index]?.position ?? null)
    i.section_id = sectionId
    const body = { ...neighbours(siblings.map((x) => x.id), index), section_id: sectionId }
    shopping.queue<WishItem>({ method: 'POST', path: `/wish-items/${i.id}/move`, body, key: i.id }, (r) => upsert(this.items, r))
  }
  /** Copy a wish to another wishlist (e.g. a gift idea into my own); claims don't come along. */
  copyItem(i: WishItem, to: Wishlist) {
    this.addItem(to.id, { title: i.title, url: i.url, price: i.price, currency: i.currency, note: i.note, priority: Number(i.priority), section_id: null })
    toast(`Copied to ${to.title}`)
  }

  claim(i: WishItem) {
    const id = ulid()
    const ts = now()
    this.claims.push({ id, wish_item_id: i.id, user_id: session.me?.id ?? '', user_name: session.me?.display_name ?? '', status: 'planned', note: '', created_at: ts, updated_at: ts, rev: 0 })
    shopping.queue<Claim>({ method: 'POST', path: `/wish-items/${i.id}/claim`, body: { id }, key: id }, (r) => {
      removeWhere(this.claims, (c) => c.id === id && r.id !== id)
      upsert(this.claims, r)
    })
  }
  setBought(c: Claim, bought: boolean) {
    c.status = bought ? 'bought' : 'planned'
    shopping.queue<Claim>({ method: 'PATCH', path: `/claims/${c.id}`, body: { status: c.status }, key: c.id }, (r) => upsert(this.claims, r))
  }
  release(c: Claim) {
    removeWhere(this.claims, (x) => x.id === c.id)
    shopping.queue({ method: 'DELETE', path: `/claims/${c.id}`, key: c.id })
  }

  /** The currency for new prices (Settings), EUR by default. */
  get currency(): string {
    const c = session.me?.prefs.currency
    return typeof c === 'string' && /^[A-Z]{3}$/.test(c) ? c : 'EUR'
  }
}

export const wishes = new WishStore()

/** "€20", "$5.50": a price for display. */
export function formatPrice(price: number | null, currency: string | null): string {
  if (price === null) return ''
  try {
    return new Intl.NumberFormat(session.me?.locale || undefined, { style: 'currency', currency: currency ?? 'EUR', maximumFractionDigits: price % 1 ? 2 : 0 }).format(price)
  } catch {
    return `${price} ${currency ?? ''}`.trim()
  }
}

/** The site a link goes to, shown instead of a fetched preview ("example.com"). */
export function host(url: string | null): string | null {
  if (!url) return null
  try {
    return new URL(url).hostname.replace(/^www\./, '')
  } catch {
    return null
  }
}
