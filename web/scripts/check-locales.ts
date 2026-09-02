/**
 * The translation parity gate.
 *
 * English is the floor. Every language in the shared catalogue must have a
 * folder on disk, that folder must carry every namespace English carries, and
 * every one of those files must carry every key the English one does.
 *
 * A key missing at runtime falls back to English, so a gap is never visible to
 * a user and would otherwise sit there for months. This is what makes it a
 * build failure instead: the pull request that adds an English string and
 * forgets the Hindi one does not merge.
 *
 * Plurals are checked per language rather than key for key. English wants
 * `_one` and `_other`; a language with six cardinal categories wants six
 * suffixes and would fail a naive comparison for being correct. The categories
 * come from `Intl.PluralRules` for the language itself, so adding a language
 * needs nothing here.
 *
 * Run by `pnpm check`. Run it alone with `pnpm --filter web locales`.
 */
import { readFileSync, readdirSync, statSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const HERE = dirname(fileURLToPath(import.meta.url))
const CATALOGUE = resolve(HERE, '../../locales/catalogue.json')
const LOCALES = resolve(HERE, '../src/locales')

/** The reference language every other one is measured against. */
const REFERENCE = 'en'

/** Every cardinal category i18next may suffix a key with. */
const PLURAL_CATEGORIES = ['zero', 'one', 'two', 'few', 'many', 'other'] as const

interface CatalogueLanguage {
  code: string
  englishName: string
  nativeName: string
  direction: 'ltr' | 'rtl'
}

interface Catalogue {
  languages: CatalogueLanguage[]
  formattingLocales: string[]
  defaults: { language: string; formattingLocale: string }
}

type Json = string | number | boolean | null | Json[] | { [key: string]: Json }

function readJson(path: string): Json {
  return JSON.parse(readFileSync(path, 'utf8')) as Json
}

/** Every leaf in a namespace file, as the dotted path i18next looks it up by. */
function flatten(value: Json, prefix = ''): string[] {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    return [prefix]
  }

  return Object.entries(value).flatMap(([key, child]) =>
    flatten(child, prefix === '' ? key : `${prefix}.${key}`),
  )
}

/**
 * Splits a language's keys into the plain ones and the plural bases.
 *
 * `_other` is the marker. i18next requires it for every plural key in every
 * language, so a key carrying it is a plural and one that does not is a plain
 * string that happens to end in an underscore and a word.
 */
function partition(keys: string[]): { plain: Set<string>; plurals: Set<string> } {
  const plurals = new Set(
    keys.filter((key) => key.endsWith('_other')).map((key) => key.slice(0, -'_other'.length)),
  )

  const plain = new Set(
    keys.filter((key) => {
      const underscore = key.lastIndexOf('_')
      if (underscore === -1) return true
      const suffix = key.slice(underscore + 1)
      const base = key.slice(0, underscore)
      const isPluralForm = (PLURAL_CATEGORIES as readonly string[]).includes(suffix)
      return !(isPluralForm && plurals.has(base))
    }),
  )

  return { plain, plurals }
}

/** The suffixes this language's own grammar needs for a plural key. */
function categoriesFor(language: string): readonly string[] {
  return new Intl.PluralRules(language, { type: 'cardinal' }).resolvedOptions().pluralCategories
}

function namespacesIn(directory: string): string[] {
  return readdirSync(directory)
    .filter((entry) => entry.endsWith('.json'))
    .map((entry) => entry.slice(0, -'.json'.length))
    .sort()
}

function isDirectory(path: string): boolean {
  try {
    return statSync(path).isDirectory()
  } catch {
    return false
  }
}

function main(): void {
  const catalogue = readJson(CATALOGUE) as unknown as Catalogue
  const failures: string[] = []
  const warnings: string[] = []
  let checked = 0

  const referenceDirectory = join(LOCALES, REFERENCE)
  if (!isDirectory(referenceDirectory)) {
    console.error(
      `\nNo ${REFERENCE} folder at ${referenceDirectory}. There is nothing to check against.\n`,
    )
    process.exit(1)
  }

  const referenceNamespaces = namespacesIn(referenceDirectory)

  // A folder nobody declared is a language the switcher will never offer, which
  // means work that silently reaches no one.
  const declared = new Set(catalogue.languages.map((language) => language.code))
  for (const entry of readdirSync(LOCALES)) {
    if (isDirectory(join(LOCALES, entry)) && !declared.has(entry)) {
      failures.push(
        `src/locales/${entry}/ exists but ${entry} is not in locales/catalogue.json, so nothing can select it.`,
      )
    }
  }

  for (const language of catalogue.languages) {
    const directory = join(LOCALES, language.code)

    if (!isDirectory(directory)) {
      failures.push(
        `${language.code} is in the catalogue but has no src/locales/${language.code}/ folder.`,
      )
      continue
    }

    const present = new Set(namespacesIn(directory))
    const categories = categoriesFor(language.code)

    for (const namespace of referenceNamespaces) {
      if (!present.has(namespace)) {
        failures.push(`${language.code} is missing the ${namespace} namespace.`)
        continue
      }

      const reference = partition(flatten(readJson(join(referenceDirectory, `${namespace}.json`))))
      const target = new Set(flatten(readJson(join(directory, `${namespace}.json`))))

      if (language.code === REFERENCE) {
        checked += target.size
        continue
      }

      const required = [
        ...reference.plain,
        ...[...reference.plurals].flatMap((base) =>
          categories.map((category) => `${base}_${category}`),
        ),
      ]

      for (const key of required) {
        checked += 1
        if (!target.has(key)) {
          failures.push(`${language.code}/${namespace}.json is missing the key "${key}".`)
        }
      }

      for (const key of target) {
        if (!required.includes(key)) {
          warnings.push(
            `${language.code}/${namespace}.json has "${key}", which ${REFERENCE} does not.`,
          )
        }
      }
    }
  }

  for (const warning of warnings) {
    console.warn(`  warning: ${warning}`)
  }

  if (failures.length > 0) {
    console.error(`\nTranslation parity failed. ${String(failures.length)} problem(s):\n`)
    for (const failure of failures) console.error(`  ${failure}`)
    console.error(
      `\nEnglish is the floor: every language in locales/catalogue.json carries every key ${REFERENCE} does.\n`,
    )
    process.exit(1)
  }

  console.log(
    `Translation parity passed: ${String(checked)} keys across ${String(catalogue.languages.length)} languages and ${String(referenceNamespaces.length)} namespaces.`,
  )
}

main()
