import { beforeEach, describe, expect, it } from 'vitest'

import type { Menu } from '@/shared/api/menu'

import {
  addOne,
  addToLine,
  basketSize,
  EMPTY_BASKET,
  loadBasket,
  NOTE_MAX_CHARS,
  noteTooLong,
  parseBasket,
  removeOne,
  saveBasket,
  sendLines,
  setNote,
  splitOne,
  takeOut,
  unorderableDishes,
  type Basket,
} from './basket'

/**
 * The unsent basket, and the rule the menu can break under it.
 *
 * Spec 0008 AC-12: a dish in the basket that is switched off or removed is
 * flagged within a second or two, from the menu the live stream keeps fresh,
 * and sending is blocked until the waiter takes it out.
 *
 * Spec 0011: a line per dish and note, a send key made with the first line,
 * and the whole thing kept per visit in session storage.
 */

const TIKKA = 'dish-tikka'
const SOUP = 'dish-soup'
const GONE = 'dish-gone'

/** A menu in which the tikka is on and the soup is whatever the test says. */
function menu(soupAvailable: boolean): Menu {
  return {
    currencyCode: 'INR',
    currencyDecimals: 2,
    categories: [
      {
        id: 'starters',
        name: 'Starters',
        dishes: [
          {
            id: TIKKA,
            name: 'Paneer tikka',
            description: null,
            price: '320.0000',
            diet: 'veg',
            available: true,
          },
          {
            id: SOUP,
            name: 'Soup',
            description: null,
            price: '180.0000',
            diet: 'veg',
            available: soupAvailable,
          },
        ],
      },
    ],
  }
}

const BASKET: Basket = {
  clientKey: 'key-1',
  lines: [
    { dishId: TIKKA, name: 'Paneer tikka', quantity: 2, note: '' },
    { dishId: SOUP, name: 'Soup', quantity: 1, note: '' },
  ],
}

describe('unorderableDishes', () => {
  it('flags nothing while every dish in the basket can be ordered', () => {
    expect(unorderableDishes(BASKET, menu(true))).toEqual([])
  }) // covers: AC-12 (spec 0008)

  it('flags a dish the kitchen switched off', () => {
    expect(unorderableDishes(BASKET, menu(false))).toEqual([SOUP])
  }) // covers: AC-12 (spec 0008)

  it('flags a dish that is no longer on the menu at all', () => {
    // Removed rather than switched off: the menu simply stops listing it.
    const basket: Basket = {
      ...BASKET,
      lines: [...BASKET.lines, { dishId: GONE, name: 'Fish pakora', quantity: 1, note: '' }],
    }
    expect(unorderableDishes(basket, menu(true))).toEqual([GONE])
  }) // covers: AC-12 (spec 0008)

  it('flags nothing before the menu has arrived', () => {
    // A loading screen must not tell a waiter their order is wrong.
    expect(unorderableDishes(BASKET, undefined)).toEqual([])
  }) // covers: AC-12 (spec 0008)
})

describe('the basket', () => {
  it('counts plates and keeps the name each line went in with', () => {
    const one = addOne(EMPTY_BASKET, TIKKA, 'Paneer tikka', () => 'key')
    const two = addOne(one, TIKKA, 'Paneer tikka')

    expect(two.lines).toEqual([{ dishId: TIKKA, name: 'Paneer tikka', quantity: 2, note: '' }])
    expect(basketSize(addOne(two, SOUP, 'Soup'))).toBe(3)
  })

  it('makes its send key with the first line and keeps it after', () => {
    let made = 0
    const makeKey = () => `key-${String(++made)}`

    const one = addOne(EMPTY_BASKET, TIKKA, 'Paneer tikka', makeKey)
    const two = addOne(one, SOUP, 'Soup', makeKey)

    expect(one.clientKey).toBe('key-1')
    expect(two.clientKey).toBe('key-1')
    expect(made).toBe(1)
  }) // covers: AC-8 (spec 0011)

  it('drops a line when its last plate is taken away', () => {
    expect(removeOne(BASKET, 0).lines[0]?.quantity).toBe(1)
    expect(removeOne(BASKET, 1).lines.map((line) => line.dishId)).toEqual([TIKKA])
    expect(removeOne(BASKET, 5)).toBe(BASKET)
    expect(addToLine(BASKET, 1).lines[1]?.quantity).toBe(2)
  })

  it('takes a flagged dish out whole, on every line it is on', () => {
    const split = splitOne(BASKET, 0)
    const left = takeOut(split, TIKKA)
    expect(left.lines.map((line) => line.dishId)).toEqual([SOUP])
    expect(basketSize(left)).toBe(1)
  }) // covers: AC-12 (spec 0008)
})

describe('notes', () => {
  it('keeps one soup with a note and the next one without as two lines', () => {
    let basket = addOne(EMPTY_BASKET, SOUP, 'Soup', () => 'key')
    basket = setNote(basket, 0, 'no onions')
    basket = addOne(basket, SOUP, 'Soup')

    expect(basket.lines).toEqual([
      { dishId: SOUP, name: 'Soup', quantity: 1, note: 'no onions' },
      { dishId: SOUP, name: 'Soup', quantity: 1, note: '' },
    ])
    expect(sendLines(basket)).toEqual([
      { dishId: SOUP, quantity: 1, note: 'no onions' },
      { dishId: SOUP, quantity: 1 },
    ])
  }) // covers: AC-6 (spec 0011)

  it('splits one plate off a line so it can carry its own note', () => {
    const split = splitOne(BASKET, 0)
    expect(split.lines.slice(0, 2).map((line) => line.quantity)).toEqual([1, 1])
    expect(splitOne(BASKET, 1)).toBe(BASKET)
  }) // covers: AC-6 (spec 0011)

  it('trims a note, sends a blank one as none, and merges two lines that match', () => {
    const basket: Basket = {
      clientKey: 'key',
      lines: [
        { dishId: SOUP, name: 'Soup', quantity: 1, note: '  no onions ' },
        { dishId: SOUP, name: 'Soup', quantity: 2, note: 'no onions' },
        { dishId: TIKKA, name: 'Paneer tikka', quantity: 1, note: '   ' },
      ],
    }

    expect(sendLines(basket)).toEqual([
      { dishId: SOUP, quantity: 3, note: 'no onions' },
      { dishId: TIKKA, quantity: 1 },
    ])
  }) // covers: AC-6 (spec 0011)

  it('counts a note in characters, as the server does, not in bytes', () => {
    expect(noteTooLong('प'.repeat(NOTE_MAX_CHARS))).toBe(false)
    expect(noteTooLong('प'.repeat(NOTE_MAX_CHARS + 1))).toBe(true)
    expect(noteTooLong(`  ${'a'.repeat(NOTE_MAX_CHARS)}  `)).toBe(false)
  }) // covers: AC-6 (spec 0011)
})

describe('keeping the basket on the phone', () => {
  beforeEach(() => {
    window.sessionStorage.clear()
  })

  it('survives a save and a load for its own visit only', () => {
    saveBasket('visit-a', BASKET, window.sessionStorage)

    expect(loadBasket('visit-a', window.sessionStorage)).toEqual(BASKET)
    expect(loadBasket('visit-b', window.sessionStorage)).toEqual(EMPTY_BASKET)
  }) // covers: AC-7 (spec 0011)

  it('forgets a visit once its basket is empty', () => {
    saveBasket('visit-a', BASKET, window.sessionStorage)
    saveBasket('visit-a', EMPTY_BASKET, window.sessionStorage)

    expect(window.sessionStorage.getItem('waiter.basket.visit-a')).toBeNull()
  }) // covers: AC-7 (spec 0011)

  it('reads anything that is not a basket as an empty one', () => {
    window.sessionStorage.setItem('waiter.basket.visit-a', '{"lines":"nope"}')
    expect(loadBasket('visit-a', window.sessionStorage)).toEqual(EMPTY_BASKET)

    window.sessionStorage.setItem('waiter.basket.visit-a', 'not json')
    expect(loadBasket('visit-a', window.sessionStorage)).toEqual(EMPTY_BASKET)

    expect(parseBasket({ clientKey: 3, lines: [] })).toBeNull()
    expect(
      parseBasket({
        clientKey: null,
        lines: [{ dishId: SOUP, name: 'Soup', quantity: 0, note: '' }],
      }),
    ).toBeNull()
  }) // covers: AC-7 (spec 0011)

  it('keeps working when storage refuses', () => {
    const denied = (): never => {
      throw new Error('denied')
    }
    const refusing: Storage = {
      length: 0,
      clear: denied,
      getItem: denied,
      key: denied,
      removeItem: denied,
      setItem: denied,
    }

    expect(() => {
      saveBasket('visit-a', BASKET, refusing)
    }).not.toThrow()
    expect(loadBasket('visit-a', refusing)).toEqual(EMPTY_BASKET)
  }) // covers: AC-7 (spec 0011)
})
