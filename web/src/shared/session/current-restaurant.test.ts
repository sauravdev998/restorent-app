import { describe, expect, it } from 'vitest'

import { currentRestaurantId } from './current-restaurant'

describe('currentRestaurantId', () => {
  it('prefers the restaurant named in the address bar', () => {
    const chosen = '22222222-2222-2222-2222-222222222222'
    window.history.replaceState({}, '', `/?restaurant_id=${chosen}`)

    expect(currentRestaurantId()).toBe(chosen)
  })

  it('falls back to a fixed restaurant so the app runs with no setup', () => {
    window.history.replaceState({}, '', '/')

    expect(currentRestaurantId()).toBe('11111111-1111-1111-1111-111111111111')
  })
})
