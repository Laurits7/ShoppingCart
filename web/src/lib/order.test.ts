import { describe, expect, it } from 'vitest'
import { keyAt, keyBetween } from './order'

describe('ordering keys (mirror of crates/domain/src/order.rs)', () => {
  it('matches the Rust implementation on known cases', () => {
    expect(keyBetween('', null)).toBe('V')
    expect(keyBetween('V', null)).toBe('k')
    expect(keyBetween('', 'V')).toBe('F')
  })

  it('always lands strictly between its neighbours and never ends in 0', () => {
    const keys = [keyBetween('', null)]
    for (let i = 0; i < 500; i++) {
      const pos = (i * 7) % (keys.length + 1)
      const k = keyAt(keys, pos)
      if (pos > 0) expect(keys[pos - 1] < k).toBe(true)
      if (pos < keys.length) expect(k < keys[pos]).toBe(true)
      expect(k.endsWith('0')).toBe(false)
      keys.splice(pos, 0, k)
    }
    expect([...keys].sort()).toEqual(keys)
  })

  it('handles adjacent digits and prefixes', () => {
    for (const [a, b] of [
      ['a', 'b'],
      ['az', 'b'],
      ['a', 'a1'],
    ]) {
      const k = keyBetween(a, b)
      expect(a < k && k < b).toBe(true)
    }
  })
})
