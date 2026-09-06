import { useQueryClient } from '@tanstack/react-query'
import { useState, type FormEvent } from 'react'
import { useTranslation } from 'react-i18next'
import { useNavigate } from 'react-router'

import { api } from '@/shared/api/client'
import { fieldErrorProps, fieldErrorsFrom, type FieldErrorCode } from '@/shared/api/field-errors'
import { catalogue } from '@/shared/i18n/catalogue'
import { forgetIdentity, rememberIdentity, type Identity } from '@/shared/session/identity'
import { useIdentity } from '@/shared/session/use-identity'
import { SIGN_IN_PATH } from '@/shared/session/signed-out'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { Field } from '@/shared/ui/field'
import { Input } from '@/shared/ui/input'
import { Select } from '@/shared/ui/select'
import { showToast } from '@/shared/ui/toast-store'

/**
 * Somebody's own account: their name, their language, their password, and the
 * way out.
 *
 * Reachable from all three shells, because a chef changes their password on the
 * same screen an owner does. Nothing on it can reach anybody else's row: the
 * two endpoints behind it write the caller's own `staff` row and nothing else,
 * whatever this screen sends.
 *
 * The personal language lives here rather than in the header switcher, and the
 * two do different things on purpose. The switcher changes what is on screen
 * now, on this device. This stores a preference that follows the person to
 * every device they sign in on.
 */
export function Account() {
  const { t } = useTranslation()
  const queryClient = useQueryClient()
  const navigate = useNavigate()
  const identity = useIdentity()

  return (
    <div className="space-y-8">
      <section aria-labelledby="account-heading">
        <h1 id="account-heading" className="text-2xl font-semibold text-foreground">
          {t('account.title')}
        </h1>
        <p className="mt-1 text-sm text-muted-foreground">{identity.staff.email}</p>
      </section>

      <Profile identity={identity} />
      <ChangePassword />

      <section aria-labelledby="signout-heading">
        <h2 id="signout-heading" className="text-xl font-semibold text-foreground">
          {t('account.signOutTitle')}
        </h2>
        <p className="mt-1 text-sm text-muted-foreground">{t('account.signOutHint')}</p>
        <Button
          variant="secondary"
          className="mt-4"
          onClick={() => {
            void (async () => {
              // The answer does not matter. A session that is already gone is
              // gone, and leaving somebody on a signed in screen because the
              // sign out request failed is the one outcome nobody wants.
              await api.POST('/api/auth/sign-out').catch(() => undefined)
              forgetIdentity(queryClient)
              await navigate(SIGN_IN_PATH, { replace: true })
            })()
          }}
        >
          {t('account.signOut')}
        </Button>
      </section>
    </div>
  )
}

/** The person's own display name and interface language. */
function Profile({ identity }: { identity: Identity }) {
  const { t } = useTranslation()
  const queryClient = useQueryClient()

  const [displayName, setDisplayName] = useState(identity.staff.displayName)
  // The empty string is "follow the restaurant", which is a real choice
  // somebody makes rather than an absence, so it is an option in the list.
  const [language, setLanguage] = useState(identity.staff.language ?? '')
  const [fields, setFields] = useState<Record<string, FieldErrorCode>>({})
  const [saving, setSaving] = useState(false)

  async function submit(event: FormEvent<HTMLFormElement>): Promise<void> {
    event.preventDefault()
    setFields({})
    setSaving(true)

    try {
      const { data, error } = await api.PATCH('/api/me', {
        body: { displayName, language: language === '' ? null : language },
      })

      if (!data) {
        const reported = fieldErrorsFrom(error)
        if (Object.keys(reported).length > 0) setFields(reported)
        else showToast({ title: t('account.saveFailed'), tone: 'late' })
        return
      }

      rememberIdentity(queryClient, data)
      showToast({ title: t('account.saved') })
    } catch {
      showToast({ title: t('account.saveFailed'), tone: 'late' })
    } finally {
      setSaving(false)
    }
  }

  return (
    <section aria-labelledby="profile-heading">
      <h2 id="profile-heading" className="text-xl font-semibold text-foreground">
        {t('account.profileTitle')}
      </h2>

      <Card className="mt-4">
        <form className="flex flex-col gap-4" onSubmit={(event) => void submit(event)}>
          <Field
            label={t('account.displayName')}
            required
            {...fieldErrorProps(fields['displayName'], t)}
          >
            <Input
              name="displayName"
              autoComplete="name"
              value={displayName}
              onChange={(event) => {
                setDisplayName(event.target.value)
              }}
            />
          </Field>

          <Field
            label={t('account.language')}
            hint={t('account.languageHint')}
            {...fieldErrorProps(fields['language'], t)}
          >
            <Select
              name="language"
              value={language}
              onChange={(event) => {
                setLanguage(event.target.value)
              }}
            >
              <option value="">{t('account.followRestaurant')}</option>
              {catalogue.languages.map((entry) => (
                <option key={entry.code} value={entry.code} lang={entry.code}>
                  {entry.nativeName}
                </option>
              ))}
            </Select>
          </Field>

          <Button type="submit" disabled={saving} className="self-start">
            {saving ? t('account.saving') : t('account.save')}
          </Button>
        </form>
      </Card>
    </section>
  )
}

/** Changing your own password, which signs every other device out. */
function ChangePassword() {
  const { t } = useTranslation()

  const [currentPassword, setCurrentPassword] = useState('')
  const [newPassword, setNewPassword] = useState('')
  const [fields, setFields] = useState<Record<string, FieldErrorCode>>({})
  const [saving, setSaving] = useState(false)

  async function submit(event: FormEvent<HTMLFormElement>): Promise<void> {
    event.preventDefault()
    setFields({})
    setSaving(true)

    try {
      const { error, response } = await api.POST('/api/me/password', {
        body: { currentPassword, newPassword },
      })

      if (!response.ok) {
        const reported = fieldErrorsFrom(error)
        if (Object.keys(reported).length > 0) setFields(reported)
        else showToast({ title: t('account.saveFailed'), tone: 'late' })
        return
      }

      setCurrentPassword('')
      setNewPassword('')
      showToast({ title: t('account.passwordChanged') })
    } catch {
      showToast({ title: t('account.saveFailed'), tone: 'late' })
    } finally {
      setSaving(false)
    }
  }

  return (
    <section aria-labelledby="password-heading">
      <h2 id="password-heading" className="text-xl font-semibold text-foreground">
        {t('account.passwordTitle')}
      </h2>
      <p className="mt-1 text-sm text-muted-foreground">{t('account.passwordHint')}</p>

      <Card className="mt-4">
        <form className="flex flex-col gap-4" onSubmit={(event) => void submit(event)}>
          <Field
            label={t('account.currentPassword')}
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
            label={t('account.newPassword')}
            required
            hint={t('register.passwordHint')}
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

          <Button type="submit" disabled={saving} className="self-start">
            {saving ? t('account.saving') : t('account.changePassword')}
          </Button>
        </form>
      </Card>
    </section>
  )
}
