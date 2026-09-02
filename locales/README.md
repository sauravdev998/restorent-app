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
