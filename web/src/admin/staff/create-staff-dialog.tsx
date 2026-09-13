import { useMutation, useQueryClient } from '@tanstack/react-query'
import { Eye, EyeOff } from 'lucide-react'
import { useId, useState, type FormEvent } from 'react'
import { useTranslation } from 'react-i18next'

import {
  createStaff,
  STAFF_ROLES,
  suggestPassword,
  type NewStaffForm,
  type StaffRole,
} from '@/admin/api/staff'
import { failureBody } from '@/shared/api/call-error'
import { apiErrorMessage } from '@/shared/api/error-message'
import { fieldErrorProps, fieldErrorsFrom, type FieldErrorCode } from '@/shared/api/field-errors'
import { staffKey } from '@/shared/events/query-keys'
import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { Field } from '@/shared/ui/field'
import { Icon } from '@/shared/ui/icon'
import { Input } from '@/shared/ui/input'
import { Select } from '@/shared/ui/select'
import { showToast } from '@/shared/ui/toast-store'

export interface CreateStaffDialogProps {
  onOpenChange: (open: boolean) => void
}

/** What the dialog shows once the account exists. */
interface HandOver {
  displayName: string
  email: string
  password: string
}

/**
 * Adds somebody to the restaurant, then shows what to hand them, once.
 *
 * Two states in one dialog rather than two dialogs, because they are one moment
 * for the admin: a new waiter is standing next to them and will be taking
 * orders in a minute. Closing the form and opening a panel would put a frame
 * between typing the password and reading it out.
 *
 * **The password is shown exactly once, and only here.** It is never stored on
 * this side, never returned by the API, and never recoverable: the row keeps
 * only its `argon2id` hash. If the admin closes this panel before reading it
 * out, the way back is a password reset, which is two taps and writes a new
 * one.
 *
 * The suggested password is generated in this browser, and the admin may
 * overwrite it with anything that passes the rules. The reveal control is not a
 * nicety either: an admin is about to say this out loud, so they have to be
 * able to see what they are saying.
 */
export function CreateStaffDialog({ onOpenChange }: CreateStaffDialogProps) {
  const { t } = useTranslation('admin')
  const { t: common } = useTranslation()
  const queryClient = useQueryClient()
  const formId = useId()

  const [displayName, setDisplayName] = useState('')
  const [email, setEmail] = useState('')
  const [password, setPassword] = useState('')
  const [role, setRole] = useState<StaffRole>('waiter')
  const [revealed, setRevealed] = useState(false)
  const [fields, setFields] = useState<Record<string, FieldErrorCode>>({})
  const [problem, setProblem] = useState<string | null>(null)
  const [handOver, setHandOver] = useState<HandOver | null>(null)

  const save = useMutation({
    mutationFn: (form: NewStaffForm) => createStaff(form),
    onSuccess: async (created) => {
      await queryClient.invalidateQueries({ queryKey: staffKey })
      // From the form, not from the response. The API returns neither the
      // password nor its hash, which is exactly as it should be.
      setHandOver({ displayName: created.displayName, email: created.email, password })
    },
    onError: (error: unknown) => {
      const body = failureBody(error)
      const reported = fieldErrorsFrom(body)

      if (Object.keys(reported).length > 0) {
        setFields(reported)
        setProblem(null)
        return
      }

      setFields({})
      setProblem(apiErrorMessage(body, common))
    },
  })

  function submit(event: FormEvent<HTMLFormElement>): void {
    event.preventDefault()
    setFields({})
    setProblem(null)
    save.mutate({ displayName, email, password, role })
  }

  if (handOver) {
    return (
      <Dialog
        open
        onOpenChange={onOpenChange}
        title={t('staff.create.doneTitle', { name: handOver.displayName })}
        description={t('staff.create.doneBody')}
        footer={
          <Button
            onClick={() => {
              showToast({ title: t('staff.create.added', { name: handOver.displayName }) })
              onOpenChange(false)
            }}
          >
            {t('staff.create.acknowledge')}
          </Button>
        }
      >
        <div className="flex flex-col gap-4">
          {/* A description list rather than two paragraphs: the pairing of a
              label with the value beside it is the whole content here, and a
              screen reader reads a list of pairs as pairs. */}
          <dl className="border-line grid gap-3 rounded-md border-border bg-muted p-4">
            <div className="flex flex-col gap-1">
              <dt className="text-xs font-medium text-muted-foreground">
                {t('staff.create.emailLabel')}
              </dt>
              <dd className="font-mono text-sm break-all text-foreground">{handOver.email}</dd>
            </div>
            <div className="flex flex-col gap-1">
              <dt className="text-xs font-medium text-muted-foreground">
                {t('staff.create.passwordLabel')}
              </dt>
              <dd className="font-mono text-sm break-all text-foreground">{handOver.password}</dd>
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
      title={t('staff.create.title')}
      description={t('staff.create.body')}
      footer={
        <>
          <Button type="submit" form={formId} disabled={save.isPending}>
            {save.isPending ? t('staff.saving') : t('staff.create.submit')}
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
          label={t('staff.name')}
          hint={t('staff.nameHint')}
          required
          {...fieldErrorProps(fields['displayName'], common)}
        >
          <Input
            name="displayName"
            autoComplete="off"
            value={displayName}
            onChange={(event) => {
              setDisplayName(event.target.value)
            }}
          />
        </Field>

        <Field
          label={t('staff.email')}
          hint={t('staff.emailHint')}
          required
          {...fieldErrorProps(fields['email'], common)}
        >
          <Input
            type="email"
            name="email"
            autoComplete="off"
            autoCapitalize="none"
            autoCorrect="off"
            spellCheck={false}
            value={email}
            onChange={(event) => {
              setEmail(event.target.value)
            }}
          />
        </Field>

        <Field
          label={t('staff.create.password')}
          hint={t('staff.create.passwordHint')}
          required
          {...fieldErrorProps(fields['password'], common)}
        >
          <div className="flex gap-2">
            <Input
              // Not `type="password"` by default and then revealed: this is
              // somebody else's password, about to be said out loud, and the
              // admin typing it is not signing in with it. `autoComplete="off"`
              // keeps a browser from offering to save it as the admin's own.
              type={revealed ? 'text' : 'password'}
              name="newStaffPassword"
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
            // Suggesting one and hiding it would be suggesting something the
            // admin cannot read out.
            setRevealed(true)
          }}
        >
          {t('staff.create.suggest')}
        </Button>

        <Field label={t('staff.role')} hint={t('staff.roleHint')} required>
          <Select
            name="role"
            value={role}
            onChange={(event) => {
              setRole(event.target.value as StaffRole)
            }}
          >
            {STAFF_ROLES.map((each) => (
              <option key={each} value={each}>
                {t(`staff.roles.${each}`)}
              </option>
            ))}
          </Select>
        </Field>
      </form>
    </Dialog>
  )
}
