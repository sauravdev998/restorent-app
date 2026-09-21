import { useQueryClient } from '@tanstack/react-query'
import { useState, type FormEvent } from 'react'
import { useTranslation } from 'react-i18next'

import { api } from '@/shared/api/client'
import { failureCode } from '@/shared/api/call-error'
import { fieldErrorProps, fieldErrorsFrom, type FieldErrorCode } from '@/shared/api/field-errors'
import { catalogue } from '@/shared/i18n/catalogue'
import { identityQuery, rememberIdentity, type RestaurantIdentity } from '@/shared/session/identity'
import { useIdentity } from '@/shared/session/use-identity'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { Field } from '@/shared/ui/field'
import { Input } from '@/shared/ui/input'
import { Select } from '@/shared/ui/select'
import { showToast } from '@/shared/ui/toast-store'

/** Seconds per minute, so the two thresholds are typed in minutes and stored in seconds. */
const A_MINUTE = 60

/**
 * The restaurant's own settings, for an admin.
 *
 * Seven things an owner can change, and deliberately no money setting among
 * them. The currency and its decimals were fixed by the country at registration
 * and changing them under bills that have already been closed would rewrite
 * history; the service charge has its own audited path.
 *
 * The two language fields are separate and not one, which is the decision spec
 * 0005 made showing through. An owner in India reading the interface in English
 * still wants rupees grouped the Indian way on their own figures, and a bill has
 * to look identical to every member of staff whatever each of them reads.
 *
 * **Every save names the version it was made against** (spec 0012). Two owners
 * with this screen open no longer overwrite each other silently: the second save
 * is refused, this screen says so, and it reloads the settings as they now stand
 * rather than leaving somebody looking at a form that has already lost.
 *
 * **The kitchen thresholds are typed in minutes and stored in seconds.** Nobody
 * thinks about their pass in seconds, and the stored unit is what an API with a
 * range check wants. Either one may be saved on its own, and the rule that amber
 * comes before red spans both, so the server merges what was sent over what is
 * stored and checks the pair.
 *
 * A waiter or a chef never reaches this screen: the route's loader sends them to
 * their own surface, and the endpoint behind it refuses them anyway. The second
 * of those is the control; the first is a courtesy.
 */
export function RestaurantSettings() {
  const { t } = useTranslation('admin')
  const identity = useIdentity()
  const { restaurant } = identity

  // Outside the form, so it survives the remount below. A refusal is the one thing
  // on this screen that has to outlive the boxes it is about.
  const [stale, setStale] = useState(false)

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

      {stale && (
        <div
          role="status"
          className="border-line border-status-late bg-card p-4 text-status-late"
          data-testid="settings-stale"
        >
          <p className="font-semibold">{t('settings.staleTitle')}</p>
          <p className="text-sm text-card-foreground">{t('settings.staleBody')}</p>
        </div>
      )}

      {/* Keyed on the version, which is React's own answer to "reset this form
          when the thing behind it changed". A refused save refetches the row,
          whose version is whoever won the race's, so this form is thrown away and
          rebuilt from their settings rather than being reset field by field in an
          effect. */}
      <SettingsForm key={restaurant.version} restaurant={restaurant} onStale={setStale} />
    </div>
  )
}

/**
 * The boxes themselves, holding one draft of one version of the settings.
 *
 * Mounted fresh whenever the stored version moves, so every piece of state in it
 * belongs to the row it was built from and there is nothing to reset.
 */
function SettingsForm({
  restaurant,
  onStale,
}: {
  restaurant: RestaurantIdentity
  onStale: (stale: boolean) => void
}) {
  const { t } = useTranslation('admin')
  const { t: common } = useTranslation()
  const queryClient = useQueryClient()

  const [name, setName] = useState(restaurant.name)
  const [address, setAddress] = useState(restaurant.address ?? '')
  const [timezone, setTimezone] = useState(restaurant.timezone)
  const [defaultLanguage, setDefaultLanguage] = useState(restaurant.defaultLanguage)
  const [formattingLocale, setFormattingLocale] = useState(restaurant.formattingLocale)
  const [warningMinutes, setWarningMinutes] = useState(
    String(restaurant.kitchenWarningAfterSeconds / A_MINUTE),
  )
  const [lateMinutes, setLateMinutes] = useState(
    String(restaurant.kitchenLateAfterSeconds / A_MINUTE),
  )

  const [fields, setFields] = useState<Record<string, FieldErrorCode>>({})
  const [saving, setSaving] = useState(false)

  /**
   * The seconds to send for one threshold, or `undefined` to leave it alone.
   *
   * Sent only when it differs from what is stored, so a form saved for its name
   * alone does not restate two numbers it never touched, and a restaurant whose
   * threshold is not a whole number of minutes keeps it.
   */
  function secondsToSend(typed: string, storedSeconds: number): number | undefined {
    const minutes = Number(typed)
    if (!Number.isFinite(minutes)) return undefined

    const seconds = Math.round(minutes * A_MINUTE)
    return seconds === storedSeconds ? undefined : seconds
  }

  async function submit(event: FormEvent<HTMLFormElement>): Promise<void> {
    event.preventDefault()
    setFields({})
    onStale(false)
    setSaving(true)

    const kitchenWarningAfterSeconds = secondsToSend(
      warningMinutes,
      restaurant.kitchenWarningAfterSeconds,
    )
    const kitchenLateAfterSeconds = secondsToSend(lateMinutes, restaurant.kitchenLateAfterSeconds)

    try {
      const { data, error } = await api.PATCH('/api/restaurant', {
        body: {
          version: restaurant.version,
          name,
          address,
          timezone,
          defaultLanguage,
          formattingLocale,
          ...(kitchenWarningAfterSeconds === undefined ? {} : { kitchenWarningAfterSeconds }),
          ...(kitchenLateAfterSeconds === undefined ? {} : { kitchenLateAfterSeconds }),
        },
      })

      if (!data) {
        if (failureCode(error) === 'restaurant_changed') {
          onStale(true)
          // Nothing of this admin's was written, so the honest thing is to show
          // what is really stored. The new version remounts this form with it.
          await queryClient.invalidateQueries({ queryKey: identityQuery.queryKey })
          return
        }

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

        <fieldset className="flex flex-col gap-4">
          <legend className="text-base font-semibold text-card-foreground">
            {t('settings.kitchenHeading')}
          </legend>
          <p className="text-sm text-muted-foreground">{t('settings.kitchenIntro')}</p>

          <Field
            label={t('settings.kitchenWarning')}
            hint={t('settings.kitchenWarningHint')}
            {...fieldErrorProps(fields['kitchenWarningAfterSeconds'], common)}
          >
            <Input
              name="kitchenWarningAfterSeconds"
              type="number"
              inputMode="numeric"
              min={1}
              max={240}
              value={warningMinutes}
              onChange={(event) => {
                setWarningMinutes(event.target.value)
              }}
            />
          </Field>

          <Field
            label={t('settings.kitchenLate')}
            hint={t('settings.kitchenLateHint')}
            {...fieldErrorProps(fields['kitchenLateAfterSeconds'], common)}
          >
            <Input
              name="kitchenLateAfterSeconds"
              type="number"
              inputMode="numeric"
              min={1}
              max={240}
              value={lateMinutes}
              onChange={(event) => {
                setLateMinutes(event.target.value)
              }}
            />
          </Field>
        </fieldset>

        <Button type="submit" disabled={saving} className="self-start">
          {saving ? t('settings.saving') : t('settings.save')}
        </Button>
      </form>
    </Card>
  )
}
