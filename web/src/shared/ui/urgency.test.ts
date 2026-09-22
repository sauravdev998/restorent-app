import { describe, expect, it } from 'vitest'

import { urgencyOf } from './urgency'

/**
 * The three bands a kitchen ticket's age falls in.
 *
 * Small enough to look obvious, and worth pinning anyway: the boundaries are
 * inclusive on purpose, so a ticket at exactly the threshold has already crossed
 * it. A restaurant that sets fifteen minutes means "red at fifteen", not "red at
 * fifteen and one second", and a chef watching the clock tick to 15:00 expects
 * the colour to change on that number.
 */
describe('urgencyOf', () => {
  it('is calm until the amber threshold', () => {
    expect(urgencyOf(0, 600, 900)).toBe('calm')
    expect(urgencyOf(599, 600, 900)).toBe('calm')
  }) // covers: AC-3

  it('turns amber on the threshold itself, not a second later', () => {
    expect(urgencyOf(600, 600, 900)).toBe('warning')
    expect(urgencyOf(899, 600, 900)).toBe('warning')
  }) // covers: AC-3

  it('turns red on its own threshold, and stays red', () => {
    expect(urgencyOf(900, 600, 900)).toBe('late')
    expect(urgencyOf(9_000, 600, 900)).toBe('late')
  }) // covers: AC-3

  it('never draws amber where no amber threshold was given', () => {
    // Every screen outside the kitchen passes one threshold, because one colour
    // is all a waiter's phone needs.
    expect(urgencyOf(700, undefined, 900)).toBe('calm')
    expect(urgencyOf(900, undefined, 900)).toBe('late')
  }) // covers: AC-3

  it('reads a pair stored the wrong way round as the more urgent of the two', () => {
    // The database holds a check constraint against this pair existing, so it
    // should be unreachable. If one ever is reached, a screen that quietly called
    // a twenty minute old ticket calm would be the worst of the available
    // answers.
    expect(urgencyOf(1_000, 900, 600)).toBe('late')
  }) // covers: AC-3
})
