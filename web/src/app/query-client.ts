import { QueryClient } from '@tanstack/react-query'

/**
 * The shared cache.
 *
 * Kept deliberately quiet on automatic refetching, because live updates arrive
 * over the event stream instead. Refetching on every window focus as well would
 * mean a busy service hammering the API for data it already has.
 */
export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 30_000,
      refetchOnWindowFocus: false,
      retry: 2,
    },
  },
})
