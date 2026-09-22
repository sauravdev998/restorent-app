import { useMutation, useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'

import { failureBody } from '@/shared/api/call-error'
import { apiErrorMessage } from '@/shared/api/error-message'
import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { Field } from '@/shared/ui/field'
import { Input } from '@/shared/ui/input'
import { Select } from '@/shared/ui/select'
import { showToast } from '@/shared/ui/toast-store'
import {
  VOID_REASON_MAX_CHARS,
  VOID_REASONS,
  voidLine,
  type OrderLine,
  type VoidReason,
} from '@/waiter/api/orders'

export interface VoidDialogProps {
  /** The dish being cancelled, or `null` when the dialog is shut. */
  line: OrderLine | null
  onClose: () => void
}

/**
 * Cancelling one dish, with a reason (spec 0011, AC-13).
 *
 * One of four reasons, and words beside it: required for "other", because
 * "other" alone tells a manager reading the log nothing, and optional for the
 * rest. The server takes the dish off the bill's running subtotal, tells the
 * kitchen, and writes the audit row, all in one step, so all this dialog does
 * is ask and wait for the answer.
 *
 * A dish somebody served or cancelled a moment ago is refused with
 * `line_not_voidable`, and the table is refetched so the screen shows why.
 */
export function VoidDialog({ line, onClose }: VoidDialogProps) {
  const { t } = useTranslation('waiter')
  const { t: common } = useTranslation()
  const queryClient = useQueryClient()

  const [reasonCode, setReasonCode] = useState<VoidReason>('guest_changed_mind')
  const [reason, setReason] = useState('')
  const [tried, setTried] = useState(false)

  const words = [...reason.trim()].length
  const problem =
    reasonCode === 'other' && words === 0
      ? t('void.reasonRequired')
      : words > VOID_REASON_MAX_CHARS
        ? t('void.reasonTooLong', { max: VOID_REASON_MAX_CHARS })
        : undefined

  const cancel = useMutation({
    mutationFn: (request: { lineId: string; reasonCode: VoidReason; reason: string }) =>
      voidLine(request.lineId, request.reasonCode, request.reason),
    onSuccess: async () => {
      showToast({ title: t('void.done'), tone: 'voided' })
      close()
      await queryClient.invalidateQueries({ queryKey: ['visit'] })
    },
    onError: (error: unknown) => {
      void queryClient.invalidateQueries({ queryKey: ['visit'] })
      showToast({ title: apiErrorMessage(failureBody(error), common), tone: 'late' })
      close()
    },
  })

  function close(): void {
    setReasonCode('guest_changed_mind')
    setReason('')
    setTried(false)
    onClose()
  }

  function submit(): void {
    setTried(true)
    if (!line || problem !== undefined) return
    cancel.mutate({ lineId: line.id, reasonCode, reason })
  }

  return (
    <Dialog
      open={line !== null}
      onOpenChange={(open) => {
        if (!open) close()
      }}
      title={t('void.title')}
      {...(line
        ? { description: t('void.description', { quantity: line.quantity, dish: line.dishName }) }
        : {})}
      footer={
        <>
          <Button variant="secondary" onClick={close}>
            {t('void.keep')}
          </Button>
          <Button variant="destructive" disabled={cancel.isPending} onClick={submit}>
            {cancel.isPending ? t('void.cancelling') : t('void.confirm')}
          </Button>
        </>
      }
    >
      <form
        className="space-y-4"
        onSubmit={(event) => {
          event.preventDefault()
          submit()
        }}
      >
        <Field label={t('void.reasonLabel')}>
          <Select
            value={reasonCode}
            onChange={(event) => {
              const picked = VOID_REASONS.find((code) => code === event.target.value)
              if (picked) setReasonCode(picked)
            }}
          >
            {VOID_REASONS.map((code) => (
              <option key={code} value={code}>
                {common(`voidReason.${code}`)}
              </option>
            ))}
          </Select>
        </Field>

        <Field
          label={t('void.detailsLabel')}
          hint={
            reasonCode === 'other' ? t('void.detailsRequiredHint') : t('void.detailsOptionalHint')
          }
          required={reasonCode === 'other'}
          {...(tried && problem !== undefined ? { error: problem } : {})}
        >
          <Input
            value={reason}
            onChange={(event) => {
              setReason(event.target.value)
            }}
            autoComplete="off"
          />
        </Field>
      </form>
    </Dialog>
  )
}
