/**
 * The fake language, for development only.
 *
 * It is generated from the English files rather than maintained by hand, and it
 * exists to make two problems visible before a translator ever sees them.
 *
 * **Text that is not coming from a translation file.** Every translated word on
 * a pseudo screen is visibly mangled, so anything still reading in plain English
 * is a string somebody wrote into a component. That catches what the lint rule
 * cannot see: a sentence assembled in a helper, a word coming back from a
 * function, a string built at runtime.
 *
 * **Layouts that only fit English.** Real translations run longer than English
 * far more often than they run shorter, so the pseudo language pads every string
 * by about a third. A button that fits its label exactly today is a button that
 * wraps in German, and this is where that shows up rather than in a screenshot
 * from a restaurant.
 *
 * The accents are the third job: they prove the font actually has the glyphs and
 * that nothing downstream is quietly assuming plain ASCII.
 *
 * Absent from a production build. `import.meta.env.DEV` is replaced with the
 * literal `false` when Vite builds for production, so the bundler drops every
 * branch that guards on it and this module is never reached. It is also left out
 * of the parity check, because it has no file on disk to compare and is
 * generated from English by construction.
 */

/** The code the fake language is offered under. Not in the catalogue on purpose. */
export const PSEUDO_LANGUAGE = 'en-XA'

/**
 * What it is called in the switcher.
 *
 * Plain English, because the only person who ever sees this option is an
 * engineer looking for it.
 */
export const PSEUDO_LANGUAGE_NAME = 'Pseudo (development)'

/** Whether the fake language is available at all. */
export const pseudoAvailable: boolean = import.meta.env.DEV

/**
 * The substitutions.
 *
 * Every letter keeps its shape, so a mangled string is still readable enough to
 * find the screen it came from. A wholly unreadable pseudo language gets turned
 * off by the second day.
 */
const ACCENTED: Readonly<Record<string, string>> = {
  a: 'á',
  b: 'b́',
  c: 'ć',
  d: 'd́',
  e: 'é',
  f: 'f́',
  g: 'ǵ',
  h: 'h́',
  i: 'í',
  j: 'j́',
  k: 'ḱ',
  l: 'ĺ',
  m: 'ḿ',
  n: 'ń',
  o: 'ó',
  p: 'ṕ',
  q: 'q́',
  r: 'ŕ',
  s: 'ś',
  t: 't́',
  u: 'ú',
  v: 'v́',
  w: 'ẃ',
  x: 'x́',
  y: 'ý',
  z: 'ź',
  A: 'Á',
  B: 'B́',
  C: 'Ć',
  D: 'D́',
  E: 'É',
  F: 'F́',
  G: 'Ǵ',
  H: 'H́',
  I: 'Í',
  J: 'J́',
  K: 'Ḱ',
  L: 'Ĺ',
  M: 'Ḿ',
  N: 'Ń',
  O: 'Ó',
  P: 'Ṕ',
  Q: 'Q́',
  R: 'Ŕ',
  S: 'Ś',
  T: 'T́',
  U: 'Ú',
  V: 'V́',
  W: 'Ẃ',
  X: 'X́',
  Y: 'Ý',
  Z: 'Ź',
}

/** How much longer than English the fake language runs. */
const PADDING = 0.35

/** The character the padding is made of. Visibly filler, never mistaken for a word. */
const PADDING_CHARACTER = '·'

/**
 * Mangles one string, leaving its interpolations alone.
 *
 * `{{count}}` and `{{column}}` are i18next placeholders, not words. Mangling
 * one would break the interpolation rather than test it, and a pseudo screen
 * full of `{{ćóúńt́}}` teaches nobody anything.
 */
function mangle(value: string): string {
  const parts = value.split(/(\{\{[^}]*\}\})/)

  const accented = parts
    .map((part) => (part.startsWith('{{') ? part : [...part].map((c) => ACCENTED[c] ?? c).join('')))
    .join('')

  const padding = PADDING_CHARACTER.repeat(Math.ceil(value.length * PADDING))
  return `[${accented}${padding}]`
}

/** What a namespace file holds: strings, nested as deeply as somebody wrote it. */
type Bundle = { [key: string]: string | Bundle }

/**
 * Builds the fake language's bundle from the English one.
 *
 * Generated rather than maintained, so it can never fall behind English and
 * never needs a key adding to it by hand.
 */
export function pseudoBundle(english: Record<string, unknown>): Bundle {
  const out: Bundle = {}

  for (const [key, value] of Object.entries(english)) {
    if (typeof value === 'string') {
      out[key] = mangle(value)
    } else if (typeof value === 'object' && value !== null) {
      out[key] = pseudoBundle(value as Record<string, unknown>)
    }
  }

  return out
}
