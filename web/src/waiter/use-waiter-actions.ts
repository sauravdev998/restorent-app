import { useMutation, useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'

import { failureBody } from '@/shared/api/call-error'
import { apiErrorMessage } from '@/shared/api/error-message'
import { showToast } from '@/shared/ui/toast-store'
import { markLineServed, takeOver } from '@/waiter/api/orders'

/**
 * The two actions both the Orders list and the table screen offer: carrying
 * one dish out, and taking a table over.
 *
 * One place, so both screens refuse the same way. A refusal (somebody served
 * it first, somebody took the table first) shows the translated reason and
 * refetches everything under `visit`, so the screen shows the truth rather
 * than the state the tap assumed (spec 0011, AC-17). Nothing is written into
 * the cache ahead of the server.
 */

/** Refetches every waiter read after a write or a refusal. */
function useRefreshWaiterReads() {
  const queryClient = useQueryClient()
  return () => queryClient.invalidateQueries({ queryKey: ['visit'] })
}

/** Serving one dish, with the dish currently being served. */
export function useServeLine() {
  const { t: common } = useTranslation()
  const refresh = useRefreshWaiterReads()
  const [serving, setServing] = useState<string | null>(null)

  const mutation = useMutation({
    mutationFn: (lineId: string) => markLineServed(lineId),
    onSuccess: async () => {
      await refresh()
    },
    onError: (error: unknown) => {
      void refresh()
      showToast({ title: apiErrorMessage(failureBody(error), common), tone: 'late' })
    },
    onSettled: () => {
      setServing(null)
    },
  })

  return {
    serving,
    serve: (lineId: string) => {
      setServing(lineId)
      mutation.mutate(lineId)
    },
  }
}

/** Taking a table over from the waiter the screen showed as responsible. */
export function useTakeOver() {
  const { t } = useTranslation('waiter')
  const { t: common } = useTranslation()
  const refresh = useRefreshWaiterReads()
  const [taking, setTaking] = useState<string | null>(null)

  const mutation = useMutation({
    mutationFn: ({ visitId, expected }: { visitId: string; expected: string }) =>
      takeOver(visitId, expected),
    onSuccess: async () => {
      showToast({ title: t('takeOver.done'), tone: 'ready' })
      await refresh()
    },
    onError: (error: unknown) => {
      void refresh()
      showToast({ title: apiErrorMessage(failureBody(error), common), tone: 'late' })
    },
    onSettled: () => {
      setTaking(null)
    },
  })

  return {
    taking,
    takeOver: (visitId: string, expected: string) => {
      setTaking(visitId)
      mutation.mutate({ visitId, expected })
    },
  }
}
