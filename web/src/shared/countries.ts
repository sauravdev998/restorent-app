import countriesFile from '../../../locales/countries.json'

/**
 * The countries a restaurant may register in.
 *
 * Imported from the repository's own `locales/countries.json`, the same file the
 * API compiles in with `include_str!`, exactly as `catalogue.ts` imports the
 * language catalogue. One file, both sides, so the registration form cannot
 * offer a country the API would refuse and the API cannot accept one the form
 * has never heard of.
 *
 * Deliberately not an endpoint. A list that never changes between deploys, that
 * both sides have to agree on, and that the form needs before anybody is signed
 * in, is a file rather than a request.
 */

/** One country, and the settings a restaurant registering there starts with. */
export interface Country {
  /** The ISO 3166-1 alpha-2 code, upper case. */
  code: string
  /** What to call it on the form. */
  englishName: string
  /** The ISO 4217 code the restaurant will charge in. */
  currencyCode: string
  /** How many decimal places that currency uses. */
  currencyDecimals: number
  /** The country's primary IANA timezone. */
  defaultTimezone: string
  /** What the restaurant will read in until somebody changes it. */
  defaultLanguage: string
  /** How it will write money, numbers, and dates. */
  formattingLocale: string
}

/** The parsed file. */
export const countries: readonly Country[] = countriesFile.countries

/** One country by its code, or nothing. */
export function countryByCode(code: string): Country | undefined {
  const wanted = code.trim().toUpperCase()
  return countries.find((country) => country.code === wanted)
}
