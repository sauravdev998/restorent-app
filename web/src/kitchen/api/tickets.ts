import { queryOptions } from '@tanstack/react-query'

import { ApiCallError } from '@/shared/api/call-error'
import { api } from '@/shared/api/client'
import type { components } from '@/shared/api/schema'
import { kitchenKey } from '@/shared/events/query-keys'

/** The kitchen queue, its thresholds, and the server's clock the ages are measured against. */
export type KitchenQueue = components['schemas']['KitchenResponse']

/** One ticket on the pass. */
export type KitchenTicket = components['schemas']['KitchenTicketDto']

/** One dish on a ticket. */
export type KitchenLine = components['schemas']['KitchenLineDto']

/**
 * Everything the kitchen still has work on: cooking oldest first, then plated
 * oldest first.
 *
 * No polling interval. The stream tells this screen when a ticket arrives or a
 * dish moves, and a kitchen screen polling every few seconds all evening is a
 * request per second per screen for an answer that has usually not changed.
 */
export const kitchenQuery = queryOptions({
  queryKey: kitchenKey,
  queryFn: async (): Promise<KitchenQueue> => {
    const { data, error } = await api.GET('/api/kitchen/tickets')
    if (!data) throw new ApiCallError(error)
    return data
  },
})

/** Marks one dish off the pass. Touches that dish and nothing else. */
export async function markLineReady(lineId: string): Promise<void> {
  const { data, error } = await api.POST('/api/order-lines/{id}/ready', {
    params: { path: { id: lineId } },
  })

  if (!data) throw new ApiCallError(error)
}

/**
 * Puts one plated dish back on the stove, undoing a tap.
 *
 * A ticket that had gone ready comes back to cooking with it, decided by the
 * server from the dish, which is why nothing here says anything about the ticket.
 */
export async function unmarkLineReady(lineId: string): Promise<void> {
  const { data, error } = await api.POST('/api/order-lines/{id}/unready', {
    params: { path: { id: lineId } },
  })

  if (!data) throw new ApiCallError(error)
}

/**
 * Marks every dish still cooking on one ticket off the pass, in one request.
 *
 * One transaction on the server, so the ticket either fully flips or does not
 * change at all. Never four requests from here: four requests is how a chef ends
 * up with two dishes marked and two not because the tablet lost the network in
 * the middle.
 */
export async function markRoundReady(roundId: string): Promise<void> {
  const { data, error } = await api.POST('/api/rounds/{id}/ready', {
    params: { path: { id: roundId } },
  })

  if (!data) throw new ApiCallError(error)
}

/**
 * Takes one dish off a ticket because the kitchen has run out of it.
 *
 * The one reason a chef may give, and the server checks it against the session's
 * role rather than trusting this call. The bill recomputes and every waiter screen
 * updates, both on the server's side of the line.
 */
export async function voidLineRanOut(lineId: string): Promise<void> {
  const { data, error } = await api.POST('/api/order-lines/{id}/void', {
    params: { path: { id: lineId } },
    body: { reasonCode: 'kitchen_unavailable' },
  })

  if (!data) throw new ApiCallError(error)
}
