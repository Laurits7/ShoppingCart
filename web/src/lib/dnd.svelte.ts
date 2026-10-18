// One drag-and-drop layer for every view (SPEC §6.5), built on pointer events so mouse,
// touch and pen behave the same (adapted from Streamline):
// - mouse: press on a row and move a few pixels;
// - touch: long-press a row (so normal scrolling never starts a drag), or drag the grip
//   handle straight away;
// - keyboard: focus a row's grip handle, Space picks it up, the arrow keys move it, Space
//   drops it and Escape puts it back; each step is announced to screen readers.
// Drags show a floating copy of the row. Ordered lists register with `dropList`, which
// reports the index a drop lands on.

import { announce } from './announce.svelte'
import { edgeDelta, insertIndex, stepIndex } from './dndmath'

export type DragItem =
  | { kind: 'item'; id: string; listId: string }
  | { kind: 'category'; id: string; listId: string }
  | { kind: 'list'; id: string; shopId: string }
  | { kind: 'shop'; id: string }
  | { kind: 'staple'; id: string; shopId: string }

export type DropAt = { x: number; y: number; el: HTMLElement }

type DropTarget = {
  accepts: (item: DragItem) => boolean
  over?: (item: DragItem, at: DropAt) => void
  leave?: () => void
  drop: (item: DragItem, at: DropAt) => void
}

/** The item being dragged, for views that react (e.g. show empty categories as targets). */
export const dnd = $state({ item: null as DragItem | null })

const targets = new WeakMap<HTMLElement, DropTarget>()

function droppable(node: HTMLElement, target: DropTarget) {
  node.setAttribute('data-drop', '')
  targets.set(node, target)
  return {
    update: (t: DropTarget) => targets.set(node, t),
    destroy: () => targets.delete(node),
  }
}

/** Innermost drop target under the point that accepts the item. */
function targetAt(x: number, y: number, item: DragItem): [HTMLElement, DropTarget] | null {
  let el = document.elementFromPoint(x, y) as HTMLElement | null
  while (el) {
    const d = el.closest<HTMLElement>('[data-drop]')
    if (!d) return null
    const t = targets.get(d)
    if (t?.accepts(item)) return [d, t]
    el = d.parentElement
  }
  return null
}

let active = false

export type DraggableOptions = {
  item: () => DragItem | null
  disabled?: boolean
  /** Only the grip ([data-handle]) starts a drag: for big things like list cards. */
  handleOnly?: boolean
}

export function draggable(node: HTMLElement, opts: DraggableOptions) {
  let o = opts
  node.setAttribute('data-draggable', '')

  function onDown(e: PointerEvent) {
    if (o.disabled || active || e.button !== 0 || !e.isPrimary) return
    const t = e.target as HTMLElement
    if (t.closest('[data-nodrag], input, textarea, select')) return
    if (t.closest('[data-draggable]') !== node) return // nested draggables: innermost wins
    const fromHandle = !!t.closest('[data-handle]')
    if (o.handleOnly && !fromHandle) return
    const longPress = e.pointerType !== 'mouse' && !fromHandle
    const sx = e.clientX
    const sy = e.clientY
    let timer: ReturnType<typeof setTimeout> | undefined

    const cancel = () => {
      clearTimeout(timer)
      window.removeEventListener('pointermove', onMove)
      window.removeEventListener('pointerup', cancel)
      window.removeEventListener('pointercancel', cancel)
      node.removeEventListener('contextmenu', noMenu)
    }
    const begin = (at: { clientX: number; clientY: number }) => {
      cancel()
      const item = o.disabled ? null : o.item()
      if (item) start(node, item, e, at)
    }
    const onMove = (ev: PointerEvent) => {
      const dist = Math.hypot(ev.clientX - sx, ev.clientY - sy)
      if (longPress) {
        if (dist > 8) cancel() // the finger moved first: it's a scroll, not a drag
      } else if (dist > 5) begin(ev)
    }
    const noMenu = (ev: Event) => ev.preventDefault()

    window.addEventListener('pointermove', onMove)
    window.addEventListener('pointerup', cancel)
    window.addEventListener('pointercancel', cancel)
    if (longPress) {
      node.addEventListener('contextmenu', noMenu)
      timer = setTimeout(() => {
        navigator.vibrate?.(12)
        begin(e)
      }, 350)
    }
  }

  // Browsers decide at touchstart whether a gesture may scroll, so the listener that stops
  // scrolling once a long-press drag has begun must exist before the touch.
  const onTouchMove = (e: TouchEvent) => {
    if (active) e.preventDefault()
  }
  node.addEventListener('pointerdown', onDown)
  node.addEventListener('touchmove', onTouchMove, { passive: false })
  return {
    update(n: DraggableOptions) {
      o = n
    },
    destroy() {
      node.removeEventListener('pointerdown', onDown)
      node.removeEventListener('touchmove', onTouchMove)
    },
  }
}

function start(node: HTMLElement, item: DragItem, down: PointerEvent, at: { clientX: number; clientY: number }) {
  active = true
  dnd.item = item
  const rect = node.getBoundingClientRect()
  const ghost = node.cloneNode(true) as HTMLElement
  ghost.classList.add('drag-ghost')
  ghost.removeAttribute('data-id')
  ghost.removeAttribute('data-drop')
  Object.assign(ghost.style, {
    position: 'fixed',
    left: `${rect.left}px`,
    top: `${rect.top}px`,
    width: `${rect.width}px`,
    height: `${rect.height}px`,
    margin: '0',
    pointerEvents: 'none',
    zIndex: '1000',
  })
  // Inside a modal sheet the ghost must live in the dialog (the top layer) to be seen.
  const dialog = node.closest('dialog')
  ;(dialog ?? document.body).appendChild(ghost)
  node.classList.add('drag-source')
  document.body.classList.add('dragging-active')
  getSelection()?.removeAllRanges()

  const offX = down.clientX - rect.left
  const offY = down.clientY - rect.top
  let x = at.clientX
  let y = at.clientY
  let current: [HTMLElement, DropTarget] | null = null
  let raf = 0

  const place = () => {
    ghost.style.transform = `translate(${x - offX - rect.left}px, ${y - offY - rect.top}px) rotate(1deg)`
  }
  const hit = () => {
    const t = targetAt(x, y, item)
    if (t?.[0] !== current?.[0]) {
      current?.[1].leave?.()
      current?.[0].classList.remove('drop-hover')
      current = t
      current?.[0].classList.add('drop-hover')
    }
    current?.[1].over?.(item, { x, y, el: current[0] })
  }
  const onMove = (e: PointerEvent) => {
    x = e.clientX
    y = e.clientY
    place()
    hit()
  }
  // Auto-scroll the window, or a scroll box marked [data-autoscroll] (like a sheet), near
  // its edges.
  const loop = () => {
    let scrolled = false
    const box = (document.elementFromPoint(x, y) as HTMLElement | null)?.closest<HTMLElement>('[data-autoscroll]')
    if (box) {
      const r = box.getBoundingClientRect()
      const d = edgeDelta(y, r.top, r.bottom, 40)
      if (d) {
        box.scrollTop += d
        scrolled = true
      }
    } else {
      const d = edgeDelta(y, 0, window.innerHeight, y < window.innerHeight / 2 ? 56 : 90)
      if (d) {
        window.scrollBy(0, d)
        scrolled = true
      }
    }
    if (scrolled) hit()
    raf = requestAnimationFrame(loop)
  }
  const onKey = (e: KeyboardEvent) => e.key === 'Escape' && end(false)
  const onUp = () => end(true)
  const onCancel = () => end(false)

  function end(commit: boolean) {
    cancelAnimationFrame(raf)
    window.removeEventListener('pointermove', onMove)
    window.removeEventListener('pointerup', onUp)
    window.removeEventListener('pointercancel', onCancel)
    window.removeEventListener('keydown', onKey)
    ghost.remove()
    node.classList.remove('drag-source')
    document.body.classList.remove('dragging-active')
    const target = current
    current?.[1].leave?.()
    current?.[0].classList.remove('drop-hover')
    current = null
    active = false
    dnd.item = null
    // The click that follows a drag must not tick the item.
    const swallow = (e: Event) => {
      e.stopPropagation()
      e.preventDefault()
    }
    window.addEventListener('click', swallow, { capture: true })
    setTimeout(() => window.removeEventListener('click', swallow, { capture: true }), 60)
    if (commit && target) target[1].drop(item, { x, y, el: target[0] })
  }

  window.addEventListener('pointermove', onMove)
  window.addEventListener('pointerup', onUp)
  window.addEventListener('pointercancel', onCancel)
  window.addEventListener('keydown', onKey)
  place()
  hit()
  raf = requestAnimationFrame(loop)
}

export type DropListOptions = {
  accepts: (item: DragItem) => boolean
  /** `index` counts the list's rows, not including the dragged one. */
  drop: (item: DragItem, index: number) => void
  /** Keyboard: move row `id` to `index` (applied at once). */
  keyMove?: (id: string, index: number) => void
}

/** An ordered list whose rows are children with [data-id] (and [data-label] for screen
 * readers); a drop reports where it lands. */
export function dropList(node: HTMLElement, opts: DropListOptions) {
  let o = opts
  let marked: HTMLElement | null = null
  const rows = () => Array.from(node.querySelectorAll<HTMLElement>(':scope > [data-id]')).filter((c) => !c.classList.contains('drag-source'))
  const clear = () => {
    marked?.classList.remove('drop-before', 'drop-after')
    marked = null
  }
  const indexAt = (y: number): [number, HTMLElement[]] => {
    const rs = rows()
    const mids = rs.map((r) => {
      const b = r.getBoundingClientRect()
      return b.top + b.height / 2
    })
    return [insertIndex(mids, y), rs]
  }
  const d = droppable(node, {
    accepts: (it) => o.accepts(it),
    over: (_, at) => {
      const [i, rs] = indexAt(at.y)
      clear()
      if (rs.length) {
        marked = rs[Math.min(i, rs.length - 1)]
        marked.classList.add(i < rs.length ? 'drop-before' : 'drop-after')
      }
    },
    leave: clear,
    drop: (it, at) => {
      const [i] = indexAt(at.y)
      clear()
      o.drop(it, i)
    },
  })

  // Keyboard: Space picks up / drops the focused handle's row, arrows move it, Escape
  // puts it back where it was.
  let lifted: { id: string; from: number } | null = null
  // Reordering moves the row's element, which can blur its handle: not a reason to drop.
  let moving = false
  const label = (row: HTMLElement) => row.dataset.label ?? 'item'
  const focusHandle = (id: string) =>
    requestAnimationFrame(() => {
      const row = node.querySelector<HTMLElement>(`:scope > [data-id="${id}"]`)
      if (lifted?.id === id) row?.classList.add('kbd-lifted')
      row?.querySelector<HTMLElement>('[data-handle]')?.focus()
      moving = false
    })
  const onKey = (e: KeyboardEvent) => {
    if (!o.keyMove) return
    const handle = (e.target as HTMLElement).closest<HTMLElement>('[data-handle]')
    const row = handle?.closest<HTMLElement>('[data-id]')
    if (!row || row.parentElement !== node) return
    const id = row.dataset.id!
    const rs = rows()
    const at = rs.indexOf(row)
    if (e.key === ' ' || e.key === 'Enter') {
      e.preventDefault()
      if (lifted?.id === id) {
        lifted = null
        row.classList.remove('kbd-lifted')
        announce(`${label(row)} dropped at position ${at + 1} of ${rs.length}.`)
      } else {
        lifted = { id, from: at }
        row.classList.add('kbd-lifted')
        announce(`${label(row)} picked up, position ${at + 1} of ${rs.length}. Use the up and down arrows to move it, Space to drop, Escape to cancel.`)
      }
    } else if ((e.key === 'ArrowUp' || e.key === 'ArrowDown') && lifted?.id === id) {
      e.preventDefault()
      const to = stepIndex(at, rs.length, e.key === 'ArrowUp')
      if (to === null) return
      moving = true
      o.keyMove(id, to)
      announce(`Position ${to + 1} of ${rs.length}.`)
      focusHandle(id)
    } else if (e.key === 'Escape' && lifted?.id === id) {
      e.preventDefault()
      const from = lifted.from
      lifted = null
      row.classList.remove('kbd-lifted')
      if (from !== at) {
        moving = true
        o.keyMove(id, from)
      }
      announce(`Move cancelled; ${label(row)} is back at position ${from + 1}.`)
      focusHandle(id)
    }
  }
  const onBlur = (e: FocusEvent) => {
    // Leaving the handle drops a keyboard-lifted row where it is.
    const row = (e.target as HTMLElement).closest<HTMLElement>('[data-id]')
    if (!moving && lifted && row?.dataset.id === lifted.id && !row.contains(e.relatedTarget as Node)) {
      row.classList.remove('kbd-lifted')
      lifted = null
    }
  }
  node.addEventListener('keydown', onKey)
  node.addEventListener('focusout', onBlur)
  return {
    update(n: DropListOptions) {
      o = n
    },
    destroy() {
      d.destroy()
      node.removeEventListener('keydown', onKey)
      node.removeEventListener('focusout', onBlur)
    },
  }
}
