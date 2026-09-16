import { useQueryClient } from '@tanstack/react-query'
import { useState, type FormEvent } from 'react'
import { useTranslation } from 'react-i18next'
import { useNavigate } from 'react-router'

import { api } from '@/shared/api/client'
import { fieldErrorProps, fieldErrorsFrom, type FieldErrorCode } from '@/shared/api/field-errors'
import { identityQuery, rememberIdentity } from '@/shared/session/identity'
import { landingFor } from '@/shared/session/signed-out'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { Field } from '@/shared/ui/field'
import { Input } from '@/shared/ui/input'
import { SignedOutShell } from '@/shared/ui/signed-out-shell'

/** Where somebody who owes a password change is sent, from everywhere. */
export const CHOOSE_PASSWORD_PATH = '/choose-password'

/**
 * The screen a new waiter or chef meets before they meet the product.
 *
 * Somebody an admin created is signed in with a password their manager chose,
 * said out loud, and possibly wrote on a note. This is where that password is
 * spent: they give it as their current one, choose their own, and land on their
 * own surface. Every other endpoint refuses them until they do, so this is not
 * a nudge, it is the only door.
 *
 * **It renders outside the application shell**, like sign in and register, and
 * that is load bearing rather than tidy. The shell opens the live event stream,
 * and the API refuses that stream while a password is owed; rendering this
 * inside the shell would open a stream that is refused at once, and a
 * `ConnectionStatus` reading "closed" would be the first thing a new waiter
 * ever saw.
 *
 * **It is reached by redirect from the root loader**, not by a link, so no
 * other screen renders on the way here, not even for a frame.
 *
 * Nothing about it is a security control. The API is what refuses the endpoints
 * and refuses the stream, whatever the browser decides to draw.
 */
export function ChoosePassword() {
  const { t } = useTranslation()
  const queryClient = useQueryClient()
  const navigate = useNavigate()

  const [currentPassword, setCurrentPassword] = useState('')
  const [newPassword, setNewPassword] = useState('')
  const [fields, setFields] = useState<Record<string, FieldErrorCode>>({})
  const [unavailable, setUnavailable] = useState(false)
  const [saving, setSaving] = useState(false)

  async function submit(event: FormEvent<HTMLFormElement>): Promise<void> {
    event.preventDefault()

    setFields({})
    setUnavailable(false)
    setSaving(true)

    try {
      const { error, response } = await api.POST('/api/me/password', {
        body: { currentPassword, newPassword },
      })

      if (!response.ok) {
        const reported = fieldErrorsFrom(error)
        // A wrong current password comes back as
        // `fields.currentPassword=incorrect`, which belongs beside that box.
        // Anything else is the service being unwell, which is a different
        // sentence: telling somebody the password they were handed is wrong
        // when the database is down sends them back to their manager for
        // nothing.
        if (Object.keys(reported).length > 0) setFields(reported)
        else setUnavailable(true)
        return
      }

      // The flag is cleared server side by the write itself, so the bundle read
      // back here is the one that lets the root loader stop redirecting.
      const identity = await queryClient.fetchQuery({ ...identityQuery, staleTime: 0 })

      if (identity) {
        rememberIdentity(queryClient, identity)
        await navigate(landingFor(identity.staff.role), { replace: true })
      }
    } catch {
      setUnavailable(true)
    } finally {
      setSaving(false)
    }
  }

  return (
    <SignedOutShell>
      <Card className="w-full max-w-sm">
        <h1 className="text-xl font-semibold text-foreground">{t('choosePassword.title')}</h1>
        <p className="mt-1 text-sm text-muted-foreground">{t('choosePassword.subtitle')}</p>

        {unavailable && (
          <p role="alert" className="mt-4 text-sm font-medium text-status-late">
            {t('apiError.unavailable')}
          </p>
        )}

        <form className="mt-6 flex flex-col gap-4" onSubmit={(event) => void submit(event)}>
          <Field
            label={t('choosePassword.current')}
            hint={t('choosePassword.currentHint')}
            required
            {...fieldErrorProps(fields['currentPassword'], t)}
          >
            <Input
              type="password"
              name="currentPassword"
              autoComplete="current-password"
              value={currentPassword}
              onChange={(event) => {
                setCurrentPassword(event.target.value)
              }}
            />
          </Field>

          <Field
            label={t('choosePassword.new')}
            hint={t('register.passwordHint')}
            required
            {...fieldErrorProps(fields['newPassword'], t)}
          >
            <Input
              type="password"
              name="newPassword"
              autoComplete="new-password"
              value={newPassword}
              onChange={(event) => {
                setNewPassword(event.target.value)
              }}
            />
          </Field>

          <Button type="submit" disabled={saving} className="mt-2">
            {saving ? t('choosePassword.submitting') : t('choosePassword.submit')}
          </Button>
        </form>
      </Card>
    </SignedOutShell>
  )
}
