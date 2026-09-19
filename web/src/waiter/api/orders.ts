import { queryOptions } from '@tanstack/react-query'

import { ApiCallError } from '@/shared/api/call-error'
import { api } from '@/shared/api/client'
import type { components } from '@/shared/api/schema'
import { floorKey, openOrdersKey, visitKey } from '@/shared/events/query-keys'
import type { SendLine } from '@/waiter/basket'

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

/** Every open table, with every round on it. */
export type OpenOrders = components['schemas']['OpenOrdersResponse']

/** One open table on the Orders list. */
export type OpenOrder = components['schemas']['OpenOrderDto']

/** Why a dish was cancelled. */
export type VoidReason = components['schemas']['VoidReasonDto']

/** The four reasons, in the order the void dialog offers them. */
export const VOID_REASONS: readonly VoidReason[] = [
  'guest_changed_mind',
  'entered_by_mistake',
  'kitchen_unavailable',
  'other',
]

/** The most characters a void explanation may hold, counted as the server counts them. */
export const VOID_REASON_MAX_CHARS = 200

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
 * Every open table and every round on it, for the Orders list and for the
 * ready alert the waiter shell holds. One cache entry for both, so the alert
 * and the list never disagree about what is ready.
 */
export const openOrdersQuery = queryOptions({
  queryKey: openOrdersKey,
  queryFn: async (): Promise<OpenOrders> => {
    const { data, error } = await api.GET('/api/orders/open')
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

/**
 * Sends the basket to the kitchen as one ticket.
 *
 * `clientKey` is the basket's own key. Sending the same key again after a
 * timeout answers with the ticket the first send made, and makes no second
 * one, so a retry is always safe.
 */
export async function sendRound(
  visitId: string,
  clientKey: string,
  lines: SendLine[],
): Promise<Round> {
  const { data, error } = await api.POST('/api/visits/{id}/rounds', {
    params: { path: { id: visitId } },
    body: { clientKey, lines },
  })

  if (!data) throw new ApiCallError(error)
  return data
}

/** Marks one dish as having reached the table. */
export async function markLineServed(lineId: string): Promise<void> {
  const { data, error } = await api.POST('/api/order-lines/{id}/served', {
    params: { path: { id: lineId } },
  })

  if (!data) throw new ApiCallError(error)
}

/** Cancels one dish, with a reason, and takes it off the bill. */
export async function voidLine(
  lineId: string,
  reasonCode: VoidReason,
  reason: string,
): Promise<void> {
  const trimmed = reason.trim()
  const { data, error } = await api.POST('/api/order-lines/{id}/void', {
    params: { path: { id: lineId } },
    body: trimmed === '' ? { reasonCode } : { reasonCode, reason: trimmed },
  })

  if (!data) throw new ApiCallError(error)
}

/**
 * Makes the signed in waiter responsible for a table, taking it from the
 * waiter the screen showed. Refused with `table_taken_over` if somebody else
 * took it first.
 */
export async function takeOver(visitId: string, expectedStaffId: string): Promise<void> {
  const { data, error } = await api.POST('/api/visits/{id}/take-over', {
    params: { path: { id: visitId } },
    body: { expectedStaffId },
  })

  if (!data) throw new ApiCallError(error)
}

/** Moves a party, with everything on their table, to a free table. */
export async function moveVisit(visitId: string, tableId: string): Promise<void> {
  const { data, error } = await api.POST('/api/visits/{id}/move', {
    params: { path: { id: visitId } },
    body: { tableId },
  })

  if (!data) throw new ApiCallError(error)
}

/** Serves every dish on a ticket that is ready, leaving the ones still cooking. */
export async function markRoundServed(roundId: string): Promise<Round> {
  const { data, error } = await api.POST('/api/rounds/{id}/served', {
    params: { path: { id: roundId } },
  })

  if (!data) throw new ApiCallError(error)
  return data
}

/**
 * Closes the bill and frees the table. A bill with nothing chargeable on it
 * comes back `voided`, with no number: there was nothing to charge.
 */
export async function closeVisit(visitId: string): Promise<Bill> {
  const { data, error } = await api.POST('/api/visits/{id}/close', {
    params: { path: { id: visitId } },
  })

  if (!data) throw new ApiCallError(error)
  return data
}
