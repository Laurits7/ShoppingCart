// The outbox (SPEC §6.7): changes are applied to the local store at once and queued here,
// persisted in IndexedDB (or memory), and sent in order. A connection problem pauses the
// queue and it retries with back-off, keeping the order; creates carry client-chosen ids,
// so a retry after a lost answer is harmless. A change the server refuses (4xx) is dropped
// with a message, and the store resyncs.

import { ApiError } from './api/client'
import { tx } from './idb'

export type Entry = {
  seq?: number
  /** Whose change this is: another user's queue is never sent. */
  user: string
  method: 'POST' | 'PATCH' | 'PUT' | 'DELETE'
  path: string
  body?: unknown
  /** The id of what it changes, so a sync doesn't overwrite it while it's waiting. */
  key?: string
}

/** Where entries are kept. */
export interface Store {
  all(): Promise<Entry[]>
  add(e: Entry): Promise<number>
  remove(seq: number): Promise<void>
  clear(): Promise<void>
}

export class MemoryStore implements Store {
  #rows: Entry[] = []
  #seq = 0
  async all() {
    return [...this.#rows]
  }
  async add(e: Entry) {
    const seq = ++this.#seq
    this.#rows.push({ ...e, seq })
    return seq
  }
  async remove(seq: number) {
    this.#rows = this.#rows.filter((r) => r.seq !== seq)
  }
  async clear() {
    this.#rows = []
  }
}

/** IndexedDB, so changes made in a dead zone survive closing the app. */
export class IdbStore implements Store {
  all() {
    return tx('outbox', 'readonly', (s) => s.getAll() as IDBRequest<Entry[]>)
  }
  async add(e: Entry) {
    return (await tx('outbox', 'readwrite', (s) => s.add(e))) as number
  }
  async remove(seq: number) {
    await tx('outbox', 'readwrite', (s) => s.delete(seq))
  }
  async clear() {
    await tx('outbox', 'readwrite', (s) => s.clear())
  }
}

export type Send = (e: Entry) => Promise<unknown>
/** What became of a refused change. */
export type Refused = { entry: Entry; error: ApiError }

const BACKOFF = [1000, 3000, 8000, 20000, 30000]

/** Whether a failure means "try again later" rather than "the server said no". */
export const retriable = (e: unknown) => !(e instanceof ApiError) || e.status === 0 || e.status === 401 || e.status === 429 || e.status >= 500

export class Outbox {
  /** Changes not yet confirmed by the server. */
  pending = $state(0)
  /** False while the server can't be reached. */
  online = $state(true)
  #store: Store
  #send: Send
  #queue: Entry[] = []
  #waiters = new Map<number, (res: unknown) => void>()
  #running = false
  #tries = 0
  #timer: ReturnType<typeof setTimeout> | undefined
  /** Called when a change was refused (dropped) and when the queue has emptied. */
  onrefused: (r: Refused) => void = () => {}
  ondrained: () => void = () => {}

  constructor(store: Store, send: Send) {
    this.#store = store
    this.#send = send
  }

  /** Load what's waiting from storage (another user's changes are discarded) and send it. */
  async start(user: string) {
    const rows = await this.#store.all().catch(() => [] as Entry[])
    for (const r of rows) if (r.user !== user && r.seq !== undefined) await this.#store.remove(r.seq)
    this.#queue = rows.filter((r) => r.user === user).sort((a, b) => a.seq! - b.seq!)
    this.pending = this.#queue.length
    this.flush()
  }

  /** Queue a change; resolves with the server's answer once it is sent. */
  async add(e: Entry): Promise<unknown> {
    let seq: number
    try {
      seq = await this.#store.add(e)
    } catch {
      seq = -Date.now() - Math.random()
    }
    const entry = { ...e, seq }
    this.#queue.push(entry)
    this.pending = this.#queue.length
    const done = new Promise((ok) => this.#waiters.set(seq, ok))
    this.flush()
    return done
  }

  /** Ids with a change still waiting (a sync must not overwrite them). */
  keys(): Set<string> {
    return new Set(this.#queue.map((e) => e.key).filter((k): k is string => !!k))
  }

  /** Forget everything (signed out). */
  async clear() {
    clearTimeout(this.#timer)
    this.#queue = []
    this.#waiters.clear()
    this.pending = 0
    await this.#store.clear().catch(() => {})
  }

  /** Send now (e.g. the connection is back). */
  retry() {
    clearTimeout(this.#timer)
    this.#tries = 0
    this.flush()
  }

  async flush() {
    if (this.#running) return
    this.#running = true
    try {
      while (this.#queue.length) {
        const e = this.#queue[0]
        let res: unknown
        try {
          res = await this.#send(e)
        } catch (err) {
          if (retriable(err)) {
            if (err instanceof ApiError && err.status === 0) this.online = false
            const wait = BACKOFF[Math.min(this.#tries++, BACKOFF.length - 1)]
            this.#timer = setTimeout(() => this.flush(), wait)
            return
          }
          // Refused: drop it and go on with the rest.
          this.onrefused({ entry: e, error: err as ApiError })
          res = undefined
        }
        this.online = true
        this.#tries = 0
        this.#queue.shift()
        this.pending = this.#queue.length
        if (e.seq !== undefined && e.seq >= 0) await this.#store.remove(e.seq).catch(() => {})
        this.#waiters.get(e.seq!)?.(res)
        this.#waiters.delete(e.seq!)
      }
      this.ondrained()
    } finally {
      this.#running = false
    }
  }
}
