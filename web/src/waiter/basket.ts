import type { Menu } from '@/shared/api/menu'

/**
 * The waiter's unsent basket: what it holds, the key its send carries, and the
 * one rule about it that the menu can break while it is being built.
 *
 * A basket never reaches the database until it is sent. It is kept on the
 * phone per visit, in session storage, so leaving the table for another
 * screen or reloading loses nothing (spec 0011, AC-7). Because it is saved
 * against the visit rather than the table, it follows a party that is moved.
 *
 * **The send key.** A basket gets a key (`clientKey`) the moment its first line
 * goes in, and keeps it until one send succeeds. A waiter who taps Send again
 * after a timeout sends the same key, and the server answers with the ticket
 * the first send made instead of making a second one (AC-8).
 *
 * **The menu can move under it.** When the kitchen switches a dish off or an
 * admin removes one, the basket is told by the menu, which the live stream
 * keeps fresh, and every line is checked against it.
 */

/** The most characters a dish note may hold, counted as the server counts them. */
export const NOTE_MAX_CHARS = 140

/** One line in the basket. */
export interface BasketLine {
  /** Which menu item. */
  readonly dishId: string
  /**
   * What the dish was called when it went in. Kept because a dish removed from
   * the menu can no longer be looked up, and the waiter still has to be told
   * which line to take out.
   */
  readonly name: string
  /** How many. */
  readonly quantity: number
  /** What the guest asked for, as typed. Empty when there is none. */
  readonly note: string
}

/** The basket for one visit. */
export interface Basket {
  /** The key every send of this basket carries, made with its first line. */
  readonly clientKey: string | null
  /** The lines, in the order they went in. */
  readonly lines: readonly BasketLine[]
}

/** A basket with nothing in it and no key yet. */
export const EMPTY_BASKET: Basket = { clientKey: null, lines: [] }

/** One line as the send request carries it. */
export interface SendLine {
  dishId: string
  quantity: number
  note?: string
}

/** How long a note is, the way the server measures it: trimmed, in code points. */
export function noteLength(note: string): number {
  return [...note.trim()].length
}

/** Whether a note is over the ceiling. */
export function noteTooLong(note: string): boolean {
  return noteLength(note) > NOTE_MAX_CHARS
}

/**
 * The basket with one more of a dish.
 *
 * Goes onto the line for that dish that has no note, so tapping + three times
 * is one line of three. A line with a note stays as it is: "no onions" on one
 * soup is not "no onions" on the next one.
 */
export function addOne(
  basket: Basket,
  dishId: string,
  name: string,
  makeKey: () => string = () => crypto.randomUUID(),
): Basket {
  const clientKey = basket.clientKey ?? makeKey()
  const index = basket.lines.findIndex((line) => line.dishId === dishId && line.note.trim() === '')

  if (index === -1) {
    return { clientKey, lines: [...basket.lines, { dishId, name, quantity: 1, note: '' }] }
  }

  return {
    clientKey,
    lines: basket.lines.map((line, at) =>
      at === index ? { ...line, quantity: line.quantity + 1 } : line,
    ),
  }
}

/** The basket with one more on one line. */
export function addToLine(basket: Basket, index: number): Basket {
  return {
    ...basket,
    lines: basket.lines.map((line, at) =>
      at === index ? { ...line, quantity: line.quantity + 1 } : line,
    ),
  }
}

/** The basket with one fewer on one line, dropping the line at zero. */
export function removeOne(basket: Basket, index: number): Basket {
  const line = basket.lines[index]
  if (!line) return basket

  if (line.quantity > 1) {
    return {
      ...basket,
      lines: basket.lines.map((each, at) =>
        at === index ? { ...each, quantity: each.quantity - 1 } : each,
      ),
    }
  }

  return { ...basket, lines: basket.lines.filter((_, at) => at !== index) }
}

/**
 * The basket with one of a line split off onto a line of its own, so it can
 * carry a different note from the rest.
 */
export function splitOne(basket: Basket, index: number): Basket {
  const line = basket.lines[index]
  if (!line || line.quantity < 2) return basket

  const lines = [...basket.lines]
  lines.splice(index, 1, { ...line, quantity: line.quantity - 1 }, { ...line, quantity: 1 })
  return { ...basket, lines }
}

/** The basket with a line's note changed, exactly as typed. */
export function setNote(basket: Basket, index: number, note: string): Basket {
  return {
    ...basket,
    lines: basket.lines.map((line, at) => (at === index ? { ...line, note } : line)),
  }
}

/** The basket without a dish at all, on every line it is on. */
export function takeOut(basket: Basket, dishId: string): Basket {
  return { ...basket, lines: basket.lines.filter((line) => line.dishId !== dishId) }
}

/** How many dishes are in the basket, counting each plate. */
export function basketSize(basket: Basket): number {
  return basket.lines.reduce((total, line) => total + line.quantity, 0)
}

/**
 * What the send carries: one entry per dish and note, the note trimmed and
 * left off when it is blank.
 *
 * Two lines that ended up with the same dish and the same note go as one, with
 * their quantities added, so the kitchen reads "2 × soup, no onions" rather
 * than the same instruction twice.
 */
export function sendLines(basket: Basket): SendLine[] {
  const merged = new Map<string, SendLine>()

  for (const line of basket.lines) {
    const note = line.note.trim()
    const key = JSON.stringify([line.dishId, note])
    const existing = merged.get(key)

    if (existing) {
      existing.quantity += line.quantity
    } else {
      merged.set(
        key,
        note === ''
          ? { dishId: line.dishId, quantity: line.quantity }
          : { dishId: line.dishId, quantity: line.quantity, note },
      )
    }
  }

  return [...merged.values()]
}

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

  return [
    ...new Set(basket.lines.map((line) => line.dishId).filter((id) => available.get(id) !== true)),
  ]
}

// ===========================================================================
// Keeping it on the phone
// ===========================================================================

/** Where one visit's basket is kept. */
export function basketStorageKey(visitId: string): string {
  return `waiter.basket.${visitId}`
}

/**
 * Reads a saved basket back, or `null` for anything that is not one.
 *
 * Checked field by field rather than cast, because session storage is outside
 * the program: an old shape from an earlier build, or a value somebody typed
 * into the developer tools, must come back as nothing rather than as a basket
 * that crashes the table screen.
 */
export function parseBasket(value: unknown): Basket | null {
  if (typeof value !== 'object' || value === null) return null
  const record = value as Record<string, unknown>

  const clientKey = record['clientKey']
  const lines = record['lines']
  if (clientKey !== null && typeof clientKey !== 'string') return null
  if (!Array.isArray(lines)) return null

  const parsed: BasketLine[] = []
  for (const item of lines as unknown[]) {
    if (typeof item !== 'object' || item === null) return null
    const line = item as Record<string, unknown>
    const { dishId, name, quantity, note } = line
    if (
      typeof dishId !== 'string' ||
      typeof name !== 'string' ||
      typeof quantity !== 'number' ||
      !Number.isInteger(quantity) ||
      quantity < 1 ||
      typeof note !== 'string'
    ) {
      return null
    }
    parsed.push({ dishId, name, quantity, note })
  }

  return { clientKey, lines: parsed }
}

/** The basket saved for a visit, or an empty one. Never throws. */
export function loadBasket(visitId: string, storage: Storage | undefined = sessionStore()): Basket {
  try {
    const raw = storage?.getItem(basketStorageKey(visitId))
    if (raw === null || raw === undefined) return EMPTY_BASKET
    return parseBasket(JSON.parse(raw)) ?? EMPTY_BASKET
  } catch {
    return EMPTY_BASKET
  }
}

/**
 * Saves a visit's basket, or forgets it when it is empty. Never throws: with
 * storage refused the basket simply lives in memory until a reload.
 */
export function saveBasket(
  visitId: string,
  basket: Basket,
  storage: Storage | undefined = sessionStore(),
): void {
  try {
    if (basket.lines.length === 0) {
      storage?.removeItem(basketStorageKey(visitId))
    } else {
      storage?.setItem(basketStorageKey(visitId), JSON.stringify(basket))
    }
  } catch {
    // Private browsing, a full quota, or storage switched off. The basket is
    // still in memory, which is all a waiter who does not reload needs.
  }
}

/** Session storage, if this browser will hand it over at all. */
function sessionStore(): Storage | undefined {
  try {
    return typeof window === 'undefined' ? undefined : window.sessionStorage
  } catch {
    return undefined
  }
}
