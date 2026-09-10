# 0005. Language and text foundation

**Date**: 2026-09-02
**Status**: Accepted

## Summary

This decision fixes where every word on every screen comes from, and how numbers, money, dates,
and times are written. Two languages ship, English and Hindi, and adding a third is one catalogue
entry plus one folder of files, with no database change and no code change. Which language a
person sees is resolved from a personal setting on their staff row, falling back to a default on
their restaurant, except on the kitchen screen, which is a shared appliance and always follows the
restaurant. Number and date formatting is deliberately cut loose from the interface language and
follows the restaurant's own region instead, so an owner in India reading English still sees
rupees grouped the Indian way on their own revenue figures. Three automatic gates keep it honest:
a lint error on any literal string in the interface, a build failure when a translation file is
short a key, and a development only fake language that makes hard coded text and cramped layouts
visible before a translator ever sees them.

Reasoning and options: see [rationale.md](rationale.md).

## Requirements

**User stories**:

- As a waiter who reads Hindi, I want the whole app in Hindi, so that I am not translating an
  English button in my head during a rush.
- As a restaurant owner, I want my revenue figures and my bills in my own country's number and
  date format, so that they match what my accountant expects, whatever language my staff read.
- As a chef, I want the kitchen screen to stay in one language across a shift handover, so that
  the screen I have learned to read at a glance does not change under me.
- As a waiter picking up whichever phone is free, I want my language to follow me, so that I do
  not reset it every time I borrow a different device.
- As an engineer building slices 1 through 6, I want the text and formatting rules already fixed,
  so that I am never adding a screen with English baked into it that somebody has to unpick later.

**Acceptance criteria** (the contract, each one independently checkable):

- **AC-1**: No user facing string is written into a component. Every visible word, every
  placeholder, every `alt`, `title`, and `aria-label` comes from a translation file through
  `t()`, and a literal string in the interface fails lint rather than review.
- **AC-2**: Two real languages ship, English (`en`) and Hindi (`hi`). Every key present in the
  English files is present in the Hindi files, and a gap fails continuous integration.
- **AC-3**: A key missing at runtime falls back to the English text. A raw key is never rendered
  to a user in any language.
- **AC-4**: Adding a language is one entry in the shared catalogue plus one folder of namespace
  files. It needs no migration, no schema change, and no change to any component.
- **AC-5**: Translation files load on demand, split per language and per namespace. An English
  session downloads no Hindi file, and the waiter surface downloads no admin or kitchen namespace.
- **AC-6**: The interface language resolves as the staff member's own `language` when set, else
  the restaurant's `default_language`. The kitchen surface ignores the personal setting and always
  uses `default_language`.
- **AC-7**: A device nobody has touched opens the sign in screen in English, whatever the browser
  reports. An explicit choice made from the sign in screen is remembered on that device and is
  what it opens in next time.
- **AC-8**: Switching language re draws the screen in place. There is no page reload, the live
  event stream stays open, and the TanStack Query cache is neither cleared nor refetched.
- **AC-9**: When the files for the chosen language fail to load, the language does not change, the
  preference is not saved, and the person is told the switch failed. A half translated screen is
  never shown.
- **AC-10**: The `lang` and `dir` attributes on the root element always match the active language,
  and the browser tab title is translated with everything else.
- **AC-11**: Text the restaurant typed (dish names, category names, table labels, line notes, void
  reasons) is never translated, and is rendered carrying the restaurant's own language and an
  automatic direction, so a screen reader pronounces it correctly inside a page declared as
  another language (WCAG 2.2 AA, Language of Parts).
- **AC-12**: Money, numbers, dates, and times are formatted from the restaurant's
  `formatting_locale`, never from the interface language, and always with Latin digits. A money
  amount is formatted from the exact decimal the API returned, never from a float and never from a
  server supplied display string, using the bill's own currency code and decimal count where the
  amount sits on a bill and the restaurant's own where it does not.
- **AC-13**: Every displayed timestamp is converted to the restaurant's `timezone`, never the
  device's, which is the same rule spec [0003](../0003-core-data-model/index.md) already fixed for
  which local day a bill belongs to.
- **AC-14**: Devanagari renders in a font chosen for it rather than whatever the device happens to
  have. A session that never renders a Devanagari character downloads no Devanagari font file.
- **AC-15**: `restaurants.default_language`, `restaurants.formatting_locale`, and `staff.language`
  exist with the types and defaults in the data model below, and each is validated in Rust against
  the shared catalogue on write. A code that is not in the catalogue is refused with a `400`, not
  stored and silently ignored later.
- **AC-16**: A development only pseudo language is selectable while developing, is generated from
  the English files rather than maintained by hand, and is absent from a production build.
- **AC-17**: The language switcher appears in the admin shell and the waiter shell, and is absent
  from the kitchen shell.

## Decision

**Chosen option**: Option 2: keep i18next, split by surface, and pull formatting away from
language.

Every user facing string lives in a JSON namespace file loaded on demand by i18next; which
language a person sees is resolved from two new database columns with a fixed precedence rule;
and money, number, date, and time formatting is driven by a third column that is deliberately
independent of the interface language, using only the browser's built in `Intl` formatters.

**Implementation skills**: `react-i18next` (`yildizberkay/skills`, `.claude/skills/react-i18next/`) ·
`tailwind-4-docs` (`lombiq/tailwind-agent-skills`, `.claude/skills/tailwind-4-docs/`) ·
`vitest` (`antfu/skills`, `.claude/skills/vitest/`) ·
`sqlx-postgres` (`daiki48/dotfiles`, `.claude/skills/sqlx-postgres/`) ·
`rust-best-practices` (`apollographql/skills`, `.claude/skills/rust-best-practices/`)

## Rationale

Reasoning, the options weighed, and what each one costs: see [rationale.md](rationale.md).

## Feature design

### The shared catalogue

One committed file, `locales/catalogue.json` at the repository root, is the single list of what
languages exist. Both sides read it, so they cannot drift:

```json
{
  "languages": [
    { "code": "en", "englishName": "English", "nativeName": "English", "direction": "ltr" },
    { "code": "hi", "englishName": "Hindi", "nativeName": "हिन्दी", "direction": "ltr" }
  ],
  "formattingLocales": ["en-US", "en-GB", "en-IN", "hi-IN"],
  "defaults": { "language": "en", "formattingLocale": "en-US" }
}
```

- **Rust** compiles it in with `include_str!` and parses it once at startup into the validator
  behind the two newtypes below, so a malformed catalogue fails the boot rather than the first
  request, which is the rule root `AGENTS.md` already sets for configuration.
- **The web app** imports it directly. It drives the switcher's list, the lazy loader's known
  codes, and the `direction` that goes on the root element.
- **A parity script** checks that every `code` in the catalogue has a matching
  `web/src/locales/<code>/` folder, and that every namespace file in it carries every key the
  English one does.

`direction` is carried per language from day one even though both shipped languages are `ltr`.
That field, plus the `dir` attribute it feeds and the lint rule feature 5 already installed
against physical direction utilities, is the whole cost of being ready for a right to left
language later. Adding one then is a catalogue entry and a folder, not a sweep across every screen.

### Data model sketch

Three additive columns in a new migration, `api/migrations/0003_language_and_formatting.sql`. No
new tables, no new relationships, no change to any existing column.

| Table | Column | Type | Null | Default | Meaning |
|---|---|---|---|---|---|
| `restaurants` | `default_language` | `text` | not null | `'en'` | The kitchen surface's language, the language printed documents use, and the fallback for any staff member with no personal setting |
| `restaurants` | `formatting_locale` | `text` | not null | `'en-US'` | Drives money, number, date, and time formatting. Independent of interface language on purpose |
| `staff` | `language` | `text` | null | `null` | Personal override. Null means "use the restaurant's default", so a staff member created by feature 10 needs no value |

**Constraints**: a `char_length` bound only (`default_language` and `staff.language` at most 8,
`formatting_locale` at most 35). No enum type and no check constraint listing codes. Validation is
in Rust against the catalogue, exactly as spec [0003](../0003-core-data-model/index.md) already
does for `restaurants.timezone` and for the same reason: the allowed set is owned by the
application, and pinning it into the schema means a migration every time a language is added,
which is the one thing **AC-4** forbids.

**Tenant scoping**: unchanged and inherited. `staff` already carries the composite
`(restaurant_id, id)` key spec 0003 fixed, so a language setting can never be read or written
across a restaurant boundary.

**New Rust types** in `api/src/domain/language.rs`:

- `LanguageCode`: a newtype whose only constructor validates against the catalogue's `languages`.
- `FormattingLocale`: a newtype whose only constructor validates against `formattingLocales`.

Both live in `domain`, so no `sqlx` or `axum` type touches them, per the layer rule.

### State transitions

None. A language setting has no lifecycle; it is a value that is read, written, or absent.

### API surface

**This feature adds no endpoint.** That is the decision, not an omission.

The scope order puts this feature before feature 7 (accounts, restaurants, and roles), so there is
no session to hang a "save my preference" endpoint on. `RestaurantScope` is still the development
only placeholder that refuses every request outside development, so any endpoint written now would
be untestable in a real setting and rewritten by feature 7 anyway.

| Endpoint | Method | Key inputs | Key outputs | Auth | Key errors |
|---|---|---|---|---|---|
| none in this feature | | | | | |

What feature 7 owes, written here so it is not rediscovered:

| Owed to feature 7 | Shape |
|---|---|
| The sign in response carries the resolved settings | `defaultLanguage`, `formattingLocale`, `timezone`, `currencyCode`, `currencyDecimals`, and the signed in person's `language` |
| A write path for the personal setting | `PATCH /api/me` with `language: string \| null`, `400` on a code outside the catalogue, and it may write only the caller's own row |
| Deleting the two placeholders | `web/src/shared/session/current-restaurant.ts` and the settings placeholder this feature adds go together |

Until then the client reads the restaurant's settings from
`web/src/shared/session/restaurant-settings.ts`, a development only placeholder built in the same
shape as the existing `currentRestaurantId()`, returning a value only in development. It is
written so feature 7 replaces its body and nothing that consumes it changes.

### Value sourcing

| Action | Value produced or displayed | Source |
|---|---|---|
| Render any signed in screen | The interface language | `staff.language` when set, else `restaurants.default_language`, else `en`. Both arrive via feature 7's sign in response; until then, the settings placeholder. `localStorage` is **not** in this chain |
| Render the kitchen surface | The interface language | `restaurants.default_language` only, never `staff.language`. The surface is known from the route group |
| Render the sign in screen | The interface language | `localStorage` key `language`, set only by an explicit choice on that screen. Absent means `en`. The browser's own language is deliberately not consulted, and this store is read **only** while signed out |
| Render any screen | The list of offered languages, their native names, their direction | `locales/catalogue.json` |
| Render any screen | `<html lang>` and `<html dir>` | The active language code, and that language's `direction` from the catalogue |
| Render restaurant typed text | The `lang` and `dir` on the wrapping element | `restaurants.default_language` and `dir="auto"`, never the interface language |
| Display a money amount on a bill | The formatted string | The exact decimal string from the API, plus **that bill's own** `currency_code` and `currency_decimals` (copied onto the bill at close per spec 0003), plus `restaurants.formatting_locale`, all through `Intl.NumberFormat` pinned to Latin digits |
| Display any other money amount | The formatted string | Spec 0003 puts a currency only on `bills`. A menu price, an unbilled order line, and a payment row therefore format from `restaurants.currency_code` and `restaurants.currency_decimals` directly, with the same locale and digit rules. The bill's copy wins only where a bill exists, which is what keeps a reprinted old bill honest |
| Display a date or a time | The formatted string | The `timestamptz` from the API, converted with `restaurants.timezone`, formatted with `restaurants.formatting_locale` through `Intl.DateTimeFormat` |
| Display an elapsed duration | The spoken form beside the clock | The plural forms in the active language's `common` namespace, and `Intl.ListFormat` for the separator, replacing today's hard coded comma |
| Display a failed request | The sentence a user reads | The stable `error` code in the API's error body, mapped to a key in the `common` namespace. The English `message` beside it is for logs only, never rendered |
| Save a language choice, signed out | Where it is written | `localStorage` only. There is no person to attach it to |
| Save a language choice, signed in | Where it is written | `localStorage` (so the next sign in screen on this device opens in it) and `staff.language` once feature 7 exists. Writing both does not make the local copy authoritative: the signed in resolver never reads it |
| Print a bill or a kitchen ticket (feature 16) | The document's language | `restaurants.default_language`, never the language of whoever pressed print. Paper has no signed in reader |
| Serve the public marketing page (feature 19) | The page's language | Its own decision, out of scope here. It has no restaurant and no signed in person, so neither rule above reaches it |

### Key invariants

1. **A component never contains a user facing literal.** Enforced by lint (**AC-1**), and by the
   pseudo language catching what lint cannot see, such as a string assembled in a helper.
2. **English is the floor.** Every other language's files are checked against English in
   continuous integration, and any key that is still missing at runtime falls through to English.
   A raw key never reaches a user.
3. **Interface language and formatting locale are separate values and are never derived from each
   other.** Neither one is guessed from the other, from the currency, or from the timezone.
4. **Restaurant typed text is data, not copy.** It is never sent to a translation file, never
   translated, and always rendered exactly as typed.
5. **Digits are Latin in every language.** Every `Intl` formatter is constructed with the Latin
   numbering system pinned, so a price or a table number is legible to every reader of a bill.
6. **A language a user can pick is a language whose files exist.** The switcher is built from the
   catalogue, the catalogue is checked against the folders on disk in continuous integration, and
   a stored code outside the catalogue falls back rather than rendering. The same holds for a
   `formatting_locale` outside the catalogue: it falls back to the catalogue's default (`en-US`),
   never to an unformatted number.
7. **The kitchen surface never reads a personal setting**, even when a chef with one is signed in.
8. **There are two resolvers, not one chain, and browser storage belongs to only one of them.**
   `resolveSignedOutLanguage()` reads `localStorage`, else `en`. `resolveSignedInLanguage()` reads
   `staff.language`, else `restaurants.default_language`, else `en`, and on the kitchen surface
   skips `staff.language` entirely. It never reads `localStorage`, so a stale code left on a shared
   phone by the previous shift can never beat the signed in person's own setting. A signed in
   switch writes to both places; writing is not reading.

### Security model

- A language preference is not sensitive, carries no personal data beyond a language choice, and
  needs no audit log entry.
- Once feature 7 exists: a staff member may write only their own `staff.language`. Only a member
  with the `admin` role may write `restaurants.default_language` or `restaurants.formatting_locale`.
  Both are enforced server side, in the scoped transaction, never by hiding a control.
- `localStorage` holds a language code and nothing else. On a shared phone the next waiter inherits
  the last explicit choice, which is intended (a house phone in a Hindi speaking restaurant should
  stay in Hindi) and leaks nothing about who used it.
- No compliance scope is triggered.

### Configuration required

None. No new environment variable, no secret, no third party account. The catalogue is a committed
file, not configuration.

### Critical test scenarios

- Happy path: with Hindi chosen, every visible string on the admin and waiter shells renders from
  the Hindi files, the root element reads `lang="hi"`, and no English text remains. Verifies
  **AC-2**, **AC-6**, **AC-10**.
- Resolution: a chef with `staff.language = 'hi'` signed in on a restaurant whose
  `default_language` is `en` sees the kitchen surface in English and the same person's waiter
  surface in Hindi. Verifies **AC-6**.
- Sign in screen: a fresh browser reporting `hi-IN` in `navigator.languages` opens the sign in
  screen in English; after an explicit switch, a reload opens it in Hindi. Verifies **AC-7**.
- Failure case, loading: the Hindi namespace request rejects while a waiter has an open bill on
  screen. The language stays English, the screen is untouched, a toast reports the failure, and
  nothing is written to `localStorage`. Verifies **AC-9**.
- Failure case, missing key: a key deleted from the Hindi file renders the English text, not the
  key. The parity script fails on the same file. Verifies **AC-2**, **AC-3**.
- Failure case, live session: switching language while the event stream is open leaves the stream
  connected and fires no refetch. Verifies **AC-8**.
- Formatting: a restaurant with `formatting_locale = 'en-IN'`, `currency_code = 'INR'`, and
  `currency_decimals = 2` renders `123456.78` as `₹1,23,456.78` for a reader in English and
  identically for a reader in Hindi, in Latin digits both times. Verifies **AC-12**.
- Formatting, time: a bill closed at `2026-09-02T19:30:00Z` renders in the restaurant's timezone,
  not the test runner's, and the two are deliberately different in the test. Verifies **AC-13**.
- Mixed language: a dish named in Devanagari inside an English interface renders inside an element
  carrying the restaurant's language and `dir="auto"`, and the axe pass over that screen is clean.
  Verifies **AC-11**.
- Lazy loading: the built bundle contains no Hindi string, and the admin namespace is absent from
  the chunk the waiter surface loads. Verifies **AC-5**.
- Validation: an uncatalogued code is refused. This feature ships no endpoint, so the test calls
  the repository function directly against a real Postgres in a rolled back transaction, not over
  HTTP. The matching `PATCH` refusal is feature 7's to test once it has an endpoint. Verifies
  **AC-15**.
- Adding a language: a test adds a third catalogue entry with a folder of files and asserts the
  switcher offers it, with no migration and no component change. Verifies **AC-4**.

## Build plan

The project's approach is **Tracer Bullet**, so the first milestone is a thin thread through every
layer that genuinely works: one namespace, in both real languages, resolved from a real database
column, chosen by a person, with the root element following. Only then does it thicken. The
migration lands in that first slice rather than up front, because the resolution rule is exactly
what the thread has to prove.

**Milestone 1: the thread, top to bottom**

1. Write `locales/catalogue.json` with `en` and `hi`, and the parity script that checks each
   catalogue entry has a folder on disk. Satisfies **AC-4**.
2. Add migration `0003_language_and_formatting.sql` with the three columns, their defaults, and
   their length bounds. Satisfies **AC-15**.
3. Add `LanguageCode` and `FormattingLocale` in `api/src/domain/language.rs`, validating against
   the catalogue compiled in with `include_str!`, parsed once at startup so a bad catalogue fails
   the boot. Extend the restaurant and staff repository reads and writes to carry them, and refresh
   the committed `.sqlx` cache. Satisfies **AC-15**.
4. Add `web/src/shared/session/restaurant-settings.ts`, the development only placeholder shaped so
   feature 7 replaces its body, and delete `i18next-browser-languagedetector`, replacing it with
   the two resolvers of invariant 8, written as two named functions and not one shared chain:
   `resolveSignedOutLanguage()` reads `localStorage` then `en`; `resolveSignedInLanguage()` reads
   `staff.language` then `restaurants.default_language` then `en`, never `localStorage` and never
   `navigator.languages`. Satisfies **AC-6**, **AC-7**.
5. Rewrite `web/src/shared/i18n/index.ts` for on demand loading by language and namespace, with
   English preloaded as the fallback, re drawing in place on a change with no reload and no cache
   touch. Load through `import.meta.glob('/src/locales/*/*.json')` with lazy loading, not an ad hoc
   `import()` on a fully runtime path. This is load bearing, not a style note: Vite can only split
   a chunk per file when the pattern is statically analysable, and a fully dynamic path makes it
   bundle every match instead, which passes every visible test while quietly breaking **AC-5**.
   Satisfies **AC-3**, **AC-5**, **AC-8**.
6. Handle a failed load: keep the current language, do not write the preference, raise a toast
   through the existing toast store. Satisfies **AC-9**.
7. Set `lang` and `dir` on the root element from the active language and the catalogue's
   `direction`, and translate the tab title, replacing the hard coded one in `web/index.html`.
   Satisfies **AC-10**.
8. Build the `LanguageSwitcher` on the existing `Select`, mount it in the admin and waiter shells
   only, and force the kitchen surface to `default_language` regardless of who is signed in.
   Satisfies **AC-6**, **AC-17**.
9. Add `hi` files for the `common` namespace, machine translated and marked unreviewed at the top
   of the folder, proving the thread with real Devanagari rather than copied English. Satisfies
   **AC-2**.

**Milestone 2: every string moved into the split**

10. Split the existing `common.json` into `common`, `admin`, `waiter`, and `kitchen` namespaces
    mirroring the feature folders, and update the sixteen call sites that use `t()` today.
    Satisfies **AC-1**, **AC-5**.
11. Sweep the remaining literals lint will now flag: `alt`, `title`, `placeholder`, `aria-label`,
    and any string built in a helper rather than in JSX. Satisfies **AC-1**.
12. Extend the Hindi files to cover every namespace, keeping key parity with English. Satisfies
    **AC-2**.

**Milestone 3: formatting, cut loose from language**

13. Add `web/src/shared/format/` with memoised `Intl.NumberFormat`, `Intl.DateTimeFormat`,
    `Intl.RelativeTimeFormat`, and `Intl.ListFormat` instances, every one built from
    `formatting_locale` with the Latin numbering system pinned and, for dates and times, the
    restaurant's `timezone`. Satisfies **AC-12**, **AC-13**.
14. Add the money formatter taking an exact decimal string plus a currency code and decimal count,
    with tests covering Indian grouping, zero decimal currencies, and a negative amount. Satisfies
    **AC-12**.
15. Move `ElapsedTime`'s spoken form onto `Intl.ListFormat` and the language's plural forms,
    replacing the hard coded comma separator. Construct it with `type: 'unit'`: the default,
    `conjunction`, produces "12 minutes and 30 seconds", which is not what the current comma does
    and is not what a screen reader should say for a duration. Satisfies **AC-1**, **AC-12**.
16. Map the API's stable `error` codes to keys in the `common` namespace, and stop rendering the
    English `message` anywhere outside logs. Satisfies **AC-1**, **AC-3**.

**Milestone 4: the three gates and the font**

17. Install `eslint-plugin-i18next` and turn on `no-literal-string` for the interface sources,
    scoped so the design gallery's own sample text and the generated client stay legal. Check its
    flat config support first: this repo is on ESLint 9 through `tseslint.config(...)`, and that
    plugin is low activity. If it ships no flat config, do not drop the gate and do not shim it;
    write the rule as a `no-restricted-syntax` selector over `JSXText` and the text carrying
    attributes, the same pattern `web/eslint.config.js` already uses for physical direction
    utilities. Satisfies **AC-1**.
18. Wire the parity script into `pnpm check` so a short translation file fails continuous
    integration. Satisfies **AC-2**.
19. Add the pseudo language: generated from the English files at build time, offered in the
    switcher in development only, excluded from the production bundle and from the parity check.
    Satisfies **AC-16**.
20. Add `@fontsource/noto-sans-devanagari` and append it to `--font-sans` after Geist, so Geist
    keeps every Latin character and only Devanagari falls through to it. The `unicode-range` the
    package ships is what stops an English only session fetching the file at all. Check the type
    scale still holds at kitchen distance with Devanagari's taller line boxes, and adjust the line
    height token rather than the size if it does not. Satisfies **AC-14**.

**Milestone 5: mixed language text and the written down rules**

21. Add the wrapper the base components use for restaurant typed text, carrying the restaurant's
    language and `dir="auto"`, and route every existing render of restaurant owned data through it.
    Satisfies **AC-11**.
22. Extend `docs/design.md` with the text rules (namespaces, key naming, what is never translated,
    which locale formats what) and record the printed document and public page language rules for
    features 16 and 19. Satisfies **AC-1**, **AC-12**.

## Consequences

**Positive**:

- Every screen built from slice 1 onward is translated by construction. The retrofit this feature
  exists to avoid never becomes possible, because a literal string stops compiling the pull request.
- Adding a language really is dropping in files. One catalogue entry, one folder, no migration, no
  deploy ordering, nothing to coordinate.
- Money and dates stop being a language question. An owner in India reading English gets Indian
  grouping on their own figures, and a bill formats identically for every member of staff.
- Right to left costs almost nothing later. The `dir` attribute, the per language `direction`, and
  feature 5's lint rule against physical direction utilities are all in place; adding an RTL
  language is a catalogue entry and a review pass, not a sweep across every screen.
- The API stays a data API. It returns exact decimals and stable error codes and knows nothing
  about language, so no second translation system grows in Rust.

**Negative / tradeoffs**:

- Hindi ships machine translated. It is real Devanagari that proves length, script, and plural
  behaviour, and the wording is provisional until a native speaker reviews it. Anyone reading Hindi
  before that pass sees translation that is understandable rather than good.
- Every new string is now two edits, English and Hindi, and the build refuses the pull request that
  does only one. That is the point, and it is friction on every feature from here on.
- Twelve screens' worth of formatting now depends on a placeholder module until feature 7 lands.
  It returns a value only in development, so a formatting bug that only shows with real restaurant
  settings cannot surface until then.
- The lint rule will produce false positives, on a `data-testid`, on a class name, on a code
  string. Each one is a suppression with a reason beside it, and that is upkeep.
- A shared repository root file compiled into Rust with `include_str!` couples the API build to a
  path outside `api/`. The Docker build already runs from the repository root, so it works, and it
  is one more thing to remember when that build changes.
- No admin can change any of this yet. The three columns take their defaults and stay there until
  feature 7 sets them at registration, so a restaurant that is not English speaking has a gap
  between signing up and being configurable.

**Neutral**:

- `i18next-browser-languagedetector` is removed. The decision that a fresh device opens in English
  is exactly what that package exists to prevent, so keeping it configured to ignore the browser
  would be a dependency whose only job is to be switched off.
- Migration `0003` is additive and separate. Spec 0003's migration and the tests feature 4 wrote
  against it are untouched.
- One new runtime dependency (the Devanagari font package) and one new development dependency (the
  lint plugin). No date library: `Intl` is in every browser this project supports and already
  covers every language the catalogue could grow to.

## Follow-up

- [ ] The Hindi files need a native speaker's pass before launch, with particular attention to the
      kitchen wording, which is read at a glance under pressure. The unreviewed marker in the
      folder is what tracks this.
- [ ] Feature 7 owns the sign in response, the `PATCH /api/me` write path, the admin controls for
      `default_language` and `formatting_locale`, and deleting both development placeholders. The
      shapes it needs are in the API surface table above.
- [ ] Feature 16 (printing) inherits the rule that a kitchen ticket and a customer bill use
      `restaurants.default_language`, not the language of whoever pressed print.
- [ ] Feature 19 (public marketing page) has no restaurant and no signed in person, so neither
      resolution rule reaches it. It owns its own language decision, plus the search engine side
      of it (the `lang` attribute, alternate language links, and localised metadata).
- [ ] Feature 13 (kitchen display) should confirm the type scale still reads at three metres with
      Devanagari, whose taller line boxes were not in front of feature 5 when it set the scale.
- [ ] Agent Skills and MCP servers were offered for the three new tools (the Devanagari font
      package, the lint plugin, the pseudo language generator) and declined. Record the decline in
      root `AGENTS.md` under `Declined:` so nothing offers them again.
- [ ] `react-i18next` is already listed in `web/AGENTS.md`. Once this ships, that file's
      conventions section needs the namespace split, the catalogue, and the formatting rules, which
      is `/sync`'s job, not this spec's.
