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
- Registration in one short form. A restaurant name, the owner's name, an email address, a
  password, and a country create one restaurant, one `admin` staff row, and one session, all in a
  single transaction, and sign the owner straight in. The country decides the restaurant's currency
  code, currency decimals, timezone, default language, and formatting locale, read from
  `locales/countries.json`, so there is no settings pass before the product is usable. Any failure
  inside that transaction, a duplicate email address in particular, leaves nothing behind and comes
  back as a field error on the box that caused it (see spec 0006).
- Sign in and sign out with an email address and a password. The session is an opaque token of 32
  bytes from a cryptographic random source, sent in a cookie that is `HttpOnly`, `SameSite=Lax`,
  `Path=/`, carries no `Domain`, and is `Secure` everywhere except development. Only the token's
  SHA-256 is stored, so the session table is not a list of usable keys. Passwords are `argon2id`,
  hashed off the async runtime, and a password is trimmed once and then hashed, so registration,
  sign in, and a password change can never disagree about what was typed.
- A session looked up in the database on every single request, with no cache anywhere. Revoking a
  session, or letting it expire, is refused on the very next request rather than eventually. The
  session slides fourteen days ahead when it is used, at most one write every five minutes, and
  stops exactly at a ninety day ceiling that is written once at sign in and never moves. Signing
  out revokes exactly the session that asked, so a phone left in the kitchen does not sign out the
  office.
- Roles carried in the handler's own type signature. An endpoint restricted to a role refuses every
  other role with `403` before the handler body runs, and the restriction shows up in the generated
  OpenAPI document, so a missing role check is something you can see rather than something you have
  to remember.
- Surfaces routed by the role the session carries. Signing in lands an admin on `/admin`, a waiter
  on `/waiter`, and a chef on `/kitchen`, walking to a surface you do not hold redirects you to the
  one you do, and a signed out visitor reaches the sign in screen with the path they wanted kept,
  without any protected screen rendering first, not even for a frame.
- Sign in throttling counted in Postgres rather than in each process, so it holds across every
  container. Five failed attempts for one email address inside fifteen minutes answer `429` with a
  `Retry-After` header, alongside a much more generous bucket counted by client address. Both
  buckets cover registration as well as sign in, so learning that an address is taken cannot be
  used to harvest addresses at speed. The count and the attempt's own insert run under a
  transaction level advisory lock keyed on the address, so attempts arriving together cannot all
  read a count below the limit. A correct password empties that address's bucket, and nothing locks
  permanently.
- A same origin check on every mutating request. `POST`, `PATCH`, `PUT`, and `DELETE` are refused
  with `403` unless the `Origin` header's host matches the request's own `Host`, with
  `Sec-Fetch-Site: same-origin` accepted as equivalent proof where the browser sent it, and a
  request carrying neither header refused. It needs no configured value, because same origin is
  defined by the request itself. Safe methods are not checked.
- An account screen where anybody signs out, changes their own password, and sets their display
  name and personal language. Changing a password requires the current one, revokes every other
  session belonging to that person, and leaves the screen that did it signed in.
- An admin settings screen for the restaurant's name, address, timezone, default language, and
  formatting locale. A waiter or a chef asking for it gets `403` from the server as well as never
  seeing the link. A timezone that is not a real IANA zone, and a language or formatting locale
  absent from `locales/catalogue.json`, are refused. No money setting is writable here.
- Audit rows for registering a restaurant, changing a password, and editing restaurant settings,
  each carrying the actor, the entity, and the before and after values. No password hash appears in
  any of them.
- `pnpm db:seed`, which creates one restaurant and one admin account with the credentials written
  in `.env.example`. It says so and creates nothing when run a second time, and it refuses to run
  anywhere but development. The integration tests get their actor by registering and signing in
  through the real endpoints rather than by inserting rows.
- A session that stays alive while a screen is only watching. The live event stream re resolves and
  slides its session on every fifteen second heartbeat, so a kitchen screen left open all shift
  does not expire underneath the person reading it, and a revoked session stops receiving events
  within fifteen seconds. A `401` from anywhere clears the cached identity and sends the person to
  the sign in screen with their path kept for after.
- Two housekeeping sweeps that run at sign in and are scoped to the person signing in: their own
  sessions that died more than seven days ago, and the sign in attempts for their own address older
  than a day. No sign in ever writes on another address's behalf.
- The load balancer now accepts traffic only from CloudFront, with `CloudFront-Viewer-Address`
  forwarded to the API. The pair is one control, not two: the address throttle trusts that header,
  which is safe exactly as long as CloudFront is the only route to the listener. The prefix list id
  comes from CDK context, and its absence leaves the listener open with a loud warning at synth
  time rather than a stack that will not synthesise.

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
- Every request now knows which restaurant it is acting for because somebody signed in, not because
  a header said so. `GET /api/me` returns the signed in person, their role, and their restaurant's
  settings as one bundle, and registration and sign in return that same shape, so the app has one
  answer to "who is this" instead of three.

### Removed

- The two development placeholders every screen was running on: the `x-restaurant-id` header and
  the `?restaurant_id=` query parameter on the API, and `current-restaurant.ts` and
  `restaurant-settings.ts` on the web side. No code path in any environment can now name a
  restaurant that did not come from a resolved session.
