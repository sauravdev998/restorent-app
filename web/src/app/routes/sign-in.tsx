import { useQueryClient } from '@tanstack/react-query'
import { useState, type FormEvent } from 'react'
import { useTranslation } from 'react-i18next'
import { Link, useLocation, useNavigate } from 'react-router'

import { api } from '@/shared/api/client'
import { rememberIdentity } from '@/shared/session/identity'
import { landingFor, REGISTER_PATH, returnPathFrom } from '@/shared/session/signed-out'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { SignedOutShell } from '@/shared/ui/signed-out-shell'
import { Field } from '@/shared/ui/field'
import { Input } from '@/shared/ui/input'

/**
 * Where somebody signs in.
 *
 * One of the two screens that render outside the application shell, and the
 * only one anybody sees before the app knows who they are. So it carries its own
 * heading, its own language switcher, and its own landmark: there is no shell
 * around it to provide them, and a screen with no `main` is a screen a keyboard
 * user cannot skip into.
 *
 * A refused sign in says one thing and never two. The server answers a wrong
 * password and an address nobody has identically, on purpose, so this screen
 * shows one message above the form rather than putting an error beside the
 * address box that would say the address was the part that was wrong.
 */
export function SignIn() {
  const { t } = useTranslation()
  const queryClient = useQueryClient()
  const navigate = useNavigate()
  const location = useLocation()

  const [email, setEmail] = useState('')
  const [password, setPassword] = useState('')
  const [failed, setFailed] = useState(false)
  const [unavailable, setUnavailable] = useState(false)
  const [submitting, setSubmitting] = useState(false)

  async function submit(event: FormEvent<HTMLFormElement>): Promise<void> {
    event.preventDefault()

    setFailed(false)
    setUnavailable(false)
    setSubmitting(true)

    try {
      const { data, response } = await api.POST('/api/auth/sign-in', {
        body: { email, password },
      })

      if (!data) {
        // A 401 is a refused sign in and says so on the form. Anything else is
        // the service being unwell, which is a different sentence: telling
        // somebody their password is wrong when the database is down sends them
        // to reset a password that was fine.
        if (response.status === 401) setFailed(true)
        else setUnavailable(true)
        return
      }

      rememberIdentity(queryClient, data)

      // The role decides where they land, unless a `401` preserved a path on
      // the way in, in which case that wins.
      const fallback = landingFor(data.staff.role)
      await navigate(returnPathFrom(location.search, fallback), { replace: true })
    } catch {
      setUnavailable(true)
    } finally {
      setSubmitting(false)
    }
  }

  return (
    <SignedOutShell>
      <Card className="w-full max-w-sm">
        <h1 className="text-xl font-semibold text-foreground">{t('signIn.title')}</h1>
        <p className="mt-1 text-sm text-muted-foreground">{t('signIn.subtitle')}</p>

        {/* `role="alert"` rather than a field error, because what failed was
              the pair and not either half of it. */}
        {failed && (
          <p role="alert" className="mt-4 text-sm font-medium text-status-late">
            {t('signIn.refused')}
          </p>
        )}
        {unavailable && (
          <p role="alert" className="mt-4 text-sm font-medium text-status-late">
            {t('apiError.unavailable')}
          </p>
        )}

        <form className="mt-6 flex flex-col gap-4" onSubmit={(event) => void submit(event)}>
          <Field label={t('signIn.email')} required>
            <Input
              type="email"
              name="email"
              autoComplete="username"
              autoCapitalize="none"
              autoCorrect="off"
              spellCheck={false}
              value={email}
              onChange={(event) => {
                setEmail(event.target.value)
              }}
            />
          </Field>

          <Field label={t('signIn.password')} required>
            <Input
              type="password"
              name="password"
              autoComplete="current-password"
              value={password}
              onChange={(event) => {
                setPassword(event.target.value)
              }}
            />
          </Field>

          <Button type="submit" disabled={submitting} className="mt-2">
            {submitting ? t('signIn.submitting') : t('signIn.submit')}
          </Button>
        </form>

        <p className="mt-6 text-sm text-muted-foreground">
          {t('signIn.noAccount')}{' '}
          <Link to={REGISTER_PATH} className="font-medium text-primary underline">
            {t('signIn.register')}
          </Link>
        </p>
      </Card>
    </SignedOutShell>
  )
}
