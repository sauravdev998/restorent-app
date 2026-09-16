import { useMutation, useQueryClient } from '@tanstack/react-query'
import { Eye, EyeOff } from 'lucide-react'
import { useId, useState, type FormEvent } from 'react'
import { useTranslation } from 'react-i18next'

import { resetStaffPassword, suggestPassword, type StaffMember } from '@/admin/api/staff'
import { failureBody } from '@/shared/api/call-error'
import { apiErrorMessage } from '@/shared/api/error-message'
import { fieldErrorProps, fieldErrorsFrom, type FieldErrorCode } from '@/shared/api/field-errors'
import { staffKey } from '@/shared/events/query-keys'
import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { Field } from '@/shared/ui/field'
import { Icon } from '@/shared/ui/icon'
import { Input } from '@/shared/ui/input'
import { showToast } from '@/shared/ui/toast-store'

export interface ResetPasswordDialogProps {
  onOpenChange: (open: boolean) => void
  /** Whose password is being written. */
  member: StaffMember
}

/**
 * Writes a new password onto somebody's row, and shows it once.
 *
 * The same two states the create dialog has, for the same reason and with the
 * same rule: the password is shown once, exists nowhere else, and cannot be
 * recovered. A chef who forgot theirs is standing at the pass, so this is the
 * fastest thing in the feature.
 *
 * It signs them out of every device, which the dialog says first. That is the
 * point rather than a side effect: whoever might have had the old password
 * loses whatever they had open, at the same instant.
 */
export function ResetPasswordDialog({ onOpenChange, member }: ResetPasswordDialogProps) {
  const { t } = useTranslation('admin')
  const { t: common } = useTranslation()
  const queryClient = useQueryClient()
  const formId = useId()

  const [password, setPassword] = useState('')
  const [revealed, setRevealed] = useState(false)
  const [fields, setFields] = useState<Record<string, FieldErrorCode>>({})
  const [problem, setProblem] = useState<string | null>(null)
  const [handedOver, setHandedOver] = useState<string | null>(null)

  const save = useMutation({
    // The password travels as the mutation's variable, never through the
    // closure: a pending mutation takes the callbacks of the latest render, so
    // an edit made while the request is in flight would otherwise be what the
    // panel shows, and not what was written.
    mutationFn: (submitted: string) => resetStaffPassword(member.id, submitted),
    onSuccess: async (_, submitted) => {
      await queryClient.invalidateQueries({ queryKey: staffKey })
      setHandedOver(submitted)
    },
    onError: async (error: unknown) => {
      const body = failureBody(error)
      const reported = fieldErrorsFrom(body)

      if (Object.keys(reported).length > 0) {
        setFields(reported)
        setProblem(null)
        return
      }

      setFields({})
      setProblem(apiErrorMessage(body, common))
      await queryClient.invalidateQueries({ queryKey: staffKey })
    },
  })

  function submit(event: FormEvent<HTMLFormElement>): void {
    event.preventDefault()
    setFields({})
    setProblem(null)
    save.mutate(password)
  }

  if (handedOver !== null) {
    return (
      <Dialog
        open
        onOpenChange={onOpenChange}
        title={t('staff.reset.doneTitle', { name: member.displayName })}
        description={t('staff.reset.doneBody')}
        footer={
          <Button
            onClick={() => {
              showToast({ title: t('staff.reset.done', { name: member.displayName }) })
              onOpenChange(false)
            }}
          >
            {t('staff.create.acknowledge')}
          </Button>
        }
      >
        <div className="flex flex-col gap-4">
          <dl className="border-line grid gap-3 rounded-md border-border bg-muted p-4">
            <div className="flex flex-col gap-1">
              <dt className="text-xs font-medium text-muted-foreground">
                {t('staff.create.emailLabel')}
              </dt>
              <dd className="font-mono text-sm break-all text-foreground">{member.email}</dd>
            </div>
            <div className="flex flex-col gap-1">
              <dt className="text-xs font-medium text-muted-foreground">
                {t('staff.create.passwordLabel')}
              </dt>
              <dd className="font-mono text-sm break-all text-foreground">{handedOver}</dd>
            </div>
          </dl>

          <p className="text-sm text-muted-foreground">{t('staff.create.onceOnly')}</p>
        </div>
      </Dialog>
    )
  }

  return (
    <Dialog
      open
      onOpenChange={onOpenChange}
      title={t('staff.reset.title', { name: member.displayName })}
      description={t('staff.reset.body')}
      footer={
        <>
          <Button type="submit" form={formId} disabled={save.isPending}>
            {save.isPending ? t('staff.saving') : t('staff.reset.submit')}
          </Button>
          <Button
            variant="secondary"
            onClick={() => {
              onOpenChange(false)
            }}
          >
            {t('staff.cancel')}
          </Button>
        </>
      }
    >
      <form id={formId} className="flex flex-col gap-4" onSubmit={submit} noValidate>
        {problem !== null && (
          <p role="alert" className="text-sm font-medium text-status-late">
            {problem}
          </p>
        )}

        <Field
          label={t('staff.create.password')}
          hint={t('staff.create.passwordHint')}
          required
          {...fieldErrorProps(fields['password'], common)}
        >
          <div className="flex gap-2">
            <Input
              type={revealed ? 'text' : 'password'}
              name="resetPassword"
              autoComplete="off"
              className="font-mono"
              value={password}
              onChange={(event) => {
                setPassword(event.target.value)
              }}
            />
            <Button
              type="button"
              variant="secondary"
              size="icon"
              aria-pressed={revealed}
              aria-label={revealed ? t('staff.create.hide') : t('staff.create.reveal')}
              onClick={() => {
                setRevealed((shown) => !shown)
              }}
            >
              <Icon icon={revealed ? EyeOff : Eye} />
            </Button>
          </div>
        </Field>

        <Button
          type="button"
          variant="secondary"
          size="sm"
          className="self-start"
          onClick={() => {
            setPassword(suggestPassword())
            setRevealed(true)
          }}
        >
          {t('staff.create.suggest')}
        </Button>
      </form>
    </Dialog>
  )
}
