# Verify: language and text foundation · spec 0005 · updated 2026-09-03

_Steps derived from spec 0005 acceptance criteria and from every row of its Value sourcing table.
`/check verify` runs these; `/test` locks the durable ones._

Some steps need the database up and migration `0003` applied:

```
pnpm db:up && pnpm migrate
```

## Commands

- [ ] `pnpm locales` → passes, naming the key count, the two languages, the four namespaces → AC-2
- [ ] delete any one key from `web/src/locales/hi/common.json`, run `pnpm locales` → fails naming that
      exact key and file; restore it → AC-2
- [ ] add a third entry to `locales/catalogue.json` with no folder, run `pnpm locales` → fails saying
      it has no folder; remove it → AC-4
- [ ] write a literal string into any `web/src/**/*.tsx` outside `src/app/design/`, run
      `pnpm --filter web lint` → fails on `i18next/no-literal-string`; revert → AC-1
- [ ] `pnpm --filter web test` → passes, including the 15 formatter tests and the 5 `RestaurantText`
      tests → AC-11, AC-12, AC-13
- [ ] `pnpm --filter web build`, then search `dist/assets/index-*.js` → contains no Hindi translation
      string, no `en-XA`, and no `Pseudo (development)`; it does contain `हिन्दी`, the switcher's own
      label → AC-5, AC-16
- [ ] in the same build, confirm the entry imports `common-*.js` (English shell) statically and every
      other namespace chunk only through `import(...)` → AC-5
- [ ] `cargo test --manifest-path api/Cargo.toml --test language` → all five pass → AC-15
- [ ] `cargo test --manifest-path api/Cargo.toml language::` → the domain unit tests pass, including
      the one pinning the catalogue defaults against migration `0003`'s column defaults → AC-15
- [ ] `pnpm sqlx:check` → no difference, so the committed cache matches the SQL → AC-15
- [ ] `docker buildx build --platform linux/arm64 -t restaurant-api -f api/Dockerfile .` from the
      repository root → builds, proving `include_str!` reaches `locales/catalogue.json` in the image
- [ ] `pnpm check` → green end to end

## UI / manual

- [ ] open `/admin`, switch the language to हिन्दी → every visible word on the shell and the screen
      is Devanagari, and no English remains → AC-2
- [ ] with Hindi active, inspect `<html>` → `lang="hi"` and `dir="ltr"`, and the browser tab title is
      the Hindi one, not `Restaurant operations` → AC-10
- [ ] switch the language while the system status screen shows the stream as connected → the screen
      re draws in place, the tab does not reload, the stream stays connected, and the event count
      does not reset → AC-8
- [ ] with the network throttled to offline, switch language → the language does not change, a toast
      says the switch failed, and `localStorage.language` still holds the previous code → AC-9
- [ ] delete a key from `web/src/locales/hi/common.json` while the dev server runs, reload in Hindi →
      the English words render for that key, never the raw key → AC-3
- [ ] open `/kitchen` → there is no language switcher in the header → AC-17
- [ ] open `/admin` and `/waiter` → the switcher is present on both → AC-17
- [ ] select `Pseudo (development)` → every translated string is bracketed, accented, and padded;
      anything still reading as plain English is a string that never went through `t()` → AC-16
- [ ] open `/design` in Hindi and step through all three densities → Devanagari renders in Noto Sans
      Devanagari (not a system fallback), and no vowel mark is clipped or colliding at any size,
      kitchen included → AC-14
- [ ] on `/design`, inspect the two restaurant typed names → each carries `lang` set to the
      restaurant's language and `dir="auto"`, whatever the interface language is → AC-11
- [ ] in an English only session, check the network panel → no Devanagari font file and no Hindi
      JSON chunk was fetched → AC-5, AC-14
- [ ] walk `/admin` → `/waiter` → `/kitchen` → the waiter session fetched no `admin` or `kitchen`
      namespace chunk before it was on that surface → AC-5

## Value sourcing

One per row of the spec's table, exercising the edge that breaks if the source is wrong.

- [ ] set `staff.language = 'hi'` for a chef whose restaurant's `default_language` is `en`, then look
      at that chef's waiter screen and their kitchen screen → waiter is Hindi, kitchen is English.
      This is the row a single shared resolver would get wrong → AC-6
- [ ] clear that `staff.language` back to `NULL` → both screens follow `restaurants.default_language`
      → AC-6
- [ ] set `localStorage.language = 'hi'` and sign in → the signed in screens ignore it entirely and
      follow the staff and restaurant columns. A stale code on a shared phone must never beat the
      signed in person's own setting → AC-6
- [ ] a browser reporting `hi-IN` in `navigator.languages`, with `localStorage` cleared → the signed
      out screen opens in English, not Hindi → AC-7
- [ ] switch language explicitly, reload → that device opens in the chosen language → AC-7
- [ ] set `formatting_locale = 'en-IN'`, `currency_code = 'INR'`, `currency_decimals = 2` → an amount
      of `123456.78` renders `₹1,23,456.78` for a reader in English and identically for a reader in
      Hindi, in Latin digits both times → AC-12
- [ ] change only the interface language, leaving `formatting_locale` alone → not one digit on screen
      changes → AC-12
- [ ] format a bill amount using a `currency_code` on the bill that differs from the restaurant's
      current one → the bill's own copy wins, so a reprinted old bill is unchanged → AC-12
- [ ] set the restaurant's `timezone` to `Asia/Kolkata` and the machine's clock to another zone → a
      bill closed at `2026-09-02T19:30:00Z` renders in the restaurant's timezone, not the runner's →
      AC-13
- [ ] set `formatting_locale` to a value not in the catalogue → figures format with `en-US` rather
      than rendering unformatted → AC-12
- [ ] make the API return a failed response, then read the sentence on screen → it comes from the
      stable `error` code mapped to a key, and the API's English `message` appears nowhere but the
      logs → AC-1, AC-3

## Acceptance-criteria coverage

- AC-1 lint gate step · literal string step · error code step
- AC-2 `pnpm locales` · deleted key step · Hindi shell step
- AC-3 missing key fallback step · error code step
- AC-4 catalogue entry without a folder step
- AC-5 built bundle step · static vs lazy import step · network panel step · surface walk step
- AC-6 three resolution steps (chef, cleared setting, stale `localStorage`)
- AC-7 fresh browser step · remembered choice step
- AC-8 switch with the stream open step
- AC-9 offline switch step
- AC-10 `lang`, `dir`, and tab title step
- AC-11 `RestaurantText` tests · `/design` inspection step
- AC-12 formatter tests · Indian grouping step · language independence step · bill currency step ·
  unknown locale step
- AC-13 formatter tests · timezone step
- AC-14 `/design` Devanagari step · font not fetched step
- AC-15 `api/tests/language.rs` · domain unit tests · `pnpm sqlx:check`
- AC-16 pseudo language step · production bundle step
- AC-17 kitchen absence step · admin and waiter presence step
