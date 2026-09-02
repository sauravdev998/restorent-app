import { Languages } from 'lucide-react'
import { useState, type ChangeEvent } from 'react'
import { useTranslation } from 'react-i18next'

import type { Surface } from '@/shared/surface'
import { cn } from '@/shared/ui/cn'
import { Field } from '@/shared/ui/field'
import { Icon } from '@/shared/ui/icon'
import { Select } from '@/shared/ui/select'
import { showToast } from '@/shared/ui/toast-store'

import { catalogue, FALLBACK_LANGUAGE } from './catalogue'
import { changeLanguage } from './index'
import { pseudoAvailable, PSEUDO_LANGUAGE, PSEUDO_LANGUAGE_NAME } from './pseudo'
import { writeStoredLanguage } from './resolve'

export interface LanguageSwitcherProps {
  /** Which surface it is sitting on, so the right namespaces are fetched. */
  surface: Surface
  className?: string
}

/**
 * Picks the language the interface is drawn in.
 *
 * Every option is written in its own script, never translated. Somebody looking
 * for Hindi is by definition somebody who may not be able to read the word
 * "Hindi" in the language currently on screen, so the list says हिन्दी.
 *
 * Nothing here reloads the page. The screen re draws in place, the live event
 * stream stays open, and the query cache is untouched.
 *
 * A failed switch changes nothing at all. The files are fetched before the
 * language moves, so when a request fails on a phone with one bar the language
 * stays put, the choice is not remembered, and a toast says so. A screen half in
 * one language and half in another is worse than a screen that stayed as it was.
 *
 * Absent from the kitchen shell. That screen follows the restaurant, not
 * whoever last walked past it.
 */
export function LanguageSwitcher({ surface, className }: LanguageSwitcherProps) {
  const { t, i18n } = useTranslation()
  const [switching, setSwitching] = useState(false)

  const current = i18n.resolvedLanguage ?? FALLBACK_LANGUAGE

  // The real languages, plus the fake one while developing. In a production
  // build `pseudoAvailable` is the literal `false`, so this collapses to the
  // catalogue's own list and the option is never emitted.
  const options = [
    ...catalogue.languages.map((language) => ({
      code: language.code,
      name: language.nativeName,
      lang: language.code,
    })),
    ...(pseudoAvailable
      ? [{ code: PSEUDO_LANGUAGE, name: PSEUDO_LANGUAGE_NAME, lang: FALLBACK_LANGUAGE }]
      : []),
  ]

  async function switchTo(next: string): Promise<void> {
    setSwitching(true)
    try {
      await changeLanguage(next, surface)
      // Remembered only once the switch actually worked, and only as the seed
      // for the next sign in screen on this device. The signed in resolver
      // never reads it back, so this can never override somebody's own setting.
      writeStoredLanguage(next)
    } catch {
      // Deliberately after the failure, in the language still on screen, which
      // is the one the person can read.
      showToast({
        title: t('language.failed'),
        description: t('language.failedHint'),
        tone: 'late',
      })
    } finally {
      setSwitching(false)
    }
  }

  function onChange(event: ChangeEvent<HTMLSelectElement>): void {
    void switchTo(event.target.value)
  }

  return (
    <div className={cn('flex items-center gap-2', className)}>
      {/* No label: decorative, because the control beside it is already named. */}
      <Icon icon={Languages} size="sm" className="text-muted-foreground" />
      <Field label={t('language.label')} labelHidden>
        <Select value={current} onChange={onChange} disabled={switching} className="w-auto">
          {options.map((option) => (
            // `lang` on the option, because the list is a mix of scripts by
            // design and a reader announcing हिन्दी with English phonetics
            // defeats the point of showing it in its own script.
            <option key={option.code} value={option.code} lang={option.lang}>
              {option.name}
            </option>
          ))}
        </Select>
      </Field>
    </div>
  )
}
