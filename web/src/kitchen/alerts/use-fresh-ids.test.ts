import { act, renderHook } from '@testing-library/react'
import { describe, expect, it } from 'vitest'

import { useFreshIds } from './use-fresh-ids'

/**
 * What counts as new to a kitchen screen.
 *
 * The behaviour worth pinning is the seeding. A chef who reloads the pass in the
 * middle of a service must not be chimed at, marked at, and interrupted for nine
 * tickets that have been cooking for twenty minutes. That is the failure this
 * hook exists to prevent, and it is invisible until somebody reloads.
 */
describe('useFreshIds', () => {
  it('treats the first answer as the starting point, not as an arrival', () => {
    const { result } = renderHook(({ ids }) => useFreshIds(ids), {
      initialProps: { ids: ['a', 'b', 'c'] as readonly string[] },
    })

    expect([...result.current]).toEqual([])
  }) // covers: AC-14

  it('reports only what arrived since the answer before', () => {
    const { result, rerender } = renderHook(({ ids }) => useFreshIds(ids), {
      initialProps: { ids: ['a'] as readonly string[] },
    })

    act(() => {
      rerender({ ids: ['a', 'b'] })
    })

    expect([...result.current]).toEqual(['b'])
  }) // covers: AC-14

  it('stops calling a ticket new once the screen has asked again', () => {
    // Without this a New badge would sit on a ticket for the rest of the shift,
    // and "new" would stop meaning anything on a screen that refetches all
    // evening.
    const { result, rerender } = renderHook(({ ids }) => useFreshIds(ids), {
      initialProps: { ids: ['a'] as readonly string[] },
    })

    act(() => {
      rerender({ ids: ['a', 'b'] })
    })
    expect([...result.current]).toEqual(['b'])

    act(() => {
      rerender({ ids: ['a', 'b'] })
    })
    expect([...result.current]).toEqual([])
  }) // covers: AC-14

  it('counts a ticket that left and came back as new again', () => {
    // An undo brings a ticket back to the cooking queue, and it is new work to a
    // chef who had already stopped looking at it.
    const { result, rerender } = renderHook(({ ids }) => useFreshIds(ids), {
      initialProps: { ids: ['a', 'b'] as readonly string[] },
    })

    act(() => {
      rerender({ ids: ['a'] })
    })
    expect([...result.current]).toEqual([])

    act(() => {
      rerender({ ids: ['a', 'b'] })
    })
    expect([...result.current]).toEqual(['b'])
  }) // covers: AC-14

  it('reports nothing at all before an answer has arrived', () => {
    const { result } = renderHook(({ ids }) => useFreshIds(ids), {
      initialProps: { ids: null as readonly string[] | null },
    })

    expect([...result.current]).toEqual([])
  }) // covers: AC-14
})
