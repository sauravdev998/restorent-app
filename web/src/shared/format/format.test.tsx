import { describe, expect, it, vi } from 'vitest'

import type { RestaurantFormatting } from '@/shared/session/identity'

import i18next from '@/shared/i18n'

import {
  formatMoney,
  formatNumber,
  formatTimestamp,
  formatUnitList,
  parseDecimalInput,
} from './index'

/**
 * The formatters, checked against the cases that actually break.
 *
 * Every test here sets the restaurant's settings deliberately rather than
 * taking whatever the machine happens to have. That is the whole subject: the
 * failure this feature exists to prevent is a figure written in the reader's
 * locale or the runner's timezone instead of the restaurant's.
 */

const BASE: RestaurantFormatting = {
  formattingLocale: 'en-US',
  timezone: 'Asia/Kolkata',
  currencyCode: 'INR',
  currencyDecimals: 2,
}

/**
 * Runs a body with the restaurant configured a particular way.
 *
 * The module reads the settings on every call rather than closing over them at
 * import time, which is what makes this possible and is also what lets the
 * identity change under a screen without touching a formatter.
 */
async function withSettings<T>(settings: Partial<RestaurantFormatting>, body: () => T): Promise<T> {
  const module = await import('@/shared/session/identity')
  const spy = vi.spyOn(module, 'restaurantFormatting').mockReturnValue({ ...BASE, ...settings })

  try {
    return body()
  } finally {
    spy.mockRestore()
  }
}

describe('formatMoney', () => {
  it('groups an Indian locale the Indian way, which is not every three digits', async () => {
    // The case a naive implementation gets wrong: en-IN groups as 1,23,456.78
    // and en-US as 123,456.78, from the same number and the same currency.
    const indian = await withSettings({ formattingLocale: 'en-IN' }, () =>
      formatMoney('123456.78', 'INR', 2),
    )
    const american = await withSettings({ formattingLocale: 'en-US' }, () =>
      formatMoney('123456.78', 'INR', 2),
    )

    expect(indian).toContain('1,23,456.78')
    expect(american).toContain('123,456.78')
  }) // covers: AC-12

  it('formats identically whatever language the reader has chosen', async () => {
    // Formatting follows the restaurant, never the interface language. A bill
    // has to read the same to the waiter reading Hindi and the one reading
    // English, because it is the same bill.
    //
    // The language is not even reachable from here any more, which is a
    // stronger statement of the same rule than the test used to make:
    // `RestaurantFormatting` carries the four values a formatter needs and no
    // language at all, so there is no longer a way to write one that read it.
    const before = await withSettings({ formattingLocale: 'en-IN' }, () =>
      formatMoney('123456.78', 'INR', 2),
    )

    await i18next.changeLanguage('hi')
    const after = await withSettings({ formattingLocale: 'en-IN' }, () =>
      formatMoney('123456.78', 'INR', 2),
    )
    await i18next.changeLanguage('en')

    expect(after).toBe(before)
  }) // covers: AC-12

  it('writes Latin digits even where the locale would not', async () => {
    const amount = await withSettings({ formattingLocale: 'hi-IN' }, () =>
      formatMoney('1234.50', 'INR', 2),
    )

    // No Devanagari digit anywhere in a price. A total half the staff cannot
    // read at a glance is worse than one in a script that is not theirs.
    expect(amount).not.toMatch(/[०-९]/)
    expect(amount).toContain('1,234.50')
  }) // covers: AC-12

  it('keeps a zero decimal currency free of a decimal point', async () => {
    const amount = await withSettings({ formattingLocale: 'en-US' }, () =>
      formatMoney('1234', 'JPY', 0),
    )

    expect(amount).toContain('1,234')
    expect(amount).not.toContain('.')
  }) // covers: AC-12

  it('holds a three decimal currency to three places', async () => {
    const amount = await withSettings({ formattingLocale: 'en-US' }, () =>
      formatMoney('1234.567', 'KWD', 3),
    )

    expect(amount).toContain('1,234.567')
  }) // covers: AC-12

  it('formats a negative amount, which a refund line really is', async () => {
    const amount = await withSettings({ formattingLocale: 'en-US' }, () =>
      formatMoney('-45.50', 'INR', 2),
    )

    expect(amount).toContain('45.50')
    expect(amount).toMatch(/-|\(/)
  }) // covers: AC-12

  it('formats the exact decimal, not the nearest float', async () => {
    // The reason the whole chain carries a string. This value is not exactly
    // representable as a double, and parsing it first would round it.
    const amount = await withSettings({ formattingLocale: 'en-US' }, () =>
      formatMoney('9007199254740993.15', 'INR', 2),
    )

    expect(amount).toContain('9,007,199,254,740,993.15')
  }) // covers: AC-12

  it('uses the currency it is handed, not the restaurant’s current one', async () => {
    // A bill keeps the currency it was charged in. Reprinting one after the
    // restaurant switched currency must not silently restate the total.
    const amount = await withSettings({ currencyCode: 'INR', currencyDecimals: 2 }, () =>
      formatMoney('1234', 'JPY', 0),
    )

    expect(amount).not.toContain('.')
    expect(amount).toContain('1,234')
  }) // covers: AC-12

  it('shows a value that is not an exact decimal rather than crashing on it', async () => {
    const noise = vi.spyOn(console, 'error').mockImplementation(() => undefined)

    const amount = await withSettings({}, () => formatMoney('not a number', 'INR', 2))

    expect(amount).toBe('not a number')
    expect(noise).toHaveBeenCalled()
    noise.mockRestore()
  }) // covers: AC-12
})

describe('formatTimestamp', () => {
  it('converts to the restaurant’s timezone, not the machine’s', async () => {
    // Deliberately two timezones on opposite sides of the same instant. 19:30
    // UTC is the same moment as one o'clock the next morning in Kolkata and
    // half past two the same afternoon in New York.
    const instant = '2026-09-02T19:30:00Z'

    const kolkata = await withSettings({ timezone: 'Asia/Kolkata' }, () =>
      formatTimestamp(instant, 'time'),
    )
    const newYork = await withSettings({ timezone: 'America/New_York' }, () =>
      formatTimestamp(instant, 'time'),
    )

    expect(kolkata).toContain('1:00')
    expect(newYork).toContain('3:30')
    expect(kolkata).not.toBe(newYork)
  }) // covers: AC-13

  it('writes the date in the restaurant’s locale', async () => {
    const american = await withSettings({ formattingLocale: 'en-US', timezone: 'UTC' }, () =>
      formatTimestamp('2026-09-02T12:00:00Z', 'date'),
    )
    const british = await withSettings({ formattingLocale: 'en-GB', timezone: 'UTC' }, () =>
      formatTimestamp('2026-09-02T12:00:00Z', 'date'),
    )

    expect(american).not.toBe(british)
  }) // covers: AC-12, AC-13

  it('answers with nothing for a timestamp it cannot read', async () => {
    const empty = await withSettings({}, () => formatTimestamp('not a timestamp'))
    expect(empty).toBe('')
  }) // covers: AC-13
})

describe('formatNumber and formatUnitList', () => {
  it('groups a plain number the restaurant’s way', async () => {
    const indian = await withSettings({ formattingLocale: 'en-IN' }, () => formatNumber(1234567))
    expect(indian).toBe('12,34,567')
  }) // covers: AC-12

  it('joins the parts of one duration without an "and"', async () => {
    // The default list type would say "12 minutes and 30 seconds", which is how
    // you list two separate things rather than two parts of one measurement.
    const spoken = await withSettings({ formattingLocale: 'en-US' }, () =>
      formatUnitList(['12 minutes', '30 seconds']),
    )

    expect(spoken).toBe('12 minutes, 30 seconds')
    expect(spoken).not.toContain('and')
  }) // covers: AC-12

  it('falls back to the default locale rather than failing on an unknown one', async () => {
    const amount = await withSettings({ formattingLocale: 'not-a-locale' }, () =>
      formatNumber(1234567),
    )

    expect(amount).toBe('1,234,567')
  }) // covers: AC-12
})

describe('parseDecimalInput', () => {
  it('turns what was typed into the plain decimal the API reads', () => {
    expect(parseDecimalInput('320', 'en-IN')).toBe('320')
    expect(parseDecimalInput(' 320.50 ', 'en-IN')).toBe('320.50')
    expect(parseDecimalInput('0', 'en-IN')).toBe('0')
  }) // covers: AC-14 (spec 0008)

  it("accepts the locale's own decimal mark as well as the point", () => {
    // An owner in Germany types a price the way they write one.
    expect(parseDecimalInput('12,50', 'de-DE')).toBe('12.50')
    expect(parseDecimalInput('12.50', 'de-DE')).toBe('12.50')
  }) // covers: AC-14 (spec 0008)

  it('refuses a grouping separator rather than guessing what it meant', () => {
    // In an Indian locale the comma groups thousands, so `1,500` is fifteen
    // hundred to the person typing and one and a half to anything guessing.
    expect(parseDecimalInput('1,500', 'en-IN')).toBeNull()
    expect(parseDecimalInput('1.234,5', 'de-DE')).toBeNull()
    expect(parseDecimalInput('1,2,3', 'de-DE')).toBeNull()
  }) // covers: AC-14 (spec 0008)

  it('refuses anything that is not digits and one mark', () => {
    for (const text of ['', '   ', 'abc', '.5', '5.', '1e3', '₹5', '1 000', '--1']) {
      expect(parseDecimalInput(text, 'en-IN'), text).toBeNull()
    }
  }) // covers: AC-14 (spec 0008)

  it('passes a minus sign through, for the API to refuse by name', () => {
    // The API says `negative`, which reads better than "not a number".
    expect(parseDecimalInput('-1', 'en-IN')).toBe('-1')
  }) // covers: AC-14 (spec 0008)
})
