import { describe, expect, it } from 'vitest'

import { DEFAULT_SURFACE, followsRestaurantLanguage, SURFACES, surfaceForPath } from './surface'

/**
 * Which surface a screen is on, and what that decides.
 *
 * Three separate things read this: the density on the document element, which
 * translation namespace is fetched, and whether a personal language setting is
 * consulted at all. So a path that lands on the wrong surface is not a cosmetic
 * mistake, it is a chef's screen changing language under them mid shift.
 */

describe('surfaceForPath', () => {
  it('reads the surface off the first segment of the path', () => {
    expect(surfaceForPath('/admin')).toBe('admin')
    expect(surfaceForPath('/waiter')).toBe('waiter')
    expect(surfaceForPath('/kitchen')).toBe('kitchen')
  }) // covers: AC-6

  it('keeps a nested screen on its own surface', () => {
    // A waiter deep in a bill is still a waiter, and still wants the waiter
    // namespace rather than the admin one.
    expect(surfaceForPath('/waiter/tables/12/bill')).toBe('waiter')
    expect(surfaceForPath('/kitchen/tickets')).toBe('kitchen')
  }) // covers: AC-5

  it('treats a path outside the three route groups as the desk surface', () => {
    // The system status page and a not found page are read at a desk, not on a
    // phone or across a kitchen.
    expect(surfaceForPath('/')).toBe(DEFAULT_SURFACE)
    expect(surfaceForPath('/status')).toBe(DEFAULT_SURFACE)
    expect(surfaceForPath('/design')).toBe(DEFAULT_SURFACE)
    expect(surfaceForPath('')).toBe(DEFAULT_SURFACE)
  }) // covers: AC-6

  it('does not mistake a longer word that merely starts the same way', () => {
    // The case an implementation written with `startsWith` gets wrong, and it
    // would put a kitchen screen on the waiter's personal language.
    expect(surfaceForPath('/waiterly')).not.toBe('waiter')
    expect(surfaceForPath('/kitchenware')).not.toBe('kitchen')
    expect(surfaceForPath('/administration')).toBe(DEFAULT_SURFACE)
  }) // covers: AC-6

  it('matches the segment exactly, so casing is not close enough', () => {
    // A link written with a capital would otherwise resolve to a surface the
    // router itself will not serve.
    expect(surfaceForPath('/Kitchen')).not.toBe('kitchen')
    expect(surfaceForPath('/WAITER')).not.toBe('waiter')
  }) // covers: AC-6

  it('needs the leading slash a pathname always has', () => {
    // Documented rather than defended: every caller passes
    // `window.location.pathname` or a router path, and both begin with a slash.
    expect(surfaceForPath('kitchen')).toBe(DEFAULT_SURFACE)
  }) // covers: AC-6
})

describe('followsRestaurantLanguage', () => {
  it('says the kitchen follows the restaurant rather than the person', () => {
    // The shared appliance. Several chefs read it across a shift handover and
    // none of them signed into it personally.
    expect(followsRestaurantLanguage('kitchen')).toBe(true)
  }) // covers: AC-6

  it('lets the admin and the waiter follow the person', () => {
    expect(followsRestaurantLanguage('admin')).toBe(false)
    expect(followsRestaurantLanguage('waiter')).toBe(false)
  }) // covers: AC-6

  it('makes the kitchen the only exception, not one of several', () => {
    // If a later surface joins this list, the personal setting silently stops
    // working somewhere. That should be a deliberate change, so it fails here.
    const following = SURFACES.filter(followsRestaurantLanguage)
    expect(following).toEqual(['kitchen'])
  }) // covers: AC-6

  it('is the same rule the switcher is hidden by', () => {
    // The shell hides the switcher on exactly the surfaces that follow the
    // restaurant, so these two answers can never disagree.
    expect(SURFACES.filter((surface) => !followsRestaurantLanguage(surface))).toEqual([
      'admin',
      'waiter',
    ])
  }) // covers: AC-17
})
