import { describe, expect, it } from 'vitest'

import type { Menu } from '@/shared/api/menu'

import { addOne, basketSize, removeOne, takeOut, unorderableDishes, type Basket } from './basket'

/**
 * The unsent basket, and the rule the menu can break under it.
 *
 * Spec 0008 AC-12: a dish in the basket that is switched off or removed is
 * flagged within a second or two, from the menu the live stream keeps fresh,
 * and sending is blocked until the waiter takes it out.
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
  [TIKKA]: { name: 'Paneer tikka', quantity: 2 },
  [SOUP]: { name: 'Soup', quantity: 1 },
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
    const basket: Basket = { ...BASKET, [GONE]: { name: 'Fish pakora', quantity: 1 } }
    expect(unorderableDishes(basket, menu(true))).toEqual([GONE])
  }) // covers: AC-12 (spec 0008)

  it('flags nothing before the menu has arrived', () => {
    // A loading screen must not tell a waiter their order is wrong.
    expect(unorderableDishes(BASKET, undefined)).toEqual([])
  }) // covers: AC-12 (spec 0008)
})

describe('the basket', () => {
  it('counts plates and keeps the name each line went in with', () => {
    const one = addOne({}, TIKKA, 'Paneer tikka')
    const two = addOne(one, TIKKA, 'Paneer tikka')

    expect(two[TIKKA]).toEqual({ name: 'Paneer tikka', quantity: 2 })
    expect(basketSize(addOne(two, SOUP, 'Soup'))).toBe(3)
  })

  it('drops a line when its last plate is taken away', () => {
    expect(removeOne(BASKET, TIKKA)[TIKKA]?.quantity).toBe(1)
    expect(removeOne(BASKET, SOUP)[SOUP]).toBeUndefined()
    expect(removeOne(BASKET, GONE)).toBe(BASKET)
  })

  it('takes a flagged line out whole, whatever its quantity', () => {
    const left = takeOut(BASKET, TIKKA)
    expect(left[TIKKA]).toBeUndefined()
    expect(basketSize(left)).toBe(1)
  }) // covers: AC-12 (spec 0008)
})
