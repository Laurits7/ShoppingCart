// Item catalogue ranking: suggestions on every keystroke and the frequent-items chips.
// Mirror of crates/domain/src/catalog.rs; keep the two in step (the tests share their cases).

export const normalizeName = (s: string) => s.split(/\s+/).filter(Boolean).join(' ').toLowerCase()

/** How a name matches what's typed; lower is better. */
export const Match = { Prefix: 0, WordStart: 1, Substring: 2 } as const
export type Match = (typeof Match)[keyof typeof Match]

/** Both arguments normalised. */
export function matchKind(name: string, query: string): Match | null {
  if (!query) return null
  if (name.startsWith(query)) return Match.Prefix
  for (let i = name.indexOf(query); i > 0; i = name.indexOf(query, i + 1)) {
    if (' -(/'.includes(name[i - 1])) return Match.WordStart
  }
  return name.includes(query) ? Match.Substring : null
}

export type Usage = { useCount: number; daysSinceUse: number; shopCount: number }

export function score(u: Usage): number {
  return Math.log(1 + Math.max(0, u.useCount)) + 2 * 0.5 ** (Math.max(0, u.daysSinceUse) / 30) + 1.5 * Math.log(1 + Math.max(0, u.shopCount))
}

const byName = (a: string, b: string) => (a < b ? -1 : a > b ? 1 : 0)

/** Indices of the entries matching `query`, best first. Entries are [normalised name, usage]. */
export function rank(entries: [string, Usage][], query: string, limit: number): number[] {
  const q = normalizeName(query)
  const hits: [Match, number, number][] = []
  entries.forEach(([name, u], i) => {
    const m = matchKind(name, q)
    if (m !== null) hits.push([m, score(u), i])
  })
  hits.sort((a, b) => a[0] - b[0] || b[1] - a[1] || byName(entries[a[2]][0], entries[b[2]][0]))
  return hits.slice(0, limit).map((h) => h[2])
}

/** Indices of the entries bought at this shop, most frequent first. */
export function frequent(entries: [string, Usage][], limit: number): number[] {
  const hits: [number, number][] = []
  entries.forEach(([, u], i) => {
    if (u.shopCount > 0) hits.push([score(u), i])
  })
  hits.sort((a, b) => b[0] - a[0] || byName(entries[a[1]][0], entries[b[1]][0]))
  return hits.slice(0, limit).map((h) => h[1])
}
