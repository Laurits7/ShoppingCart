import { describe, expect, it } from 'vitest'
import { match } from './router.svelte'

describe('match', () => {
  it('maps paths to routes', () => {
    expect(match('/')).toEqual({ name: 'shopping' })
    expect(match('/shopping/')).toEqual({ name: 'shopping' })
    expect(match('/wishlist')).toEqual({ name: 'wishlist' })
    expect(match('/settings')).toEqual({ name: 'settings' })
    expect(match('/nope')).toEqual({ name: 'notfound' })
  })
})
