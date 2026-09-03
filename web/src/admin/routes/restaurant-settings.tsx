import { useQueryClient } from '@tanstack/react-query'
import { useState, type FormEvent } from 'react'
import { useTranslation } from 'react-i18next'
import { useRouteLoaderData } from 'react-router'

import { api } from '@/shared/api/client'
import { fieldErrorProps, fieldErrorsFrom, type FieldErrorCode } from '@/shared/api/field-errors'
import { catalogue } from '@/shared/i18n/catalogue'
import { rememberIdentity, type Identity } from '@/shared/session/identity'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { Field } from '@/shared/ui/field'
import { Input } from '@/shared/ui/input'
import { Select } from '@/shared/ui/select'
import { showToast } from '@/shared/ui/toast-store'

/**
 * The restaurant's own settings, for an admin.
 *
 * Five things an owner can change, and deliberately no money setting among
 * them. The currency and its decimals were fixed by the country at registration
 * and changing them under bills that have already been closed would rewrite
 * history; the service charge has its own audited path.
 *
 * The two language fields are separate and not one, which is the decision spec
 * 0005 made showing through. An owner in India reading the interface in English
 * still wants rupees grouped the Indian way on their own figures, and a bill has
 * to look identical to every member of staff whatever each of them reads.
 *
 * A waiter or a chef never reaches this screen: the route's loader sends them to
 * their own surface, and the endpoint behind it refuses them anyway. The second
 * of those is the control; the first is a courtesy.
 */
export function RestaurantSettings() {
  const { t } = useTranslation('admin')
  const { t: common } = useTranslation()
  const queryClient = useQueryClient()
  const identity = useRouteLoaderData('root') as Identity
  const { restaurant } = identity

  const [name, setName] = useState(restaurant.name)
  const [address, setAddress] = useState(restaurant.address ?? '')
  const [timezone, setTimezone] = useState(restaurant.timezone)
  const [defaultLanguage, setDefaultLanguage] = useState(restaurant.defaultLanguage)
  const [formattingLocale, setFormattingLocale] = useState(restaurant.formattingLocale)

  const [fields, setFields] = useState<Record<string, FieldErrorCode>>({})
  const [saving, setSaving] = useState(false)

  async function submit(event: FormEvent<HTMLFormElement>): Promise<void> {
    event.preventDefault()
    setFields({})
    setSaving(true)

    try {
      const { data, error } = await api.PATCH('/api/restaurant', {
        body: { name, address, timezone, defaultLanguage, formattingLocale },
      })

      if (!data) {
        const reported = fieldErrorsFrom(error)
        if (Object.keys(reported).length > 0) setFields(reported)
        else showToast({ title: t('settings.failed'), tone: 'late' })
        return
      }

      rememberIdentity(queryClient, data)
      showToast({ title: t('settings.saved') })
    } catch {
      showToast({ title: t('settings.failed'), tone: 'late' })
    } finally {
      setSaving(false)
    }
  }

  return (
    <div className="space-y-8">
      <section aria-labelledby="settings-heading">
        <h1 id="settings-heading" className="text-2xl font-semibold text-foreground">
          {t('settings.title')}
        </h1>
        <p className="mt-1 text-sm text-muted-foreground">
          {t('settings.currencyFixed', {
            currency: restaurant.currencyCode,
            country: restaurant.countryCode,
          })}
        </p>
      </section>

      <Card>
        <form className="flex flex-col gap-4" onSubmit={(event) => void submit(event)}>
          <Field label={t('settings.name')} required {...fieldErrorProps(fields['name'], common)}>
            <Input
              name="name"
              value={name}
              onChange={(event) => {
                setName(event.target.value)
              }}
            />
          </Field>

          <Field
            label={t('settings.address')}
            hint={t('settings.addressHint')}
            {...fieldErrorProps(fields['address'], common)}
          >
            <Input
              name="address"
              autoComplete="street-address"
              value={address}
              onChange={(event) => {
                setAddress(event.target.value)
              }}
            />
          </Field>

          <Field
            label={t('settings.timezone')}
            hint={t('settings.timezoneHint')}
            {...fieldErrorProps(fields['timezone'], common)}
          >
            <Input
              name="timezone"
              value={timezone}
              onChange={(event) => {
                setTimezone(event.target.value)
              }}
            />
          </Field>

          <Field
            label={t('settings.defaultLanguage')}
            hint={t('settings.defaultLanguageHint')}
            {...fieldErrorProps(fields['defaultLanguage'], common)}
          >
            <Select
              name="defaultLanguage"
              value={defaultLanguage}
              onChange={(event) => {
                setDefaultLanguage(event.target.value)
              }}
            >
              {catalogue.languages.map((entry) => (
                <option key={entry.code} value={entry.code} lang={entry.code}>
                  {entry.nativeName}
                </option>
              ))}
            </Select>
          </Field>

          <Field
            label={t('settings.formattingLocale')}
            hint={t('settings.formattingLocaleHint')}
            {...fieldErrorProps(fields['formattingLocale'], common)}
          >
            <Select
              name="formattingLocale"
              value={formattingLocale}
              onChange={(event) => {
                setFormattingLocale(event.target.value)
              }}
            >
              {catalogue.formattingLocales.map((locale) => (
                <option key={locale} value={locale}>
                  {locale}
                </option>
              ))}
            </Select>
          </Field>

          <Button type="submit" disabled={saving} className="self-start">
            {saving ? t('settings.saving') : t('settings.save')}
          </Button>
        </form>
      </Card>
    </div>
  )
}
