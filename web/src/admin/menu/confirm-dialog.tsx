import { useMutation, useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'

import { failureBody } from '@/shared/api/call-error'
import { apiErrorMessage } from '@/shared/api/error-message'
import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { showToast } from '@/shared/ui/toast-store'

export interface ConfirmDialogProps {
  onOpenChange: (open: boolean) => void
  title: string
  description: string
  /** The words on the button that does it, such as "Remove dish". */
  confirmLabel: string
  /** What is said once it is done. */
  doneMessage: string
  /** The write. Rejects when the server refuses. */
  action: () => Promise<unknown>
}

/**
 * Asks before a removal, and does it only once the admin says yes.
 *
 * A removal is recoverable, which is why this is one plain question rather
 * than typing a name to confirm: the removed dish or category lands in the
 * Archived section and comes back in two taps. It is still asked, because a
 * removal takes the item off every waiter's screen at once.
 *
 * A refusal, such as a category that still holds a dish, is said inside the
 * dialog where the admin is looking, and the menus are read again so the
 * screen behind it is true.
 */
export function ConfirmDialog({
  onOpenChange,
  title,
  description,
  confirmLabel,
  doneMessage,
  action,
}: ConfirmDialogProps) {
  const { t } = useTranslation('admin')
  const { t: common } = useTranslation()
  const queryClient = useQueryClient()
  const [problem, setProblem] = useState<string | null>(null)

  const confirm = useMutation({
    mutationFn: action,
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ['dish'] })
      showToast({ title: doneMessage })
      onOpenChange(false)
    },
    onError: async (error: unknown) => {
      setProblem(apiErrorMessage(failureBody(error), common))
      await queryClient.invalidateQueries({ queryKey: ['dish'] })
    },
  })

  return (
    <Dialog
      open
      onOpenChange={onOpenChange}
      title={title}
      description={description}
      footer={
        <>
          <Button
            variant="destructive"
            disabled={confirm.isPending}
            onClick={() => {
              setProblem(null)
              confirm.mutate()
            }}
          >
            {confirm.isPending ? t('menu.working') : confirmLabel}
          </Button>
          <Button
            variant="secondary"
            onClick={() => {
              onOpenChange(false)
            }}
          >
            {t('menu.cancel')}
          </Button>
        </>
      }
      // Only when there is something to say, so an empty body does not leave
      // a gap above the buttons.
      {...(problem === null
        ? {}
        : {
            children: (
              <p role="alert" className="text-sm font-medium text-status-late">
                {problem}
              </p>
            ),
          })}
    />
  )
}
