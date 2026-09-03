import { afterEach, describe, expect, it, vi } from 'vitest'

import { catalogue, FALLBACK_LANGUAGE, isKnownLanguage } from '@/shared/i18n/catalogue'

import { restaurantSettings, staffSettings } from './restaurant-settings'

/**
 * The settings every screen formats and translates through.
 *
 * These two functions are placeholders until feature 7 replaces their bodies
 * with a read from the sign in response, so what is worth pinning here is not
 * the values themselves but the two things that stay true afterwards: the
 * placeholder has to be a restaurant the rest of the app considers valid, and
 * a language column that has outlived its catalogue entry has to fall back
 * rather than render nothing.
 */

describe('restaurantSettings', () => {
  it('answers with a language the platform actually offers', () => {
    // A placeholder outside the catalogue would put every screen built between
    // now and feature 7 on a language with no files behind it.
    expect(isKnownLanguage(restaurantSettings().defaultLanguage)).toBe(true)
  }) // covers: AC-6

  it('answers with a formatting locale the platform actually offers', () => {
    expect(catalogue.formattingLocales).toContain(restaurantSettings().formattingLocale)
  }) // covers: AC-12

  it('names a timezone Intl can actually convert with', () => {
    const { timezone } = restaurantSettings()

    expect(() => new Intl.DateTimeFormat('en-US', { timeZone: timezone })).not.toThrow()

    // Checked by what it converts to rather than by name. Intl answers
    // `resolvedOptions().timeZone` with whichever alias the platform's own zone
    // data prefers, so a name comparison would fail on a runner whose ICU still
    // calls this zone Asia/Calcutta while converting identically.
    const evening = new Date('2026-09-02T19:30:00Z')
    const formatted = new Intl.DateTimeFormat('en-GB', {
      timeZone: timezone,
      dateStyle: 'short',
      timeStyle: 'short',
      hour12: false,
    }).format(evening)

    // Five and a half hours on, so the same instant is already the next day.
    expect(formatted).toContain('03/09/2026')
    expect(formatted).toContain('01:00')
  }) // covers: AC-13

  it('names a fixed zone rather than reading the machine’s', () => {
    // Deliberately a named zone, and deliberately one the runner is unlikely to
    // be in. A placeholder that agreed with the device would hide the entire
    // class of bug this feature cares about, which is a timestamp written in
    // the wrong place's clock.
    expect(restaurantSettings().timezone).toBe('Asia/Kolkata')
  }) // covers: AC-13

  it('names a currency Intl can format, with a matching decimal count', () => {
    const { currencyCode, currencyDecimals } = restaurantSettings()

    const formatted = new Intl.NumberFormat('en-US', {
      style: 'currency',
      currency: currencyCode,
    }).format(1)

    expect(formatted).not.toContain(currencyCode)
    expect(currencyDecimals).toBeGreaterThanOrEqual(0)
    expect(Number.isInteger(currencyDecimals)).toBe(true)
  }) // covers: AC-12

  it('keeps the language and the formatting locale as separate values', () => {
    // Never derived from each other. The type system says so on the API side
    // and this says so here.
    const { defaultLanguage, formattingLocale } = restaurantSettings()

    expect(defaultLanguage).not.toBe(formattingLocale)
  }) // covers: AC-12
})

describe('staffSettings', () => {
  it('reports nobody as having a personal language yet', () => {
    // Honest rather than convenient: there is nobody signed in to have a
    // preference until feature 7, so this answers null rather than pretending.
    expect(staffSettings().language).toBeNull()
  }) // covers: AC-6
})

/**
 * The guard on the language column, exercised through the catalogue rather
 * than through the settings.
 *
 * `restaurantLanguage` calls `restaurantSettings` inside its own module, so
 * spying on that export cannot reach it. What it asks the catalogue is the
 * other half of the branch, and that is a different module, so replacing the
 * catalogue is what actually drives both arms.
 */
async function withCatalogueKnowing(
  known: boolean,
): Promise<typeof import('./restaurant-settings')> {
  const actual = await import('@/shared/i18n/catalogue')

  vi.doMock('@/shared/i18n/catalogue', () => ({ ...actual, isKnownLanguage: () => known }))
  vi.resetModules()

  return await import('./restaurant-settings')
}

describe('restaurantLanguage', () => {
  afterEach(() => {
    vi.doUnmock('@/shared/i18n/catalogue')
    vi.resetModules()
  })

  it('answers with the restaurant’s own column while the catalogue offers it', async () => {
    const module = await withCatalogueKnowing(true)

    expect(module.restaurantLanguage()).toBe(module.restaurantSettings().defaultLanguage)
  }) // covers: AC-6, AC-11

  it('falls back when the column holds a code that is no longer offered', async () => {
    // A column can outlive a catalogue entry. When it does the translation
    // files behind that code are gone, and this value also marks up every piece
    // of restaurant typed text on screen, so a stale code would put a wrong
    // `lang` on every dish name as well as failing to translate the shell.
    const module = await withCatalogueKnowing(false)

    expect(module.restaurantLanguage()).toBe(FALLBACK_LANGUAGE)
  }) // covers: AC-6, AC-11
})
