import { describe, expect, it } from 'vitest'
import { match } from './router.svelte'

describe('match', () => {
  it('maps paths to routes', () => {
    expect(match('/')).toEqual({ name: 'shopping' })
    expect(match('/shopping/')).toEqual({ name: 'shopping' })
    expect(match('/wishlist')).toEqual({ name: 'wishlist' })
    expect(match('/templates')).toEqual({ name: 'templates' })
    expect(match('/settings')).toEqual({ name: 'settings' })
    expect(match('/list/01jabcdefghjkmnpqrstvwxyz0')).toEqual({ name: 'shopping', listId: '01JABCDEFGHJKMNPQRSTVWXYZ0' })
    expect(match('/list/nope')).toEqual({ name: 'notfound' })
    expect(match('/settings/catalog')).toEqual({ name: 'settings', page: 'catalog' })
    expect(match('/nope')).toEqual({ name: 'notfound' })
  })
})
