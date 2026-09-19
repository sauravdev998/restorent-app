/**
 * The rules behind the waiter's ready alert and the Orders list order, as
 * plain functions of the Orders read.
 *
 * Nothing about the alert is stored on the server (spec 0011, invariant 7).
 * "Ready" is a dish whose status is `ready`, read fresh every time, and a phone
 * keeps only what it has chimed for and what it has acknowledged. So all of
 * the deciding lives here, where it can be tested without a browser.
 */

/** Dishes that turn ready within this long of the first join its alert silently. */
export const GROUP_WINDOW_MS = 3000

/** How often an unacknowledged ready dish chimes again. */
export const REMINDER_MS = 120_000

/** What the vibration motor does for a ready alert, where the browser has one. */
export const VIBRATION_PATTERN = [200, 100, 200]

/** The parts of the Orders read these rules look at. */
export interface OrderLike {
  readonly id: string
  readonly tableLabel: string
  readonly openedAt: string
  readonly responsibleStaffId: string
  readonly rounds: readonly {
    readonly sentAt: string
    readonly lines: readonly {
      readonly id: string
      readonly dishName: string
      readonly quantity: number
      readonly status: string
      readonly readyAt?: string | null
    }[]
  }[]
}

/** One dish waiting on the pass for a waiter to carry out. */
export interface ReadyDish {
  readonly lineId: string
  readonly visitId: string
  readonly tableLabel: string
  readonly dishName: string
  readonly quantity: number
}

/**
 * Every dish that is ready, on every open table, or only on the tables one
 * waiter is responsible for when `responsibleStaffId` is given.
 */
export function readyDishes(
  orders: readonly OrderLike[],
  responsibleStaffId?: string,
): ReadyDish[] {
  const dishes: ReadyDish[] = []

  for (const order of orders) {
    if (responsibleStaffId !== undefined && order.responsibleStaffId !== responsibleStaffId) {
      continue
    }

    for (const round of order.rounds) {
      for (const line of round.lines) {
        if (line.status !== 'ready') continue
        dishes.push({
          lineId: line.id,
          visitId: order.id,
          tableLabel: order.tableLabel,
          dishName: line.dishName,
          quantity: line.quantity,
        })
      }
    }
  }

  return dishes
}

/** How many dishes on one table are waiting to be carried out. */
export function readyCount(order: OrderLike): number {
  return readyDishes([order]).length
}

/**
 * The Orders list in the order a waiter should walk it (AC-2).
 *
 * Tables with a ready dish first, the one whose dish has waited longest at the
 * top. Then tables with a dish still cooking, by their oldest unfinished round.
 * Then the rest, by when they were opened. Worked out here from the response
 * rather than by the server, because it changes every time a dish moves and
 * the read already carries every timestamp it needs.
 */
export function sortOrders<T extends OrderLike>(orders: readonly T[]): T[] {
  return orders
    .map((order) => ({ order, rank: rankOf(order) }))
    .sort(
      (a, b) =>
        a.rank.group - b.rank.group ||
        a.rank.at - b.rank.at ||
        a.order.tableLabel.localeCompare(b.order.tableLabel) ||
        a.order.id.localeCompare(b.order.id),
    )
    .map(({ order }) => order)
}

function rankOf(order: OrderLike): { group: number; at: number } {
  const readyAts: number[] = []
  const cookingSentAts: number[] = []

  for (const round of order.rounds) {
    let cooking = false
    for (const line of round.lines) {
      if (line.status === 'ready') readyAts.push(instant(line.readyAt ?? round.sentAt))
      if (line.status === 'queued') cooking = true
    }
    if (cooking) cookingSentAts.push(instant(round.sentAt))
  }

  if (readyAts.length > 0) return { group: 0, at: Math.min(...readyAts) }
  if (cookingSentAts.length > 0) return { group: 1, at: Math.min(...cookingSentAts) }
  return { group: 2, at: instant(order.openedAt) }
}

function instant(timestamp: string): number {
  const parsed = Date.parse(timestamp)
  return Number.isNaN(parsed) ? Number.MAX_SAFE_INTEGER : parsed
}

/**
 * What one new Orders answer means for this phone's alert.
 *
 * - `fresh`: dishes on my tables that turned ready since this phone last
 *   looked. Each alerts once per phone.
 * - `chime`: whether they start a new alert with a chime, or join the one
 *   that started less than {@link GROUP_WINDOW_MS} ago without a second one.
 * - `seen` and `groupStartedAt`: what to remember for next time.
 *
 * `seen` is `null` until the first answer arrives. That first answer marks
 * every dish already ready as seen and chimes for none of them, so a reload
 * never chimes for food that was already up (AC-11).
 */
export function nextAlert(
  state: { seen: ReadonlySet<string> | null; groupStartedAt: number | null },
  orders: readonly OrderLike[],
  me: string,
  now: number,
): {
  fresh: ReadyDish[]
  chime: boolean
  seen: ReadonlySet<string>
  groupStartedAt: number | null
} {
  if (state.seen === null) {
    return {
      fresh: [],
      chime: false,
      seen: new Set(readyDishes(orders).map((dish) => dish.lineId)),
      groupStartedAt: state.groupStartedAt,
    }
  }

  const seen = state.seen
  const fresh = readyDishes(orders, me).filter((dish) => !seen.has(dish.lineId))
  if (fresh.length === 0) {
    return { fresh, chime: false, seen, groupStartedAt: state.groupStartedAt }
  }

  const joins = state.groupStartedAt !== null && now - state.groupStartedAt < GROUP_WINDOW_MS

  return {
    fresh,
    chime: !joins,
    seen: new Set([...seen, ...fresh.map((dish) => dish.lineId)]),
    groupStartedAt: joins ? state.groupStartedAt : now,
  }
}

/** Dishes grouped by the table they are going to, in the order they came. */
export function byTable(
  dishes: readonly ReadyDish[],
): { visitId: string; tableLabel: string; dishes: ReadyDish[] }[] {
  const tables = new Map<string, { visitId: string; tableLabel: string; dishes: ReadyDish[] }>()

  for (const dish of dishes) {
    const table = tables.get(dish.visitId)
    if (table) {
      table.dishes.push(dish)
    } else {
      tables.set(dish.visitId, {
        visitId: dish.visitId,
        tableLabel: dish.tableLabel,
        dishes: [dish],
      })
    }
  }

  return [...tables.values()]
}
