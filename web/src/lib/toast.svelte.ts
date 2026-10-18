export type Toast = { id: number; text: string; kind: 'info' | 'error'; action?: { label: string; run: () => void } }

let next = 1
export const toasts = $state<Toast[]>([])

export function toast(text: string, kind: Toast['kind'] = 'info', action?: Toast['action']) {
  const t = { id: next++, text, kind, action }
  toasts.push(t)
  // Long messages stay long enough to read.
  setTimeout(() => dismiss(t.id), Math.max(action ? 6000 : 3500, text.length * 60))
}

export function dismiss(id: number) {
  const i = toasts.findIndex((t) => t.id === id)
  if (i >= 0) toasts.splice(i, 1)
}
