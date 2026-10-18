// Item amounts and units: quick-add parsing, −/+ steps, adding duplicates.
// Mirror of crates/domain/src/amount.rs; keep the two in step (the tests share their cases).

export const MAX_AMOUNT = 100_000

export const round = (x: number) => Math.round(x * 1000) / 1000

// Canonical unit for a known spelling; null = pieces; undefined = not a known unit.
const UNITS: Record<string, string | null> = {}
for (const [canon, words] of [
  ['kg', 'kg kgs kilo kilos kilogram kilograms'],
  ['g', 'g gr gram grams'],
  ['l', 'l ltr liter liters litre litres'],
  ['dl', 'dl'],
  ['cl', 'cl'],
  ['ml', 'ml'],
  ['pack', 'pack packs pk pkg'],
  ['bottle', 'bottle bottles'],
  ['can', 'can cans'],
  ['box', 'box boxes'],
  ['bag', 'bag bags'],
  ['jar', 'jar jars'],
  ['bunch', 'bunch bunches'],
  ['', 'pc pcs piece pieces tk'],
] as const)
  for (const w of words.split(' ')) UNITS[w] = canon || null

/** The canonical unit; null for pieces. Unknown units are kept, trimmed and lower-cased. */
export function normalizeUnit(u: string | null | undefined): string | null {
  const s = (u ?? '').trim().toLowerCase()
  if (!s) return null
  const k = UNITS[s]
  return k === undefined ? s : k
}

/** Units offered in the editor. */
export const COMMON_UNITS = ['', 'kg', 'g', 'l', 'dl', 'ml', 'pack', 'bottle', 'can', 'box', 'bag', 'jar', 'bunch']

export function defaultStep(unit: string | null | undefined): number {
  if (unit === 'kg' || unit === 'l') return 0.5
  if (unit === 'g' || unit === 'ml') return 100
  return 1
}

/** The amount after one press of + or −; null means − at the smallest step (remove it). */
export function step(amount: number, unit: string | null | undefined, stepOverride: number | null | undefined, up: boolean): number | null {
  const s = stepOverride && stepOverride > 0 ? stepOverride : defaultStep(unit)
  if (up) return round(Math.min(amount + s, MAX_AMOUNT))
  if (amount - s < 1e-9) return null
  return round(amount - s)
}

const BASE: Record<string, [string, number]> = {
  g: ['g', 1],
  kg: ['g', 1000],
  ml: ['ml', 1],
  cl: ['ml', 10],
  dl: ['ml', 100],
  l: ['ml', 1000],
}

/** Adding `amount unit` to an item of `existing existingUnit`: the new total in the existing
 * unit, or null when the units don't mix. */
export function combine(existing: number, existingUnit: string | null, amount: number, unit: string | null): number | null {
  if ((existingUnit ?? null) === (unit ?? null)) return round(Math.min(existing + amount, MAX_AMOUNT))
  const e = existingUnit ? BASE[existingUnit] : undefined
  const a = unit ? BASE[unit] : undefined
  if (!e || !a || e[0] !== a[0]) return null
  return round(Math.min(existing + (amount * a[1]) / e[1], MAX_AMOUNT))
}

export type Parsed = { name: string; amount: number | null; unit: string | null }

function number(s: string): number | null {
  const t = s.replace(/,/g, '.')
  if (!/^[0-9.]+$/.test(t) || !/[0-9]/.test(t) || (t.match(/\./g)?.length ?? 0) > 1) return null
  const n = Number(t)
  return Number.isFinite(n) && n > 0 && n <= MAX_AMOUNT ? round(n) : null
}

function glued(tok: string): [number, string | null] | null {
  let t = tok.toLowerCase()
  if (t.startsWith('x') || t.startsWith('×')) t = t.slice(1)
  else if (t.endsWith('x') || t.endsWith('×')) t = t.slice(0, -1)
  const n = number(t)
  if (n !== null) return [n, null]
  const i = t.search(/[^0-9.,]/)
  if (i < 0) return null
  const u = UNITS[t.slice(i)]
  const a = number(t.slice(0, i))
  return u === undefined || a === null ? null : [a, u]
}

/** Split a quick-add line ("milk 2", "flour 1 kg", "2 milk") into name, amount and unit. */
export function parse(text: string): Parsed {
  const toks = text.split(/\s+/).filter(Boolean)
  const whole: Parsed = { name: toks.join(' '), amount: null, unit: null }
  const n = toks.length
  if (n < 2) return whole
  const make = (name: string[], [amount, unit]: [number, string | null]): Parsed =>
    name.length === 1 && UNITS[name[0].toLowerCase()] !== undefined ? whole : { name: name.join(' '), amount, unit }
  if (n >= 3) {
    const a = number(toks[n - 2])
    const u = UNITS[toks[n - 1].toLowerCase()]
    if (a !== null && u !== undefined) return make(toks.slice(0, n - 2), [a, u])
  }
  const last = glued(toks[n - 1])
  if (last) return make(toks.slice(0, n - 1), last)
  if (n >= 3) {
    const a = number(toks[0])
    const u = UNITS[toks[1].toLowerCase()]
    if (a !== null && u !== undefined) return make(toks.slice(2), [a, u])
  }
  const first = glued(toks[0])
  if (first) return make(toks.slice(1), first)
  return whole
}

/** "2", "1.5 kg", "500 g": for display. */
export function formatAmount(amount: number, unit: string | null | undefined): string {
  const n = String(round(amount))
  return unit ? `${n} ${unit}` : n
}
