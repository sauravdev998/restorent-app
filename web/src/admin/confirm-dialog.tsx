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
  /**
   * The query key prefix to read again, whether the write worked or was
   * refused.
   *
   * Both cases, deliberately. On success the screen behind the dialog has to
   * show the new state; on a refusal it has to show the state that caused the
   * refusal, which is usually not the one the admin was looking at.
   */
  invalidateKey: readonly string[]
}

/**
 * Asks before something that takes effect at once, and does it only on a yes.
 *
 * Used by both admin screens, for the same kind of moment: an action that is
 * recoverable but that changes what other people see immediately. A removed
 * dish lands in the Archived section and comes back in two taps; a switched off
 * account keeps its row and comes back from the section below the list. Both
 * are still asked, because both reach somebody else's screen the instant they
 * land, and one of them ends a shift.
 *
 * That is also why this is one plain question rather than typing a name to
 * confirm: the weight of the action is in how quickly it takes effect, not in
 * how hard it is to undo.
 *
 * A refusal, such as a category that still holds a dish or the last admin who
 * cannot be switched off, is said inside the dialog where the admin is looking,
 * and the list behind it is read again so the screen is true.
 */
export function ConfirmDialog({
  onOpenChange,
  title,
  description,
  confirmLabel,
  doneMessage,
  action,
  invalidateKey,
}: ConfirmDialogProps) {
  const { t } = useTranslation('admin')
  const { t: common } = useTranslation()
  const queryClient = useQueryClient()
  const [problem, setProblem] = useState<string | null>(null)

  const confirm = useMutation({
    mutationFn: action,
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: invalidateKey })
      showToast({ title: doneMessage })
      onOpenChange(false)
    },
    onError: async (error: unknown) => {
      setProblem(apiErrorMessage(failureBody(error), common))
      await queryClient.invalidateQueries({ queryKey: invalidateKey })
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
