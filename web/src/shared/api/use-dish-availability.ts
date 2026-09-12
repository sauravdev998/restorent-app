import { useMutation, useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'

import { showToast } from '@/shared/ui/toast-store'

import { failureBody } from './call-error'
import { apiErrorMessage } from './error-message'
import { setDishAvailability } from './menu'

/** One switch being thrown. */
interface Throw {
  dishId: string
  available: boolean
  /** The dish's name, for the confirmation a person reads and hears. */
  name: string
}

/** What a screen with availability switches on it needs. */
export interface DishAvailability {
  /** What the switch shows: the value asked for while it is pending, else the stored one. */
  valueFor: (dishId: string, stored: boolean) => boolean
  /** Whether a switch for this dish is waiting on the server. */
  isPending: (dishId: string) => boolean
  /** Throws the switch. */
  set: (dishId: string, available: boolean, name: string) => void
}

/**
 * The availability switch's behaviour, shared by the admin's dish rows and the
 * chef's Menu tab.
 *
 * **The value asked for is held here while the request is pending, never in the
 * query cache.** A switch that does not move when it is tapped reads as broken,
 * so it shows what was asked for at once. But nothing is written into the
 * cache ahead of the server, which is spec 0007's rule: on success the menus are
 * invalidated and refetched, and only once that refetch has landed is the held
 * value dropped, so the switch never flicks back to the old value in between.
 * On failure the held value is dropped straight away, the menus are refetched,
 * and the switch shows where the dish really stands.
 *
 * Several switches can be in flight at once, one per dish, because a chef
 * running out of three things taps three switches without waiting.
 */
export function useDishAvailability(): DishAvailability {
  const queryClient = useQueryClient()
  const { t } = useTranslation()
  const [pending, setPending] = useState<Readonly<Record<string, boolean>>>({})

  const mutation = useMutation({
    mutationFn: ({ dishId, available }: Throw) => setDishAvailability(dishId, available),
    onMutate: ({ dishId, available }: Throw) => {
      setPending((current) => ({ ...current, [dishId]: available }))
    },
    onSuccess: async (_dish, { available, name }: Throw) => {
      // Awaited, so the held value outlives the refetch rather than the request.
      await queryClient.invalidateQueries({ queryKey: ['dish'] })
      showToast({
        title: t(available ? 'availability.backOn' : 'availability.switchedOff', { dish: name }),
      })
    },
    onError: async (error: unknown) => {
      showToast({ title: apiErrorMessage(failureBody(error), t), tone: 'late' })
      await queryClient.invalidateQueries({ queryKey: ['dish'] })
    },
    onSettled: (_dish, _error, { dishId }: Throw) => {
      setPending((current) => {
        const next = { ...current }
        delete next[dishId]
        return next
      })
    },
  })

  return {
    valueFor: (dishId, stored) => pending[dishId] ?? stored,
    isPending: (dishId) => dishId in pending,
    set: (dishId, available, name) => {
      mutation.mutate({ dishId, available, name })
    },
  }
}
