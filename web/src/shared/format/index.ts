import { FALLBACK_FORMATTING_LOCALE } from '@/shared/i18n/catalogue'
import { restaurantFormatting } from '@/shared/session/identity'

/**
 * How money, numbers, dates, and times are written.
 *
 * Nothing here reads the interface language, and that is the decision rather
 * than an oversight. An owner in India reading the app in English still wants
 * rupees grouped the Indian way on their own revenue figures, and a bill has to
 * read identically to every member of staff whatever language each of them
 * happens to have chosen. So every formatter below is built from the
 * restaurant's `formatting_locale`, and a language switch changes not one digit
 * on screen.
 *
 * Two rules run through all of it.
 *
 * **Digits are Latin, always.** The locale itself carries `-u-nu-latn`, so
 * every `Intl` constructor here is pinned by construction rather than each one
 * remembering an option. A locale can otherwise decide to write a price in
 * Devanagari digits, and a table number or a total that half the staff cannot
 * read at a glance is worse than one written in a script that is not theirs.
 * The extension is also the only way to say it to `Intl.RelativeTimeFormat`,
 * whose options carry no field for it.
 *
 * **Times are the restaurant's, never the device's.** Every date formatter is
 * built with the restaurant's own IANA timezone. This is the same rule spec
 * 0003 fixed for which local day a bill belongs to, applied to what a screen
 * shows: a tablet somebody carried in from another country must not quietly
 * shift every timestamp on it.
 */

/**
 * The Unicode extension that pins the numbering system.
 *
 * Appended to the locale rather than passed as an option, so it reaches every
 * formatter below the same way. See the two rules at the top of this file.
 */
const LATIN_DIGITS = '-u-nu-latn'

/** What every formatter here is built from. */
export interface FormattingContext {
  /** The restaurant's `formatting_locale`. */
  locale: string
  /** The restaurant's IANA timezone. */
  timeZone: string
}

/**
 * Whether a locale is one `Intl` actually knows.
 *
 * A `formatting_locale` can outlive the catalogue entry that put it there, and
 * an unknown tag makes every `Intl` constructor throw. Falling back to the
 * catalogue's default is the only behaviour that still renders a number.
 */
function usableLocale(locale: string): string {
  try {
    const known = Intl.NumberFormat.supportedLocalesOf([locale]).length > 0
    return `${known ? locale : FALLBACK_FORMATTING_LOCALE}${LATIN_DIGITS}`
  } catch {
    return `${FALLBACK_FORMATTING_LOCALE}${LATIN_DIGITS}`
  }
}

/** The context every formatter on this screen is built from. */
export function formattingContext(): FormattingContext {
  const settings = restaurantFormatting()
  return {
    locale: usableLocale(settings.formattingLocale),
    timeZone: settings.timezone,
  }
}

/**
 * One `Intl` instance per distinct set of options, kept.
 *
 * Constructing an `Intl` formatter is expensive enough to matter: a kitchen
 * screen redraws a wall of tickets every second, and building a fresh formatter
 * per ticket per tick is measurable. The options are the key, so a screen that
 * asks for the same shape twice gets the same object.
 */
function memoise<O, T>(
  build: (locale: string, options: O) => T,
): (locale: string, options: O) => T {
  const cache = new Map<string, T>()

  return (locale, options) => {
    const key = `${locale}|${JSON.stringify(options)}`
    const existing = cache.get(key)
    if (existing !== undefined) return existing

    const made = build(locale, options)
    cache.set(key, made)
    return made
  }
}

const numberFormat = memoise(
  (locale: string, options: Intl.NumberFormatOptions) => new Intl.NumberFormat(locale, options),
)

const dateFormat = memoise(
  (locale: string, options: Intl.DateTimeFormatOptions) => new Intl.DateTimeFormat(locale, options),
)

const listFormat = memoise(
  (locale: string, options: Intl.ListFormatOptions) => new Intl.ListFormat(locale, options),
)

const relativeTimeFormat = memoise(
  (locale: string, options: Intl.RelativeTimeFormatOptions) =>
    new Intl.RelativeTimeFormat(locale, options),
)

/** A plain number, grouped the restaurant's way. */
export function formatNumber(value: number, options: Intl.NumberFormatOptions = {}): string {
  return numberFormat(formattingContext().locale, options).format(value)
}

/** Postgres `numeric` as it arrives over the wire: a sign, digits, a fraction. */
const EXACT_DECIMAL = /^-?\d+(?:\.\d+)?$/

/**
 * Whether this string is a decimal `Intl.NumberFormat` will format exactly.
 *
 * A type predicate and not a cast. `format` accepts a string only when the type
 * says it is numeric, and this is the check that earns that: a value that
 * passes really is what the signature promises, so nothing downstream is
 * trusting an assertion somebody wrote.
 */
function isExactDecimal(amount: string): amount is Intl.StringNumericLiteral {
  return EXACT_DECIMAL.test(amount)
}

/**
 * A money amount, from the exact decimal the API returned.
 *
 * The string is the point. Money is `numeric` in Postgres and `Decimal` in
 * Rust, and it arrives here as a decimal string precisely so that no float ever
 * touches it. `Intl.NumberFormat` accepts a string and formats it exactly, so
 * the value a customer is charged and the value printed on their bill are the
 * same value. Parsing it into a `number` first would round `1234567.85` to
 * something that is very nearly it, which is the whole failure this avoids.
 *
 * The currency is always supplied by the caller rather than assumed. On a bill
 * it is that bill's own copied `currency_code` and `currency_decimals`, which is
 * what keeps a bill reprinted next year honest after the restaurant has changed
 * currency. Everywhere else it is the restaurant's current one.
 *
 * @param amount the exact decimal string from the API, such as `"1234.50"`.
 * @param currencyCode the three letter code, such as `INR`.
 * @param decimals how many decimal places that currency uses.
 */
export function formatMoney(amount: string, currencyCode: string, decimals: number): string {
  if (!isExactDecimal(amount)) {
    // Not a decimal the API could have sent. Showing the raw value is wrong
    // looking but still the right number, which beats both a crash on a screen
    // somebody is working from and a silently rounded total.
    console.error(`Not an exact decimal, so it is shown unformatted: ${JSON.stringify(amount)}`)
    return amount
  }

  return numberFormat(formattingContext().locale, {
    style: 'currency',
    currency: currencyCode,
    minimumFractionDigits: decimals,
    maximumFractionDigits: decimals,
  }).format(amount)
}

/**
 * A money amount in the restaurant's own current currency.
 *
 * For anything that is not on a bill: a menu price, an unbilled order line, a
 * figure in a report. Spec 0003 puts a currency only on `bills`, so these have
 * nowhere else to read one from. A bill always uses [`formatMoney`] with its own
 * copy instead.
 */
export function formatRestaurantMoney(amount: string): string {
  const { currencyCode, currencyDecimals } = restaurantFormatting()
  return formatMoney(amount, currencyCode, currencyDecimals)
}

/** How a date or a time is written. */
export type DateStyle = 'date' | 'time' | 'dateTime'

const DATE_OPTIONS: Readonly<Record<DateStyle, Intl.DateTimeFormatOptions>> = {
  date: { dateStyle: 'medium' },
  time: { timeStyle: 'short' },
  dateTime: { dateStyle: 'medium', timeStyle: 'short' },
}

/**
 * A timestamp, in the restaurant's timezone.
 *
 * @param timestamp the `timestamptz` the API returned, as an ISO string.
 */
export function formatTimestamp(timestamp: string, style: DateStyle = 'dateTime'): string {
  const value = new Date(timestamp)
  if (Number.isNaN(value.getTime())) return ''

  const { locale, timeZone } = formattingContext()
  return dateFormat(locale, { ...DATE_OPTIONS[style], timeZone }).format(value)
}

/**
 * Joins parts of one thing into a phrase, the restaurant's way.
 *
 * `type: 'unit'` and not the default. The default, `conjunction`, produces
 * "12 minutes and 30 seconds", which is how you list two separate things.
 * Minutes and seconds are two parts of one duration, so `unit` is what says
 * "12 minutes, 30 seconds" and it is what a screen reader should be given.
 *
 * `style: 'long'` alongside it, because `narrow` drops the separator entirely
 * and gives "12 minutes 30 seconds". That is the pause a screen reader needs to
 * make two numbers sound like two numbers rather than one long one.
 */
export function formatUnitList(parts: string[]): string {
  return listFormat(formattingContext().locale, { type: 'unit', style: 'long' }).format(parts)
}

/** A time relative to now, such as "in 5 minutes". */
export function formatRelativeTime(value: number, unit: Intl.RelativeTimeFormatUnit): string {
  return relativeTimeFormat(formattingContext().locale, { numeric: 'auto' }).format(value, unit)
}

/**
 * What a person typed into a price box, as the plain decimal string the API
 * reads, or `null` when it is not one.
 *
 * Two decimal marks are accepted: the ASCII point, which every keyboard has,
 * and the formatting locale's own mark, so an owner in Germany can type `12,50`
 * the way they write it. Either becomes a point on the way out. Nothing else is
 * accepted, and in particular no grouping separator: `1,500` in an Indian
 * locale is a thousand and a half written with a comma the API cannot tell from
 * a decimal mark, so it is refused here rather than guessed at.
 *
 * Only ASCII digits. The formatting layer pins every number it writes to Latin
 * digits, so that is also what every screen shows back.
 *
 * `null` is not the refusal a person reads. The form sends what was typed
 * anyway and the API answers with the field code, because the API is the
 * authority on every rule a price has to pass.
 *
 * @param text what was typed.
 * @param locale the formatting locale; the restaurant's own by default.
 */
export function parseDecimalInput(text: string, locale?: string): string | null {
  const trimmed = text.trim()
  if (trimmed === '') return null

  const mark = decimalMarkOf(locale ?? formattingContext().locale)
  const sign = trimmed.startsWith('-') ? '-' : ''
  let unsigned = sign === '' ? trimmed : trimmed.slice(1)

  if (mark !== '.') {
    // One mark at most, whichever it is: `1.234,5` is a grouped number.
    if (unsigned.includes('.') && unsigned.includes(mark)) return null
    unsigned = unsigned.replace(mark, '.')
  }

  return /^\d+(?:\.\d+)?$/u.test(unsigned) ? `${sign}${unsigned}` : null
}

/** The character a locale writes between the whole part and the fraction. */
function decimalMarkOf(locale: string): string {
  const part = numberFormat(locale, {})
    .formatToParts(1.5)
    .find((piece) => piece.type === 'decimal')

  return part?.value ?? '.'
}
