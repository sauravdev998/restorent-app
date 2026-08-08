import { queryOptions } from '@tanstack/react-query'

import { api } from './client'
import type { components } from './schema'

export type HealthResponse = components['schemas']['HealthResponse']

/**
 * Whether the API instance serving this browser can actually serve.
 *
 * Shared query options rather than a hook, so any screen can use it and they
 * all hit one cache entry.
 */
export const healthQuery = queryOptions({
  queryKey: ['system', 'health'] as const,
  queryFn: async (): Promise<HealthResponse> => {
    const { data, error, response } = await api.GET('/api/health')

    // A 503 is a real answer here, not a transport failure: it means the
    // instance is up but a dependency is down, and the body says which one.
    if (data) return data

    throw new Error(
      error ? `The API is unhealthy (${response.status}).` : `Could not reach the API.`,
    )
  },
  // The kitchen cares whether the pipe is alive, so this is worth rechecking
  // rather than trusting a cached answer for a whole service.
  refetchInterval: 30_000,
})
