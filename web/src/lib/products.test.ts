import { describe, expect, it } from 'vitest'
import { label, picture, preferred } from './products'

describe('products (mirror of crates/domain/src/products.rs)', () => {
  it('prefers the shop preference', () => {
    expect(preferred('farmi', 'alma')).toBe('farmi')
    expect(preferred(null, 'alma')).toBe('alma')
    expect(preferred(undefined, undefined)).toBe(null)
  })

  it('picks the picture like the server', () => {
    expect(picture('p', 'q', 'c')).toEqual({ image: 'p', from: 'pinned' })
    expect(picture(null, 'q', 'c')).toEqual({ image: 'c', from: 'catalog' })
    expect(picture(null, 'q', null)).toBe(null)
    expect(picture(undefined, 'q', 'c')).toEqual({ image: 'q', from: 'preferred' })
    expect(picture(undefined, null, 'c')).toEqual({ image: 'c', from: 'catalog' })
    expect(picture(undefined, undefined, undefined)).toBe(null)
  })

  it('labels', () => {
    expect(label('Alma', 'Piim 2.5%')).toBe('Alma · Piim 2.5%')
    expect(label(' ', 'Piim')).toBe('Piim')
    expect(label('Alma', '')).toBe('Alma')
  })
})
