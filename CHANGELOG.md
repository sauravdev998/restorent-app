# Changelog

All notable changes to this project are documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Hindi as a second interface language, alongside English. Every surface reads in either one, and
  the switcher writes each language in its own script (`हिन्दी`, not "Hindi"), so somebody who
  cannot read the current language can still find their own (see spec 0005).
- A shared language catalogue at `locales/catalogue.json`, read by both sides: the web app imports
  it, and the API compiles it in and parses it once at boot. Adding a language is one entry plus one
  folder of translation files, with no migration and no schema change. A malformed catalogue refuses
  the API's boot rather than surfacing as a puzzling `400` on the first write.
- A language and a formatting locale stored per restaurant and per staff member, through migration
  `0003`: `restaurants.default_language`, `restaurants.formatting_locale`, and `staff.language`. A
  code outside the catalogue is refused with a `400` before any statement is prepared, so it can
  never be stored and silently ignored later.
- Formatting driven by the restaurant's own `formatting_locale`, `timezone`, and currency rather
  than by the reader's language. An owner reading Hindi still sees the figures an accountant in
  India expects, always in Latin digits. Money is formatted from the exact decimal the API returned,
  using the bill's own currency code where the amount sits on a bill.
- Translation files that load on demand, split per language and per namespace. An English session
  downloads no Hindi file, and the waiter surface downloads no admin or kitchen vocabulary.
- A `RestaurantText` wrapper for text the restaurant typed, such as dish and category names. It is
  never translated, and it carries the restaurant's own language and an automatic direction so a
  screen reader pronounces it correctly inside a page declared as another language (WCAG 2.2 AA,
  Language of Parts).
- Three gates in continuous integration and in the editor. `pnpm locales` fails when a key present
  in English is missing from Hindi, or when a catalogue entry has no folder behind it. The
  `i18next/no-literal-string` lint rule fails on a user facing string written into a component.
- A development only pseudo language, generated from the English files rather than maintained by
  hand, which brackets, accents, and pads every translated string. Anything still reading as plain
  English on a pseudo screen is a string that never went through `t()`. It is absent from a
  production build.
- Noto Sans Devanagari, loaded for Hindi and sitting after Geist in the stack, so a session that
  renders no Devanagari character downloads no Devanagari font file.
- A test suite covering the resolvers, the catalogue parse, the pseudo language, the error code
  mapping, and the switcher: 114 web tests and 6 API tests, taking automated coverage from four of
  the feature's seventeen acceptance criteria to fifteen.

### Changed

- Every user facing string moved out of the components and into four translation namespaces
  (`common`, `admin`, `waiter`, `kitchen`) per language. Nothing visible is written into a
  component any more, including placeholders and `alt`, `title`, and `aria-label` text.
- The interface language now resolves from the signed in person's own setting, falling back to the
  restaurant's default. The kitchen surface deliberately ignores the personal setting and always
  follows the restaurant, because it is a shared screen several chefs read across a shift handover.
- The browser's reported language is no longer consulted. A device nobody has configured opens in
  English, and an explicit choice made on that device is remembered for next time.
- Switching language redraws the screen in place. There is no page reload, the live event stream
  stays open, and the query cache is neither cleared nor refetched.
- A failed language switch now changes nothing at all. The files are fetched before the language
  moves, so a request that fails leaves the language where it was, does not save the preference,
  and says so in a toast, rather than showing a half translated screen.
- The `lang` and `dir` attributes on the root element and the browser tab title now follow the
  active language instead of staying English.
- Failed API responses are shown as a sentence mapped from the response's stable `error` code. The
  API's own English `message` is kept for the logs and is never rendered, so one untranslated
  English sentence can no longer land in the middle of an otherwise Hindi screen.
- The API's Docker build now runs from `/build/api` so the compiled in catalogue path resolves.
