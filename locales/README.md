# The shared language catalogue

`catalogue.json` is the single list of what languages this platform has. Both sides read this one
file, so they cannot drift apart:

- **The API** compiles it in with `include_str!` and parses it once at startup, into the validator
  behind `LanguageCode` and `FormattingLocale`. A malformed catalogue fails the boot rather than
  the first request.
- **The web app** imports it directly. It drives the switcher's list, the lazy loader's known
  codes, and the `direction` that goes on the root element.
- **`pnpm locales`** checks that every `code` here has a folder in `web/src/locales/`, and that
  every namespace file in that folder carries every key the English one does.

## Adding a language

1. Add an entry to `languages` here.
2. Create `web/src/locales/<code>/` and copy every namespace file from `web/src/locales/en/`.
3. Translate them, keeping every key.
4. Run `pnpm locales`.

No migration, no schema change, no component change. That is the point (spec 0005, AC-4).

`direction` is carried per language even though both shipped languages are `ltr`. It feeds the
`dir` attribute on the root element, which together with the lint rule against physical direction
utilities is the whole cost of being ready for a right to left language later.

Reasoning: [docs/specs/0005-language-and-text-foundation/index.md](../docs/specs/0005-language-and-text-foundation/index.md).

# The shared country list

`countries.json` is the single list of countries a restaurant may register in, and the settings it
starts with. Same shape of arrangement as the catalogue above: one file, read by both sides.

- **The API** compiles it in with `include_str!`, behind `CountryCode`. Registration reads the row
  for the country the owner picked and writes its currency, decimals, timezone, default language,
  and formatting locale onto the new restaurant. Nothing else derives those five values.
- **The web app** imports it directly to build the country choice on the registration form. There
  is deliberately no endpoint for it.

## Adding a country

1. Add an entry to `countries` here.
2. Its `formattingLocale` must already be in `catalogue.json`'s `formattingLocales`, and its
   `defaultLanguage` must be one of its `languages`. A test refuses the file otherwise, because a
   country could otherwise hand a new restaurant a locale the app cannot format with.
3. Its `defaultTimezone` must be a real IANA zone name, and its `currencyCode` an upper case ISO 4217 code.

Reasoning: [docs/specs/0006-accounts-restaurants-and-roles/index.md](../docs/specs/0006-accounts-restaurants-and-roles/index.md).
