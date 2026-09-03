import i18next, { type BackendModule, type ReadCallback, type Resource } from 'i18next'
import { initReactI18next } from 'react-i18next'

import englishShell from '@/locales/en/common.json'
import { surfaceForPath, type Surface } from '@/shared/surface'

import { catalogue, FALLBACK_LANGUAGE } from './catalogue'
import { DEFAULT_NAMESPACE, NAMESPACES, namespacesForSurface, type Namespace } from './namespaces'
import { pseudoAvailable, pseudoBundle, PSEUDO_LANGUAGE } from './pseudo'
import { resolveSignedOutLanguage } from './resolve'

/**
 * Translations, set up before the first screen renders.
 *
 * Three things are load bearing here.
 *
 * **Files load on demand, split per language and per namespace.** An English
 * session downloads no Hindi file, and a waiter's phone downloads no admin
 * vocabulary.
 *
 * **English is preloaded and is the floor.** The English shell file is bundled
 * with the app, so the first paint waits on nothing and every other language
 * has something to fall through to. A key missing from Hindi renders the
 * English words, never the key itself.
 *
 * **Switching language is all or nothing.** The files are fetched first and the
 * language changes only once every one of them has arrived, so a failed request
 * leaves the screen exactly as it was rather than half translated.
 */

/**
 * Every translation file, as a map of path to a function that fetches it.
 *
 * `import.meta.glob` and not a bare `import()` on a built up path, and the
 * difference is not a style note. Vite can only emit one chunk per file when
 * the pattern is statically analysable. A fully dynamic path makes it bundle
 * every match into the caller's chunk instead, which passes every visible test
 * while quietly downloading Hindi to an English session.
 *
 * The pattern is rooted at `/src` rather than written relative to this file, so
 * moving this module cannot silently widen or empty it.
 */
const FILES = import.meta.glob<Record<string, unknown>>('/src/locales/*/*.json', {
  import: 'default',
})

/**
 * The English shell file, imported rather than fetched.
 *
 * Statically, so it is fetched alongside the app rather than after it and the
 * first paint waits on nothing. Only this one file: every surface reads
 * `common`, so loading it eagerly costs nobody anything, whereas doing the same
 * to the surface namespaces would put the admin vocabulary on a waiter's phone
 * and undo the split entirely.
 *
 * Vite emits it as its own chunk, because the glob below names the same path.
 * That chunk is a static import of the entry, not a lazy one, and the glob's
 * entry re-exports it rather than copying it, so it is fetched with the app and
 * shipped exactly once. Checked in the built output, not assumed.
 */
const FALLBACK_BUNDLE: Record<string, unknown> = englishShell

/**
 * Every language code that may be selected.
 *
 * The catalogue's, plus the fake one while developing. i18next refuses a
 * language outside `supportedLngs` outright, so the pseudo language has to be
 * named here as well as offered in the switcher.
 */
const SUPPORTED = [
  ...catalogue.languages.map((language) => language.code),
  ...(pseudoAvailable ? [PSEUDO_LANGUAGE] : []),
]

/** Fetches one namespace file, or says which one was missing. */
async function load(language: string, namespace: string): Promise<Record<string, unknown>> {
  // The fake language has no file. It is generated from the English one every
  // time, which is what stops it ever falling behind English. In a production
  // build `pseudoAvailable` is the literal `false`, so the bundler drops this
  // branch and the generator with it.
  if (pseudoAvailable && language === PSEUDO_LANGUAGE) {
    return pseudoBundle(await load(FALLBACK_LANGUAGE, namespace))
  }

  const fetchFile = FILES[`/src/locales/${language}/${namespace}.json`]

  if (fetchFile === undefined) {
    throw new Error(`No translation file at src/locales/${language}/${namespace}.json`)
  }

  return await fetchFile()
}

/**
 * The bridge i18next asks for a namespace it does not have yet.
 *
 * This covers ordinary demand, such as walking onto a surface whose namespace
 * has not been needed before. A language change does not go through here: it
 * has an atomicity requirement this interface cannot express, and takes the
 * explicit path below instead.
 */
const backend: BackendModule = {
  type: 'backend',
  init: () => undefined,
  read(language: string, namespace: string, callback: ReadCallback) {
    load(language, namespace).then(
      (resources) => {
        callback(null, resources)
      },
      (error: unknown) => {
        callback(error instanceof Error ? error : new Error(String(error)), false)
      },
    )
  },
}

/**
 * Fetches every namespace a language needs, then hands them to i18next
 * together.
 *
 * Together is the whole point. `Promise.all` rejects on the first failure and
 * nothing is added, so a language whose files did not all arrive is never
 * partly installed. The caller is then free to leave the current language
 * exactly where it was.
 *
 * @throws if any of the files cannot be fetched.
 */
async function installBundles(language: string, namespaces: readonly Namespace[]): Promise<void> {
  const bundles = await Promise.all(
    namespaces.map(async (namespace) => [namespace, await load(language, namespace)] as const),
  )

  for (const [namespace, resources] of bundles) {
    // Deep merge, and overwrite. The pair matters on a reload of a file that is
    // already present: a shallow merge would leave a stale nested group behind.
    i18next.addResourceBundle(language, namespace, resources, true, true)
  }
}

/**
 * The language the very first paint uses.
 *
 * The signed out one, always, because at this point nobody has asked the server
 * who is looking: the app has not booted, so the identity has not arrived. That
 * is what somebody explicitly chose on this device, else English, and never the
 * browser's own language. See `resolve.ts` for why that is deliberate.
 *
 * Once the identity does arrive, `useIdentityLanguage` moves the screen to the
 * language that person's own setting and their restaurant's default resolve to.
 * The two halves are separate on purpose: the sign in screen has to be readable
 * before anybody is signed in.
 */
const initialSurface: Surface = surfaceForPath(window.location.pathname)
const initialLanguage: string = resolveSignedOutLanguage()

/**
 * What is in memory before the first render.
 *
 * Always English, so there is a floor under every key. Plus the initial
 * language when it is something else, fetched here rather than after mounting,
 * so a Hindi session opens in Hindi rather than flashing English at it.
 */
const initialResources: Resource = {
  [FALLBACK_LANGUAGE]: { [DEFAULT_NAMESPACE]: FALLBACK_BUNDLE },
}

if (initialLanguage !== FALLBACK_LANGUAGE) {
  try {
    for (const namespace of namespacesForSurface(initialSurface)) {
      initialResources[initialLanguage] = {
        ...initialResources[initialLanguage],
        [namespace]: await load(initialLanguage, namespace),
      }
    }
  } catch (error) {
    // English is already in hand, so the app opens rather than failing to boot.
    // Reported, not swallowed: somebody has to know a language in the catalogue
    // has no files behind it.
    console.error(`Could not load the ${initialLanguage} translations, opening in English.`, error)
    delete initialResources[initialLanguage]
  }
}

await i18next
  .use(backend)
  .use(initReactI18next)
  .init({
    lng: initialLanguage in initialResources ? initialLanguage : FALLBACK_LANGUAGE,
    fallbackLng: FALLBACK_LANGUAGE,
    supportedLngs: SUPPORTED,
    // `hi` means `hi`, never a hunt for `hi-IN` that has no file behind it.
    load: 'currentOnly',
    ns: [DEFAULT_NAMESPACE],
    defaultNS: DEFAULT_NAMESPACE,
    resources: initialResources,
    // Says out loud that `resources` above is a floor rather than the whole
    // set, so i18next still asks the backend for what is missing instead of
    // treating a bundled language as complete.
    partialBundledLanguages: true,
    interpolation: {
      // React escapes for us, so i18next doing it again would double escape.
      escapeValue: false,
    },
  })

/**
 * Whether a language is already fully in memory for this surface.
 *
 * Used to keep a switch back to a language somebody already used instant, and
 * to keep the failure path honest: a language that is in hand cannot fail.
 */
export function isLanguageReady(language: string, surface: Surface): boolean {
  return namespacesForSurface(surface).every((namespace) =>
    i18next.hasResourceBundle(language, namespace),
  )
}

/**
 * Switches the interface language, or leaves it exactly where it was.
 *
 * The files are fetched first. Only when every one of them has arrived does the
 * language actually change, which is what stops a screen going half English and
 * half Hindi when one request fails on a phone in a basement kitchen.
 *
 * Nothing here reloads the page. React re draws in place, the live event stream
 * stays open, and the query cache is neither cleared nor refetched: none of the
 * data is language dependent, and throwing it away would empty a waiter's open
 * bill on screen to change a word in the header.
 *
 * @throws if the files for that language could not be fetched. The language is
 * unchanged when it throws, and the caller decides what to say about it.
 */
export async function changeLanguage(language: string, surface: Surface): Promise<void> {
  if (language === i18next.resolvedLanguage) return

  if (!isLanguageReady(language, surface)) {
    await installBundles(language, namespacesForSurface(surface))
  }

  await i18next.changeLanguage(language)
}

/**
 * Loads every namespace in the active language.
 *
 * For tests, and only for tests. The app deliberately never does this: it is
 * the exact opposite of the split, and calling it from a screen would put the
 * kitchen's vocabulary on a waiter's phone. A test is not measuring what got
 * downloaded, though, and a component that suspends waiting for its namespace
 * renders a fallback instead of itself, which fails assertions for a reason
 * that has nothing to do with what they check.
 */
export async function preloadAllNamespaces(): Promise<void> {
  await installBundles(i18next.resolvedLanguage ?? FALLBACK_LANGUAGE, NAMESPACES)
}

export default i18next
