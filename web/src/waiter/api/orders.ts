import { queryOptions } from '@tanstack/react-query'

import { ApiCallError } from '@/shared/api/call-error'
import { api } from '@/shared/api/client'
import type { components } from '@/shared/api/schema'
import { floorKey, visitKey } from '@/shared/events/query-keys'

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

/** One table's whole meal. */
export type Visit = components['schemas']['VisitResponse']

/** One ticket with its dishes. */
export type Round = components['schemas']['OrderRoundDto']

/** One dish on a ticket. */
export type OrderLine = components['schemas']['OrderLineDto']

/** A bill, open or closed. */
export type Bill = components['schemas']['BillDto']

/** The floor, for the waiter's landing screen. */
export const floorQuery = queryOptions({
  queryKey: floorKey,
  queryFn: async (): Promise<Floor> => {
    const { data, error } = await api.GET('/api/floor')
    if (!data) throw new ApiCallError(error)
    return data
  },
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
