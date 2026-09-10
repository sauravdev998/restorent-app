import { describe, expect, it } from 'vitest'

import {
  ENTITY_KINDS,
  FAN_OUT,
  floorKey,
  isEntityKind,
  kitchenKey,
  menuKey,
  visitKey,
} from './query-keys'

/**
 * The map from a live event to the queries it invalidates.
 *
 * Both halves of this are worth guarding. Too narrow and a screen goes stale
 * with nothing reporting an error; too wide and a chef marking one dish sends
 * every open browser back to the API for the menu, the floor, and every table.
 */

describe('the fan out map', () => {
  it('answers for every entity kind the server can send', () => {
    // A kind with no row would throw at the call site rather than being
    // ignored, on a screen somebody is working from.
    for (const kind of ENTITY_KINDS) {
      expect(FAN_OUT[kind], `no fan out row for ${kind}`).toBeDefined()
    }
  }) // covers: AC-15

  it('sends a line change to the kitchen and to the table, and nowhere else', () => {
    expect(FAN_OUT.order_line.map((key) => key.join('/'))).toEqual(['order_round', 'visit'])
  }) // covers: AC-15

  it('sends a dish change to the menu and to every open table', () => {
    // The second half is the one that is easy to miss: a waiter with the
    // ordering screen open on a table is holding the menu that changed.
    expect(FAN_OUT.dish.map((key) => key.join('/'))).toEqual(['dish', 'visit'])
  }) // covers: AC-15

  it('sends a table or staff change only to the floor', () => {
    expect(FAN_OUT.dining_table).toEqual([floorKey])
    expect(FAN_OUT.staff).toEqual([floorKey])
  }) // covers: AC-15

  it('sends a probe nowhere at all', () => {
    // It carries no product meaning. Invalidating on it would send every open
    // screen back to the API for a message about nothing.
    expect(FAN_OUT.probe).toEqual([])
  }) // covers: AC-15

  it('never invalidates a whole cache by naming an empty prefix', () => {
    // `queryKey: []` matches every query there is, which is the blanket refetch
    // this map exists to replace.
    for (const kind of ENTITY_KINDS) {
      for (const key of FAN_OUT[kind]) {
        expect(key.length, `${kind} invalidates everything`).toBeGreaterThan(0)
      }
    }
  }) // covers: AC-15
})

describe('the query keys', () => {
  it('each start with the entity kind that feeds them', () => {
    // This is the mechanism, not a naming convention: an event can invalidate
    // exactly the queries built from its entity because those queries are the
    // ones whose key begins with that word.
    for (const key of [floorKey, visitKey('any-visit'), kitchenKey, menuKey]) {
      const [first] = key
      expect(first).toBeDefined()
      expect(
        isEntityKind(first as string),
        `${key.join('/')} does not start with an entity kind`,
      ).toBe(true)
    }
  }) // covers: AC-15

  it('put the floor and one table under the same prefix', () => {
    // A visit event refreshes both at once, which is what makes a table opening
    // and a table closing show up on every waiter's floor.
    expect(floorKey[0]).toBe(visitKey('any-visit')[0])
  }) // covers: AC-1, AC-15

  it('give two visits two different keys', () => {
    expect(visitKey('one')).not.toEqual(visitKey('two'))
  }) // covers: AC-15
})

describe('isEntityKind', () => {
  it('recognises what the server sends', () => {
    expect(isEntityKind('order_round')).toBe(true)
    expect(isEntityKind('visit')).toBe(true)
  }) // covers: AC-15

  it('refuses anything else rather than guessing', () => {
    // Acting on the wrong entity is a bug a user sees; a dropped event is one
    // the next stream open fixes.
    expect(isEntityKind('table_section')).toBe(false)
    expect(isEntityKind('orderround')).toBe(false)
    expect(isEntityKind('')).toBe(false)
  }) // covers: AC-15
})
