import { useQueryClient } from '@tanstack/react-query'
import { useState, type FormEvent } from 'react'
import { useTranslation } from 'react-i18next'
import { Link, useNavigate } from 'react-router'

import { api } from '@/shared/api/client'
import { countries } from '@/shared/countries'
import { fieldErrorProps, fieldErrorsFrom, type FieldErrorCode } from '@/shared/api/field-errors'
import { LanguageSwitcher } from '@/shared/i18n/language-switcher'
import { rememberIdentity } from '@/shared/session/identity'
import { landingFor, SIGN_IN_PATH } from '@/shared/session/signed-out'
import { DEFAULT_SURFACE } from '@/shared/surface'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { Field } from '@/shared/ui/field'
import { Input } from '@/shared/ui/input'
import { Select } from '@/shared/ui/select'

/**
 * Where an owner registers a restaurant and becomes its admin.
 *
 * One screen and one form, which is the product decision showing through:
 * trying this costs a minute, not an afternoon of settings. The five settings a
 * restaurant starts with are derived from the country and changed later in
 * settings, so nothing here asks for a currency or a timezone.
 *
 * The country list is imported rather than fetched. It is the same file the API
 * compiles in, so the form cannot offer a country registration would refuse.
 *
 * The other of the two screens that render outside the application shell, so
 * like the sign in screen it carries its own heading, landmark, and language
 * switcher.
 */
export function Register() {
  const { t } = useTranslation()
  const queryClient = useQueryClient()
  const navigate = useNavigate()

  const [restaurantName, setRestaurantName] = useState('')
  const [displayName, setDisplayName] = useState('')
  const [email, setEmail] = useState('')
  const [password, setPassword] = useState('')
  const [countryCode, setCountryCode] = useState(countries[0]?.code ?? '')

  const [fields, setFields] = useState<Record<string, FieldErrorCode>>({})
  const [unavailable, setUnavailable] = useState(false)
  const [submitting, setSubmitting] = useState(false)

  async function submit(event: FormEvent<HTMLFormElement>): Promise<void> {
    event.preventDefault()

    setFields({})
    setUnavailable(false)
    setSubmitting(true)

    try {
      const { data, error } = await api.POST('/api/auth/register', {
        body: { restaurantName, displayName, email, password, countryCode },
      })

      if (!data) {
        const reported = fieldErrorsFrom(error)
        if (Object.keys(reported).length > 0) setFields(reported)
        else setUnavailable(true)
        return
      }

      rememberIdentity(queryClient, data)
      await navigate(landingFor(data.staff.role), { replace: true })
    } catch {
      setUnavailable(true)
    } finally {
      setSubmitting(false)
    }
  }

  return (
    <div className="flex min-h-screen flex-col bg-background">
      <header className="flex items-center justify-between px-4 py-3">
        <span className="text-base font-semibold text-foreground">{t('app.name')}</span>
        <LanguageSwitcher surface={DEFAULT_SURFACE} />
      </header>

      <main id="main-content" tabIndex={-1} className="flex flex-1 items-center justify-center p-4">
        <Card className="w-full max-w-md">
          <h1 className="text-xl font-semibold text-foreground">{t('register.title')}</h1>
          <p className="mt-1 text-sm text-muted-foreground">{t('register.subtitle')}</p>

          {unavailable && (
            <p role="alert" className="mt-4 text-sm font-medium text-status-late">
              {t('apiError.unavailable')}
            </p>
          )}

          <form className="mt-6 flex flex-col gap-4" onSubmit={(event) => void submit(event)}>
            <Field
              label={t('register.restaurantName')}
              required
              {...fieldErrorProps(fields['restaurantName'], t)}
            >
              <Input
                name="restaurantName"
                autoComplete="organization"
                value={restaurantName}
                onChange={(event) => {
                  setRestaurantName(event.target.value)
                }}
              />
            </Field>

            <Field
              label={t('register.country')}
              required
              hint={t('register.countryHint')}
              {...fieldErrorProps(fields['countryCode'], t)}
            >
              <Select
                name="countryCode"
                autoComplete="country"
                value={countryCode}
                onChange={(event) => {
                  setCountryCode(event.target.value)
                }}
              >
                {countries.map((country) => (
                  <option key={country.code} value={country.code}>
                    {country.englishName}
                  </option>
                ))}
              </Select>
            </Field>

            <Field
              label={t('register.displayName')}
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

            <Field label={t('register.email')} required {...fieldErrorProps(fields['email'], t)}>
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

            <Field
              label={t('register.password')}
              required
              hint={t('register.passwordHint')}
              {...fieldErrorProps(fields['password'], t)}
            >
              <Input
                type="password"
                name="password"
                autoComplete="new-password"
                value={password}
                onChange={(event) => {
                  setPassword(event.target.value)
                }}
              />
            </Field>

            <Button type="submit" disabled={submitting} className="mt-2">
              {submitting ? t('register.submitting') : t('register.submit')}
            </Button>
          </form>

          <p className="mt-6 text-sm text-muted-foreground">
            {t('register.haveAccount')}{' '}
            <Link to={SIGN_IN_PATH} className="font-medium text-primary underline">
              {t('register.signIn')}
            </Link>
          </p>
        </Card>
      </main>
    </div>
  )
}
