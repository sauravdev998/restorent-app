import { queryOptions } from '@tanstack/react-query'

import { api } from '@/shared/api/client'
import type { components } from '@/shared/api/schema'
import { floorKey, menuKey, visitKey } from '@/shared/events/query-keys'

/**
 * Everything the waiter surface asks the API for.
 *
 * Query options rather than a hook each, so the floor screen and the table
 * screen hit the same cache entries and a live event invalidating one key
 * refetches whatever is on screen.
 *
 * The keys are not free form. Each starts with the entity kind the live stream
 * names, which is what lets `use-live-events.ts` invalidate exactly the queries
 * an event feeds instead of everything. See `shared/events/query-keys.ts`.
 */

/** Every live table and who is sitting at it. */
export type Floor = components['schemas']['FloorResponse']

/** One table on the floor. */
export type FloorTable = components['schemas']['FloorTableDto']

/** The live menu, grouped the way it is printed. */
export type Menu = components['schemas']['MenuResponse']

/** One item on the menu. */
export type MenuDish = components['schemas']['MenuDishDto']

/** One table's whole meal. */
export type Visit = components['schemas']['VisitResponse']

/** One ticket with its dishes. */
export type Round = components['schemas']['OrderRoundDto']

/** One dish on a ticket. */
export type OrderLine = components['schemas']['OrderLineDto']

/** A bill, open or closed. */
export type Bill = components['schemas']['BillDto']

/** What a failed request threw, so a screen can show the right sentence. */
export class ApiCallError extends Error {
  /** The failed response body, for `apiErrorMessage` to turn into words. */
  readonly body: unknown

  constructor(body: unknown) {
    super('The request was refused.')
    this.name = 'ApiCallError'
    this.body = body
  }
}

/** The floor, for the waiter's landing screen. */
export const floorQuery = queryOptions({
  queryKey: floorKey,
  queryFn: async (): Promise<Floor> => {
    const { data, error } = await api.GET('/api/floor')
    if (!data) throw new ApiCallError(error)
    return data
  },
})

/**
 * The menu, for the ordering screen.
 *
 * Kept a while, because a menu changes far less often than a floor does and a
 * waiter opens this screen at every table. A dish going unavailable arrives as
 * a live event and invalidates it, so the staleness never outlasts a change.
 */
export const menuQuery = queryOptions({
  queryKey: menuKey,
  queryFn: async (): Promise<Menu> => {
    const { data, error } = await api.GET('/api/menu')
    if (!data) throw new ApiCallError(error)
    return data
  },
  staleTime: 5 * 60_000,
})

/** One visit, with its rounds and its bill. */
export function visitQuery(visitId: string) {
  return queryOptions({
    queryKey: visitKey(visitId),
    queryFn: async (): Promise<Visit> => {
      const { data, error } = await api.GET('/api/visits/{id}', {
        params: { path: { id: visitId } },
      })
      if (!data) throw new ApiCallError(error)
      return data
    },
  })
}

/** Seats a party and opens their bill. */
export async function openVisit(tableId: string, guestCount?: number): Promise<string> {
  const { data, error } = await api.POST('/api/visits', {
    body: guestCount === undefined ? { tableId } : { tableId, guestCount },
  })

  if (!data) throw new ApiCallError(error)
  return data.visitId
}

/** One entry in the basket, before it is sent. */
export interface BasketLine {
  dishId: string
  quantity: number
}

/** Sends the basket to the kitchen as one ticket. */
export async function sendRound(visitId: string, lines: BasketLine[]): Promise<Round> {
  const { data, error } = await api.POST('/api/visits/{id}/rounds', {
    params: { path: { id: visitId } },
    body: { lines },
  })

  if (!data) throw new ApiCallError(error)
  return data
}

/** Marks a whole ticket as having reached the table. */
export async function markRoundServed(roundId: string): Promise<Round> {
  const { data, error } = await api.POST('/api/rounds/{id}/served', {
    params: { path: { id: roundId } },
  })

  if (!data) throw new ApiCallError(error)
  return data
}

/** Closes the bill and frees the table. */
export async function closeVisit(visitId: string): Promise<Bill> {
  const { data, error } = await api.POST('/api/visits/{id}/close', {
    params: { path: { id: visitId } },
  })

  if (!data) throw new ApiCallError(error)
  return data
}
