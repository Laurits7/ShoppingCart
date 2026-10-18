export type Toast = { id: number; text: string; kind: 'info' | 'error'; action?: { label: string; run: () => void } }

let next = 1
export const toasts = $state<Toast[]>([])
const timers = new Map<number, { left: number; started: number; timer?: ReturnType<typeof setTimeout> }>()

export function toast(text: string, kind: Toast['kind'] = 'info', action?: Toast['action']) {
  const t = { id: next++, text, kind, action }
  toasts.push(t)
  // Long messages stay long enough to read; ones with an action (Undo) at least 10 s.
  timers.set(t.id, { left: Math.max(action ? 10000 : 4000, text.length * 70), started: 0 })
  resume(t.id)
}

/** Hovered or focused: keep it until the pointer or focus leaves (WCAG 2.2.1). */
export function pause(id: number) {
  const t = timers.get(id)
  if (!t?.timer) return
  clearTimeout(t.timer)
  t.timer = undefined
  t.left -= Date.now() - t.started
}

export function resume(id: number) {
  const t = timers.get(id)
  if (!t || t.timer) return
  t.started = Date.now()
  t.timer = setTimeout(() => dismiss(id), Math.max(t.left, 1500))
}

export function dismiss(id: number) {
  const t = timers.get(id)
  if (t?.timer) clearTimeout(t.timer)
  timers.delete(id)
  const i = toasts.findIndex((t) => t.id === id)
  if (i >= 0) toasts.splice(i, 1)
}
