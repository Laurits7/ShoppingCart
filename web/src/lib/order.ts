// Fractional-index ordering keys. Mirror of crates/domain/src/order.rs.
const DIGITS = '0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz'

function midpoint(a: string, b: string | null): string {
  if (b !== null) {
    let n = 0
    while (n < b.length && (a[n] ?? '0') === b[n]) n++
    if (n > 0) return b.slice(0, n) + midpoint(a.slice(n), b.slice(n))
  }
  const da = a ? DIGITS.indexOf(a[0]) : 0
  const db = b !== null && b.length ? DIGITS.indexOf(b[0]) : DIGITS.length
  if (db - da > 1) return DIGITS[Math.floor((da + db) / 2)]
  if (b !== null && b.length > 1) return b[0]
  return DIGITS[da] + midpoint(a.slice(1), null)
}

/** A key strictly between a ('' = start) and b (null = end). */
export const keyBetween = (a: string | null | undefined, b: string | null | undefined) => midpoint(a ?? '', b ?? null)

/** Key for moving an item to `index` within `keys` (the list without the moved item). */
export const keyAt = (keys: string[], index: number) => keyBetween(keys[index - 1] ?? '', keys[index] ?? null)
