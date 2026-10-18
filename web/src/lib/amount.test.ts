import { describe, expect, it } from 'vitest'
import { combine, formatAmount, normalizeUnit, parse, step } from './amount'

const p = (name: string, amount: number | null = null, unit: string | null = null) => ({ name, amount, unit })

describe('amounts (mirror of crates/domain/src/amount.rs)', () => {
  it('parses quick-add lines like the server', () => {
    expect(parse('milk')).toEqual(p('milk'))
    expect(parse('  milk   2 ')).toEqual(p('milk', 2))
    expect(parse('flour 1 kg')).toEqual(p('flour', 1, 'kg'))
    expect(parse('flour 1,5 KG')).toEqual(p('flour', 1.5, 'kg'))
    expect(parse('butter 500g')).toEqual(p('butter', 500, 'g'))
    expect(parse('2 milk')).toEqual(p('milk', 2))
    expect(parse('1 l oat milk')).toEqual(p('oat milk', 1, 'l'))
    expect(parse('eggs x10')).toEqual(p('eggs', 10))
    expect(parse('3x lemons')).toEqual(p('lemons', 3))
    expect(parse('eggs 10 pcs')).toEqual(p('eggs', 10))
    expect(parse('piim 2.5% 1 l')).toEqual(p('piim 2.5%', 1, 'l'))
    expect(parse('water 6 bottles')).toEqual(p('water', 6, 'bottle'))
    expect(parse('7up')).toEqual(p('7up'))
    expect(parse('milk 2.5%')).toEqual(p('milk 2.5%'))
    expect(parse('2')).toEqual(p('2'))
    expect(parse('1 kg')).toEqual(p('1 kg'))
    expect(parse('milk 0')).toEqual(p('milk 0'))
    expect(parse('flour 1 big')).toEqual(p('flour 1 big'))
    expect(parse('1.2.3 x')).toEqual(p('1.2.3 x'))
  })

  it('normalises units', () => {
    expect(normalizeUnit(' KG ')).toBe('kg')
    expect(normalizeUnit('pcs')).toBe(null)
    expect(normalizeUnit('')).toBe(null)
    expect(normalizeUnit('Tin')).toBe('tin')
  })

  it('steps', () => {
    expect(step(1, null, null, true)).toBe(2)
    expect(step(1, null, null, false)).toBe(null)
    expect(step(2, null, null, false)).toBe(1)
    expect(step(1, 'kg', null, false)).toBe(0.5)
    expect(step(0.5, 'kg', null, false)).toBe(null)
    expect(step(0.3, 'kg', null, true)).toBe(0.8)
    expect(step(500, 'g', null, true)).toBe(600)
    expect(step(100, 'g', null, false)).toBe(null)
    expect(step(2, 'pack', 0.25, false)).toBe(1.75)
    expect(step(1, null, 0, true)).toBe(2)
  })

  it('combines', () => {
    expect(combine(2, null, 1, null)).toBe(3)
    expect(combine(1, 'kg', 500, 'g')).toBe(1.5)
    expect(combine(500, 'g', 1, 'kg')).toBe(1500)
    expect(combine(1, 'l', 2, 'dl')).toBe(1.2)
    expect(combine(1, 'l', 1, null)).toBe(null)
    expect(combine(1, 'kg', 1, 'l')).toBe(null)
    expect(combine(1, 'tin', 2, 'tin')).toBe(3)
  })

  it('formats', () => {
    expect(formatAmount(2, null)).toBe('2')
    expect(formatAmount(1.5, 'kg')).toBe('1.5 kg')
    expect(formatAmount(0.1 + 0.2, 'l')).toBe('0.3 l')
  })
})
