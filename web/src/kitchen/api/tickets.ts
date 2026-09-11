import { queryOptions } from '@tanstack/react-query'

import { ApiCallError } from '@/shared/api/call-error'
import { api } from '@/shared/api/client'
import type { components } from '@/shared/api/schema'
import { kitchenKey } from '@/shared/events/query-keys'

/** The kitchen queue, and the server's clock the ages are measured against. */
export type KitchenQueue = components['schemas']['KitchenResponse']

/** One ticket on the pass. */
export type KitchenTicket = components['schemas']['KitchenTicketDto']

/** One dish on a ticket. */
export type KitchenLine = components['schemas']['KitchenLineDto']

/**
 * Everything the kitchen still has work on, oldest first.
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
