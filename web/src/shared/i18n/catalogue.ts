import raw from '@catalogue'

/**
 * The shared language catalogue, parsed and typed.
 *
 * The file itself is `locales/catalogue.json` at the repository root, which the
 * API compiles in with `include_str!` and reads the same way. One list, both
 * sides, so the switcher can never offer a language the API refuses and the API
 * can never accept one this app has no files for.
 *
 * It is parsed here rather than cast. A cast would type a file this module does
 * not control, and the shape it is typed as is exactly the shape everything
 * downstream then trusts: the switcher's list, the loader's known codes, and the
 * `dir` attribute on the root element. Parsing costs a few microseconds once at
 * boot and turns a malformed catalogue into a loud failure at the point it can
 * still be read, which is the same rule the API applies to the same file.
 */

/** Which way a language is written. It becomes the `dir` attribute. */
export type Direction = 'ltr' | 'rtl'

/** One language the platform offers. */
export interface CatalogueLanguage {
  /** The code stored in the database and used in a file path, such as `hi`. */
  code: string
  /** What to call it in English, for an English speaking admin. */
  englishName: string
  /**
   * What to call it in itself, which is what the switcher shows. Somebody who
   * cannot read the current language has to be able to find their own.
   */
  nativeName: string
  /** Which way it is written. */
  direction: Direction
}

/** The whole list of what languages exist, and how figures may be written. */
export interface Catalogue {
  /** Every language the interface can be read in. */
  languages: CatalogueLanguage[]
  /**
   * Every locale a restaurant may format its money, numbers, and dates with.
   * A separate list on purpose: it is shaped by where restaurants are, not by
   * what their staff read.
   */
  formattingLocales: string[]
  /** What an unconfigured restaurant falls back to. */
  defaults: {
    /** The default interface language. */
    language: string
    /** The default formatting locale. */
    formattingLocale: string
  }
}

function fail(what: string): never {
  throw new Error(`locales/catalogue.json is malformed: ${what}`)
}

function asRecord(value: unknown, what: string): Record<string, unknown> {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    fail(`${what} is not an object`)
  }
  return value as Record<string, unknown>
}

function asString(value: unknown, what: string): string {
  if (typeof value !== 'string' || value.trim() === '') fail(`${what} is not a non empty string`)
  return value
}

function asArray(value: unknown, what: string): unknown[] {
  if (!Array.isArray(value)) fail(`${what} is not an array`)
  return value
}

function parseLanguage(value: unknown, index: number): CatalogueLanguage {
  const where = `languages[${String(index)}]`
  const entry = asRecord(value, where)
  const direction = asString(entry['direction'], `${where}.direction`)

  if (direction !== 'ltr' && direction !== 'rtl') {
    fail(`${where}.direction is ${JSON.stringify(direction)}, which is neither "ltr" nor "rtl"`)
  }

  return {
    code: asString(entry['code'], `${where}.code`),
    englishName: asString(entry['englishName'], `${where}.englishName`),
    nativeName: asString(entry['nativeName'], `${where}.nativeName`),
    direction,
  }
}

function parse(value: unknown): Catalogue {
  const root = asRecord(value, 'the catalogue')
  const languages = asArray(root['languages'], 'languages').map(parseLanguage)

  if (languages.length === 0) fail('languages is empty')

  const defaults = asRecord(root['defaults'], 'defaults')

  return {
    languages,
    formattingLocales: asArray(root['formattingLocales'], 'formattingLocales').map((entry, index) =>
      asString(entry, `formattingLocales[${String(index)}]`),
    ),
    defaults: {
      language: asString(defaults['language'], 'defaults.language'),
      formattingLocale: asString(defaults['formattingLocale'], 'defaults.formattingLocale'),
    },
  }
}

/** The catalogue, parsed once when this module is first imported. */
export const catalogue: Catalogue = parse(raw)

/**
 * English, or whatever the catalogue says the default is.
 *
 * Every resolver ends here, and every language falls back to it for a key it is
 * missing, so a raw key never reaches a user.
 */
export const FALLBACK_LANGUAGE = catalogue.defaults.language

/** The formatting locale a restaurant gets before anybody configures it. */
export const FALLBACK_FORMATTING_LOCALE = catalogue.defaults.formattingLocale

/** Looks a language up by its code. */
export function findLanguage(code: string): CatalogueLanguage | undefined {
  return catalogue.languages.find((language) => language.code === code)
}

/**
 * Whether this code is a language the platform actually offers.
 *
 * The gate on anything read from outside: a code left in `localStorage` by an
 * older build, or a column set before a language was removed. A stored code
 * that is no longer in the catalogue falls back rather than rendering, because
 * the files behind it are gone.
 */
export function isKnownLanguage(code: string | null | undefined): code is string {
  return typeof code === 'string' && findLanguage(code) !== undefined
}

/**
 * Which way a language is written, for the `dir` attribute.
 *
 * An unknown code answers with the fallback's direction rather than throwing.
 * By the time anything asks this the language has already been resolved, and a
 * layout that fails to render is worse than one written the usual way.
 */
export function directionOf(code: string): Direction {
  return findLanguage(code)?.direction ?? findLanguage(FALLBACK_LANGUAGE)?.direction ?? 'ltr'
}
