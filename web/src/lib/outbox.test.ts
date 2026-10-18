import { describe, expect, it, vi } from 'vitest'
import { ApiError } from './api/client'
import { MemoryStore, Outbox, type Entry } from './outbox.svelte'

const e = (path: string, key?: string): Entry => ({ user: 'u1', method: 'PATCH', path, key })

describe('outbox', () => {
  it('sends in order and resolves each change with its answer', async () => {
    const sent: string[] = []
    const box = new Outbox(new MemoryStore(), async (x) => (sent.push(x.path), x.path.toUpperCase()))
    await box.start('u1')
    const [a, b] = await Promise.all([box.add(e('/a')), box.add(e('/b'))])
    expect(sent).toEqual(['/a', '/b'])
    expect([a, b]).toEqual(['/A', '/B'])
    expect(box.pending).toBe(0)
  })

  it('keeps the order through a dead zone and retries', async () => {
    vi.useFakeTimers()
    let down = true
    const sent: string[] = []
    const box = new Outbox(new MemoryStore(), async (x) => {
      if (down) throw new ApiError(0, 'Could not reach the server')
      sent.push(x.path)
    })
    await box.start('u1')
    box.add(e('/a', 'i1'))
    box.add(e('/b', 'i2'))
    await vi.advanceTimersByTimeAsync(10)
    expect(box.online).toBe(false)
    expect(box.pending).toBe(2)
    expect([...box.keys()]).toEqual(['i1', 'i2'])
    down = false
    await vi.advanceTimersByTimeAsync(5000)
    expect(sent).toEqual(['/a', '/b'])
    expect(box.online).toBe(true)
    expect(box.pending).toBe(0)
    vi.useRealTimers()
  })

  it('drops what the server refuses and goes on', async () => {
    const refused: string[] = []
    const sent: string[] = []
    const box = new Outbox(new MemoryStore(), async (x) => {
      if (x.path === '/bad') throw new ApiError(400, 'no')
      sent.push(x.path)
    })
    box.onrefused = (r) => refused.push(r.entry.path)
    await box.start('u1')
    await Promise.all([box.add(e('/bad')), box.add(e('/ok'))])
    expect(refused).toEqual(['/bad'])
    expect(sent).toEqual(['/ok'])
  })

  it('never sends another user’s waiting changes', async () => {
    const store = new MemoryStore()
    await store.add({ ...e('/theirs'), user: 'u2' })
    await store.add(e('/mine'))
    const sent: string[] = []
    const box = new Outbox(store, async (x) => void sent.push(x.path))
    await box.start('u1')
    await vi.waitFor(() => expect(sent).toEqual(['/mine']))
    expect(await store.all()).toEqual([])
  })
})
