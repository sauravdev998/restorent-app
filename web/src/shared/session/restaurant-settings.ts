import {
  FALLBACK_FORMATTING_LOCALE,
  FALLBACK_LANGUAGE,
  isKnownLanguage,
} from '@/shared/i18n/catalogue'

/**
 * The restaurant's own settings, and the signed in person's own language.
 *
 * A placeholder, and written as one. Feature 7 (accounts, restaurants, and
 * roles) replaces the body of these two functions with a read from the sign in
 * response, and nothing that consumes them changes: the shapes below are exactly
 * what that response is specified to carry.
 *
 * Until then, development answers with defaults so the app runs with no setup,
 * and production answers with the same defaults because there is no session to
 * read anything from yet. That is honest rather than convenient: every screen
 * built between now and feature 7 formats through these, so a formatting bug
 * that only shows with real restaurant settings cannot surface until the real
 * settings arrive. It is written down in spec 0005's consequences as a known
 * cost of building this before accounts exist.
 *
 * Deleted together with `current-restaurant.ts` when feature 7 lands.
 */

/** What the restaurant has set, and every screen reads. */
export interface RestaurantSettings {
  /**
   * What the kitchen screen reads, what a printed bill is written in, and what
   * a member of staff with no personal setting sees. From
   * `restaurants.default_language`.
   */
  defaultLanguage: string
  /**
   * How money, numbers, dates, and times are written here. Deliberately not
   * derived from the language. From `restaurants.formatting_locale`.
   */
  formattingLocale: string
  /**
   * The restaurant's own IANA timezone. Every timestamp on a screen is
   * converted with it, never with the device's. From `restaurants.timezone`.
   */
  timezone: string
  /** What it charges in, for anything that is not on a bill. */
  currencyCode: string
  /** How many decimal places that currency uses. */
  currencyDecimals: number
}

/** What the signed in person has set for themselves. */
export interface StaffSettings {
  /**
   * Their own interface language, or `null` for "whatever the restaurant uses",
   * which is what a new account has. From `staff.language`.
   */
  language: string | null
}

/**
 * What development gets before feature 7 exists.
 *
 * The timezone is deliberately not the machine's. A formatter that happens to
 * agree with the device hides the entire class of bug this project cares about,
 * which is a timestamp written in the wrong place's clock.
 */
const DEVELOPMENT_SETTINGS: RestaurantSettings = {
  defaultLanguage: FALLBACK_LANGUAGE,
  formattingLocale: FALLBACK_FORMATTING_LOCALE,
  timezone: 'Asia/Kolkata',
  currencyCode: 'INR',
  currencyDecimals: 2,
}

/**
 * The restaurant's settings.
 *
 * Feature 7 replaces this body with the sign in response. The fallbacks stay,
 * because a settings read that fails must still leave a screen that renders.
 */
export function restaurantSettings(): RestaurantSettings {
  return DEVELOPMENT_SETTINGS
}

/**
 * The signed in person's own settings.
 *
 * `null` throughout until feature 7, because there is nobody signed in to have
 * a preference. A person with no personal language takes the restaurant's, which
 * is the same answer either way.
 */
export function staffSettings(): StaffSettings {
  return { language: null }
}

/**
 * The restaurant's language, guarded against a code that is no longer offered.
 *
 * A column can outlive a catalogue entry. When it does, the files behind that
 * code are gone, so falling back is the only thing that renders.
 */
export function restaurantLanguage(): string {
  const { defaultLanguage } = restaurantSettings()
  return isKnownLanguage(defaultLanguage) ? defaultLanguage : FALLBACK_LANGUAGE
}
