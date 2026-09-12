import type { Menu } from '@/shared/api/menu'

/**
 * The waiter's unsent basket, and the one rule about it that the menu can
 * break while it is being built.
 *
 * A basket lives only in the table screen until it is sent. Nothing about it
 * reaches the database, so when the kitchen switches a dish off or an admin
 * removes one, the basket is the one place that cannot be told by the server.
 * It is told by the menu instead: the live event refetches the menu, and every
 * line is checked against the fresh copy.
 */

/** One line in the basket. */
export interface BasketLine {
  /** How many. */
  quantity: number
  /**
   * What the dish was called when it went in. Kept here because a dish removed
   * from the menu is no longer in the menu to look its name up, and the waiter
   * still has to be told which line to take out.
   */
  name: string
}

/** The basket, by dish id. */
export type Basket = Readonly<Record<string, BasketLine>>

/**
 * The dishes in the basket that can no longer be ordered.
 *
 * A dish is flagged when the menu no longer lists it (removed) or lists it as
 * unavailable (switched off). With no menu in hand yet, nothing is flagged: a
 * loading screen must not tell a waiter their order is wrong.
 *
 * This is a courtesy, not the control. The server refuses a ticket carrying
 * such a dish whole, with `dish_not_orderable`, whatever this says.
 */
export function unorderableDishes(basket: Basket, menu: Menu | undefined): string[] {
  if (!menu) return []

  const available = new Map<string, boolean>()
  for (const category of menu.categories) {
    for (const dish of category.dishes) available.set(dish.id, dish.available)
  }

  return Object.keys(basket).filter((dishId) => available.get(dishId) !== true)
}

/** The basket with one more of a dish. */
export function addOne(basket: Basket, dishId: string, name: string): Basket {
  const current = basket[dishId]
  return { ...basket, [dishId]: { name, quantity: (current?.quantity ?? 0) + 1 } }
}

/** The basket with one fewer of a dish, dropping the line at zero. */
export function removeOne(basket: Basket, dishId: string): Basket {
  const current = basket[dishId]
  if (!current) return basket

  if (current.quantity > 1) {
    return { ...basket, [dishId]: { ...current, quantity: current.quantity - 1 } }
  }

  return takeOut(basket, dishId)
}

/** The basket without a dish at all, whatever quantity it had. */
export function takeOut(basket: Basket, dishId: string): Basket {
  const next = { ...basket }
  delete next[dishId]
  return next
}

/** How many dishes are in the basket, counting each plate. */
export function basketSize(basket: Basket): number {
  return Object.values(basket).reduce((total, line) => total + line.quantity, 0)
}
