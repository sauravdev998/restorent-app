import { afterEach, describe, expect, it, vi } from 'vitest'

import {
  catalogue,
  directionOf,
  FALLBACK_FORMATTING_LOCALE,
  FALLBACK_LANGUAGE,
  findLanguage,
  isKnownLanguage,
} from './catalogue'

/**
 * The shared catalogue, and the parse that stands between a malformed file and
 * a screen.
 *
 * Two subjects here. The committed file, which both this app and the API read,
 * so a mistake in it breaks both sides at once. And the parse itself, checked
 * by handing it catalogues that do not exist on disk: adding a language has to
 * be one entry and nothing else, and a broken entry has to fail loudly at boot
 * rather than quietly at the first write.
 */

/**
 * Loads the module fresh against a catalogue of our own.
 *
 * The real file is imported through the `@catalogue` alias and parsed once when
 * the module is first imported, so testing the parse means replacing that
 * import and re-importing. This is also exactly how a new language arrives: a
 * different catalogue, and no other change anywhere.
 */
async function loadCatalogue(raw: unknown): Promise<typeof import('./catalogue')> {
  vi.doMock('@catalogue', () => ({ default: raw }))
  vi.resetModules()
  return await import('./catalogue')
}

const ENGLISH = {
  code: 'en',
  englishName: 'English',
  nativeName: 'English',
  direction: 'ltr',
}

/** A whole valid catalogue, to vary one field of at a time. */
function validCatalogue() {
  return {
    languages: [{ ...ENGLISH }],
    formattingLocales: ['en-US'],
    defaults: { language: 'en', formattingLocale: 'en-US' },
  }
}

afterEach(() => {
  vi.doUnmock('@catalogue')
  vi.resetModules()
})

describe('the committed catalogue', () => {
  it('offers both languages that ship, each named in its own script', () => {
    const english = findLanguage('en')
    const hindi = findLanguage('hi')

    expect(english?.nativeName).toBe('English')
    // Devanagari, not the word "Hindi". Somebody looking for their own language
    // is by definition somebody who may not be able to read the current one.
    expect(hindi?.nativeName).toBe('हिन्दी')
  }) // covers: AC-2

  it('names the fallback as a language it actually offers', () => {
    // A fallback outside the list would be a language every missing key falls
    // through to and no file exists for.
    expect(isKnownLanguage(FALLBACK_LANGUAGE)).toBe(true)
  }) // covers: AC-3

  it('names the fallback formatting locale as one it actually offers', () => {
    expect(catalogue.formattingLocales).toContain(FALLBACK_FORMATTING_LOCALE)
  }) // covers: AC-12

  it('gives every language a native name, a code, and a direction', () => {
    for (const language of catalogue.languages) {
      expect(language.code.trim()).not.toBe('')
      expect(language.nativeName.trim()).not.toBe('')
      expect(language.englishName.trim()).not.toBe('')
      expect(['ltr', 'rtl']).toContain(language.direction)
    }
  }) // covers: AC-2

  it('keeps the language list and the formatting list apart', () => {
    // The two are separate lists on purpose. A restaurant reading English may
    // well write its figures the way an accountant in India expects, and
    // deriving one from the other is the mistake this shape prevents.
    for (const locale of catalogue.formattingLocales) {
      expect(isKnownLanguage(locale)).toBe(false)
    }
  }) // covers: AC-12
})

describe('isKnownLanguage', () => {
  it('accepts a code the platform offers', () => {
    expect(isKnownLanguage('en')).toBe(true)
    expect(isKnownLanguage('hi')).toBe(true)
  }) // covers: AC-2

  it('refuses a code that is not offered, whatever shape it arrives in', () => {
    // Everything here can reach this function for real: a stale code left in
    // storage by an older build, a column set before a language was removed, an
    // empty string from a cleared setting.
    expect(isKnownLanguage('xx')).toBe(false)
    expect(isKnownLanguage('')).toBe(false)
    expect(isKnownLanguage(null)).toBe(false)
    expect(isKnownLanguage(undefined)).toBe(false)
  }) // covers: AC-6

  it('refuses a formatting locale offered as a language', () => {
    // `en-US` is a real entry in the other list, which is what makes this the
    // confusion worth pinning rather than a nonsense input.
    expect(isKnownLanguage('en-US')).toBe(false)
    expect(isKnownLanguage('hi-IN')).toBe(false)
  }) // covers: AC-12
})

describe('directionOf', () => {
  it('answers with the language’s own direction', () => {
    expect(directionOf('en')).toBe('ltr')
    expect(directionOf('hi')).toBe('ltr')
  }) // covers: AC-10

  it('falls back rather than throwing on a code it does not know', () => {
    // This feeds the `dir` attribute on the root element. A layout written the
    // usual way is bad; a layout that fails to render is worse.
    expect(directionOf('xx')).toBe('ltr')
    expect(directionOf('')).toBe('ltr')
  }) // covers: AC-10
})

describe('adding a language', () => {
  it('needs one catalogue entry and no other change', async () => {
    const withArabic = validCatalogue()
    withArabic.languages.push({
      code: 'ar',
      englishName: 'Arabic',
      nativeName: 'العربية',
      direction: 'rtl',
    })

    const module = await loadCatalogue(withArabic)

    // Offered, findable, and carrying its own direction, from the entry alone.
    expect(module.isKnownLanguage('ar')).toBe(true)
    expect(module.findLanguage('ar')?.nativeName).toBe('العربية')
    expect(module.directionOf('ar')).toBe('rtl')
  }) // covers: AC-4

  it('carries a right to left language through to the dir attribute', async () => {
    // Both shipped languages are written left to right, so this is the only
    // place the right to left path is exercised at all. It is the difference
    // between adding Arabic later being a catalogue entry and being a sweep
    // across every screen.
    const rightToLeft = validCatalogue()
    rightToLeft.languages = [
      { code: 'ar', englishName: 'Arabic', nativeName: 'العربية', direction: 'rtl' },
    ]
    rightToLeft.defaults.language = 'ar'

    const module = await loadCatalogue(rightToLeft)

    expect(module.directionOf('ar')).toBe('rtl')
    // And an unknown code now falls back to the fallback's direction rather
    // than to a hardcoded `ltr`.
    expect(module.directionOf('xx')).toBe('rtl')
  }) // covers: AC-4, AC-10
})

describe('a malformed catalogue', () => {
  it('fails naming the file, so nobody hunts for the source', async () => {
    await expect(loadCatalogue('not an object')).rejects.toThrow(/locales\/catalogue\.json/)
  }) // covers: AC-4

  it('refuses a direction that is neither way round, naming the entry', async () => {
    const broken = validCatalogue()
    broken.languages = [{ ...ENGLISH, direction: 'sideways' }]

    await expect(loadCatalogue(broken)).rejects.toThrow(/languages\[0\]\.direction/)
  }) // covers: AC-4

  it('refuses a language with no native name', async () => {
    const broken = validCatalogue()
    broken.languages = [{ ...ENGLISH, nativeName: '   ' }]

    await expect(loadCatalogue(broken)).rejects.toThrow(/languages\[0\]\.nativeName/)
  }) // covers: AC-4

  it('refuses a language whose code is missing', async () => {
    const broken: Record<string, unknown> = validCatalogue()
    broken['languages'] = [{ englishName: 'English', nativeName: 'English', direction: 'ltr' }]

    await expect(loadCatalogue(broken)).rejects.toThrow(/languages\[0\]\.code/)
  }) // covers: AC-4

  it('refuses an empty language list', async () => {
    const broken = validCatalogue()
    broken.languages = []

    await expect(loadCatalogue(broken)).rejects.toThrow(/languages is empty/)
  }) // covers: AC-4

  it('refuses a language list that is not a list', async () => {
    const broken: Record<string, unknown> = validCatalogue()
    broken['languages'] = { en: ENGLISH }

    await expect(loadCatalogue(broken)).rejects.toThrow(/languages is not an array/)
  }) // covers: AC-4

  it('refuses a missing defaults block', async () => {
    const broken: Record<string, unknown> = validCatalogue()
    delete broken['defaults']

    await expect(loadCatalogue(broken)).rejects.toThrow(/defaults/)
  }) // covers: AC-4

  it('refuses a formatting locale that is not a string', async () => {
    const broken: Record<string, unknown> = validCatalogue()
    broken['formattingLocales'] = ['en-US', 42]

    await expect(loadCatalogue(broken)).rejects.toThrow(/formattingLocales\[1\]/)
  }) // covers: AC-4
})
