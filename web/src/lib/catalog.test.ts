import { describe, expect, it } from 'vitest'
import { frequent, Match, matchKind, normalizeName, rank, score, type Usage } from './catalog'

const u = (useCount: number, daysSinceUse: number, shopCount: number): Usage => ({ useCount, daysSinceUse, shopCount })

describe('catalogue ranking (mirror of crates/domain/src/catalog.rs)', () => {
  it('normalises and matches', () => {
    expect(normalizeName('  Oat   Milk ')).toBe('oat milk')
    expect(normalizeName('Õun')).toBe('õun')
    expect(matchKind('milk', 'mi')).toBe(Match.Prefix)
    expect(matchKind('oat milk', 'mi')).toBe(Match.WordStart)
    expect(matchKind('buttermilk', 'mi')).toBe(Match.Substring)
    expect(matchKind('bread', 'mi')).toBe(null)
    expect(matchKind('bread', '')).toBe(null)
  })

  it('ranks like the server', () => {
    const e: [string, Usage][] = [
      ['buttermilk', u(50, 1, 0)],
      ['oat milk', u(3, 2, 0)],
      ['milk', u(1, 200, 0)],
      ['mint', u(10, 1, 0)],
      ['bread', u(99, 0, 0)],
    ]
    expect(rank(e, 'Mi', 10)).toEqual([3, 2, 1, 0])
    expect(rank(e, 'mi', 2)).toEqual([3, 2])
    expect(rank(e, '', 10)).toEqual([])
    expect(
      rank(
        [
          ['milk', u(5, 1, 0)],
          ['mince', u(5, 1, 3)],
        ],
        'mi',
        10,
      ),
    ).toEqual([1, 0])
  })

  it('scores', () => {
    expect(score(u(10, 0, 0))).toBeGreaterThan(score(u(2, 0, 0)))
    expect(score(u(5, 1, 0))).toBeGreaterThan(score(u(5, 90, 0)))
    expect(score(u(5, 1, 2))).toBeGreaterThan(score(u(5, 1, 0)))
  })

  it('picks frequent items', () => {
    const e: [string, Usage][] = [
      ['milk', u(20, 1, 10)],
      ['bread', u(30, 1, 0)],
      ['eggs', u(5, 3, 5)],
      ['coffee', u(5, 3, 5)],
    ]
    expect(frequent(e, 10)).toEqual([0, 3, 2])
    expect(frequent(e, 1)).toEqual([0])
  })
})
