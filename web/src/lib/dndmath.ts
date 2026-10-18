// The arithmetic behind drag and drop, kept pure so it can be tested.

/** Where a drop at height `y` goes among rows whose vertical midpoints are `mids` (in order). */
export function insertIndex(mids: number[], y: number): number {
  let i = 0
  for (const m of mids) if (y > m) i++
  return i
}

/** How far to scroll when the pointer at `pos` is within `margin` of either end of
 * [start, end]: negative near the start, positive near the end, 0 elsewhere. Faster the
 * closer it gets. */
export function edgeDelta(pos: number, start: number, end: number, margin: number): number {
  if (pos < start + margin) return (pos - (start + margin)) / 3
  if (pos > end - margin) return (pos - (end - margin)) / 3
  return 0
}

/** The index a keyboard move lands on, or null at either end. */
export function stepIndex(from: number, count: number, up: boolean): number | null {
  const to = up ? from - 1 : from + 1
  return to < 0 || to >= count ? null : to
}

/** The neighbours to name when moving a row to `index` of `ids` (which leaves the moved row
 * out): the server places it after the one before, or before the one after. */
export function neighbours(ids: string[], index: number): { after?: string; before?: string } {
  if (index > 0 && ids[index - 1]) return { after: ids[index - 1] }
  if (ids[index]) return { before: ids[index] }
  return {}
}
