# 0006. Accounts, restaurants, and roles

**Date**: 2026-09-03
**Status**: Accepted

## Summary

This decision fixes who anybody is and what they are allowed to do. An owner registers a restaurant
in one short form and is signed in as its admin. Everyone signs in with an email address and a
password, and gets back a session that lives in a cookie the browser cannot read from JavaScript, is
looked up in the database on every single request, and can be switched off instantly. A person's
role travels in that lookup, and a handler that is admin only says so in its own type signature, so
forgetting a role check is a thing you can see rather than a thing you have to remember. This
feature also deletes the two development placeholders every screen has been running on, which means
from here on the app knows which restaurant it is talking about because somebody signed in, not
because a header said so.

Reasoning and options: see [rationale.md](rationale.md).

## Requirements

**User stories**:

- As a restaurant owner, I want to register my restaurant in one short form and be working
  immediately, so that trying the product costs me a minute, not an afternoon of settings.
- As a waiter, I want to stay signed in on the house phone across my shifts, so that I am not typing
  an email address on a phone keyboard in the middle of service.
- As a restaurant owner, I want a member of staff I have switched off to lose access at once rather
  than eventually, so that ending somebody's employment is not a fourteen day wait.
- As a chef, I want the kitchen screen to be the kitchen screen, so that I never land somewhere I
  cannot use with wet hands.
- As a restaurant owner, I want another restaurant on this platform to be physically unable to reach
  my orders and takings, so that signing up costs me nothing in confidentiality.
- As an engineer building features 8 to 20, I want a single, typed way to know who is asking and
  what they may do, so that no feature invents its own answer.

**Acceptance criteria** (the contract, each independently checkable):

- **AC-1**: Registration takes a restaurant name, the owner's name, an email address, a password,
  and a country. On success one restaurant row, one `staff` row with role `admin`, and one session
  all exist, and the response body is the same shape `GET /api/me` returns. The restaurant's
  currency code, currency decimals, timezone, default language, and formatting locale are exactly
  the values `locales/countries.json` names for that country. A country code is accepted in any
  case and stored upper case. An email address is stored exactly as the person typed it and matched
  case insensitively, so the bundle returns what they typed.
- **AC-2**: Registration is one transaction. Any failure inside it, a duplicate email address in
  particular, leaves no restaurant, no staff row, and no session, and a duplicate address returns a
  `400` whose body carries `fields: { "email": "already_taken" }`. A request missing a required
  field returns that same body shape with `fields: { "<name>": "required" }`, never the JSON
  extractor's own rejection body.
- **AC-3**: Sign in with a correct email address and password returns the same bundle as
  `GET /api/me` and sets a cookie that is `HttpOnly`, `SameSite=Lax`, `Path=/`, carries no `Domain`
  attribute, and is `Secure` in every environment except development. The cookie value is 32 bytes
  from a cryptographic random source; only its SHA-256 is stored, in `sessions.token_hash`.
- **AC-4**: A wrong password and an unknown email address produce the same status, the same error
  code, and the same field errors as each other, and an unknown address still costs one password
  hash so the two take comparable time.
- **AC-5**: A password is accepted only if it is at least 10 characters and at most 72 bytes after
  surrounding whitespace is trimmed. A password outside that returns a field error and creates
  nothing. The trimmed value is the one hashed, and it is the one every later verification runs
  against, so registration, sign in, and a password change can never disagree about what was typed.
  The stored value is an `argon2id` hash at the crate's default parameters, computed off the async
  runtime.
- **AC-6**: Every request that acts for a restaurant resolves its session from the cookie against
  the database, with no cache anywhere. A revoked session, an expired one, or one past its absolute
  ceiling is refused with `401` on the very next request, with no window.
- **AC-7**: A session slides: a request made more than five minutes after `last_seen_at` moves
  `expires_at` to fourteen days ahead and `last_seen_at` to now. `absolute_expires_at` is written
  once at sign in and never moves, and a session past it is refused even if it has been used every
  day.
- **AC-8**: An endpoint restricted to a role refuses every other role with `403` before the handler
  body runs, and the restriction is visible in the handler's own signature and in the OpenAPI
  document. A waiter and a chef each get `403` from every admin only endpoint.
- **AC-9**: `RestaurantScope`, the `x-restaurant-id` header, the `?restaurant_id=` query parameter,
  `web/src/shared/session/current-restaurant.ts`, and
  `web/src/shared/session/restaurant-settings.ts` are all gone. No code path in any environment can
  name a restaurant that did not come from a resolved session.
- **AC-10**: Both throttle buckets are counted in `login_attempts`, so they hold across every
  container rather than per process. After five failed attempts for one email address inside fifteen
  minutes, further attempts for that address are refused with `429` and a `Retry-After` header,
  while a correct sign in for a different address in the same restaurant is unaffected. A separate
  and much more generous bucket counts the same window by client address. Both buckets cover
  `/api/auth/register` as well as `/api/auth/sign-in`, so revealing that an address is taken cannot
  be used to harvest addresses at speed. The count and the attempt's own insert happen under a
  transaction level advisory lock keyed on the lowered address, so parallel attempts cannot all read
  a count below the limit before any of them commits. Nothing locks permanently and nothing needs an
  admin to unlock it.
- **AC-11**: The client address both the record and the address bucket use comes from the
  `CloudFront-Viewer-Address` header outside development and from the socket address in development,
  and the load balancer accepts traffic only from CloudFront, so the header cannot be forged by
  reaching the load balancer directly.
- **AC-12**: A mutating request (`POST`, `PATCH`, `PUT`, `DELETE`) is refused with `403` unless the
  host in its `Origin` header equals the request's own `Host` header, which is same origin by
  definition and needs no configured value. `Sec-Fetch-Site: same-origin` is accepted as an
  equivalent proof where the browser sent it. A mutating request carrying neither header is refused.
  Safe methods are not checked.
- **AC-13**: Signing out revokes exactly the session that made the request and clears the cookie.
  Every other session belonging to that person keeps working.
- **AC-14**: Changing your own password requires the current one, applies the same password rules,
  revokes every other session belonging to you, and leaves the session that made the request
  working. A wrong current password returns a field error and changes nothing.
- **AC-15**: An admin can change the restaurant's name, address, timezone, default language, and
  formatting locale. A waiter and a chef get `403`. A language code or formatting locale not present
  in `locales/catalogue.json` is refused, and so is a timezone that is not a real IANA zone. No
  money setting is writable here.
- **AC-16**: `PATCH /api/me` writes only the caller's own `staff` row and covers the personal
  language (which may be set to null, meaning follow the restaurant) and the display name. It can
  never write another person's row and it can never write a restaurant setting.
- **AC-17**: Registering a restaurant, changing a password, and editing restaurant settings each
  write exactly one `audit_log` row carrying the actor, the entity, and the before and after values.
  Registration's row names the restaurant as its entity, with a null `before` and an `after` holding
  the restaurant's fields plus the new admin's staff id, display name, and email address. No
  password hash appears in any value.
- **AC-18**: The live event stream re resolves its session on every fifteen second heartbeat, and
  slides it exactly as any other use of the session would, so a screen left open all shift with no
  other request cannot expire under the person watching it. The stream closes as soon as a resolve
  fails, so a revoked session stops receiving events within fifteen seconds. The browser's automatic
  reconnect then receives a `401`, which the client treats as signed out.
- **AC-19**: A `401` from any request clears the cached identity and sends the person to the sign in
  screen with the path they were on preserved. Signing back in returns them to that path. The event
  stream's fatal error, which is distinguished by `readyState`, uses the same path.
- **AC-20**: A signed in person who opens a surface their role does not hold is redirected to the
  surface it does hold, and the role decides where sign in lands them: admin on `/admin`, waiter on
  `/waiter`, chef on `/kitchen`. A signed out visitor to any surface reaches the sign in screen with
  no protected screen rendering first, not even for a frame.
- **AC-21**: Every screen this feature adds carries no hard coded user facing text, formats through
  the layer spec 0005 built, and meets the accessibility level spec 0004 set, including the two
  screens that render outside the application shell.
- **AC-22**: `pnpm db:seed` creates one restaurant and one admin account with credentials named in
  `.env.example`, and the integration tests obtain an actor by registering and signing in through
  the real endpoints rather than by inserting rows.
- **AC-23**: Signing in deletes that person's own `sessions` rows that expired or were revoked more
  than seven days ago, and the `login_attempts` rows for that same email address older than twenty
  four hours. Both sweeps are scoped to the person signing in: no sign in ever deletes another
  address's rows, so a routine sign in is never a platform wide write.

## Decision

**Chosen option**: Option 1: An opaque session token in a cookie, resolved against the database on
every request.

Identity is a row in `sessions` that the server can destroy at any moment: sign in issues 32 random
bytes in an httpOnly cookie, stores only their hash, and every request resolves that hash into a
staff member, a role, and the restaurant that scopes the rest of the transaction, with the role
requirement carried in the handler's own type.

**Implementation skills**: `axum-web-framework` (`manutej/luxor-claude-marketplace`,
`.agents/skills/axum-web-framework/`) · `rust-backend` (`windmill-labs/windmill`,
`.agents/skills/rust-backend/`) · `rust-best-practices` (`apollographql/skills`,
`.agents/skills/rust-best-practices/`) · `sqlx-postgres` (`daiki48/dotfiles`,
`.agents/skills/sqlx-postgres/`) · `postgresql-table-design` (`wshobson/agents`,
`.agents/skills/postgresql-table-design/`) · `react-router-data-mode` (`remix-run/agent-skills`,
`.agents/skills/react-router-data-mode/`) · `tanstack-query` (`tanstack-skills/tanstack-skills`,
`.agents/skills/tanstack-query/`) · `shadcn` (`shadcn/ui`, `.agents/skills/shadcn/`) ·
`react-i18next` (`yildizberkay/skills`, `.agents/skills/react-i18next/`)

## Rationale

Reasoning and options: see [rationale.md](rationale.md).

## Feature design

**Data model sketch**

Spec 0003 already created `restaurants`, `staff`, `sessions`, and `audit_log`. Migration `0004` adds
only what is listed here; everything else is unchanged.

| Entity | Feature 7 change | Notes |
|---|---|---|
| `restaurants` | add `country_code char(2) NOT NULL` | Check constraint `~ '^[A-Z]{2}$'`, same shape as the existing currency code check. Needs a default for the migration's sake only if rows exist; there are none, so it is added as `NOT NULL` outright |
| `staff` | add `last_sign_in_at timestamptz NULL` | Null means never signed in, which is what feature 10's staff list will show |
| `sessions` | add `absolute_expires_at timestamptz NOT NULL` | Written once at sign in, never moved. Check constraint `absolute_expires_at > expires_at` |
| `login_attempts` | new table | `id uuid PRIMARY KEY`, `email text NOT NULL` (stored lowered), `ip inet NULL`, `attempted_at timestamptz NOT NULL DEFAULT now()`. No `restaurant_id`, no foreign key, no row level security, granted directly to `app_api`. Indexes on `(email, attempted_at DESC)`, which serves both the email bucket and the sweep, and on `(ip, attempted_at DESC)` for the address bucket. A table comment records why it is unscoped |
| `resolve_session` | altered | Adds `AND sess.absolute_expires_at > now()`. Still `SECURITY DEFINER`, still owned by `auth_lookup`, still returns the same columns plus nothing new |
| `audit_log` | three new `action` values | `restaurant_registered`, `password_changed`, `restaurant_settings_updated`. No column change |

Nothing else is added. There is no password reset token table, no email verification flag, no
`user_agent` column, and no roles or permissions table: `staff.role` with its three values is the
entire authorisation model.

**State transitions**

A session: `active` → `expired` (either `expires_at` or `absolute_expires_at` passes) or `revoked`
(`revoked_at` set). Both end states are terminal; there is no renewal of a dead session, only a new
sign in. `active` → `active` on every use more than five minutes after `last_seen_at`, which moves
`expires_at` forward and never moves `absolute_expires_at`.

Revocation is written by exactly five events, and every one of them is one `UPDATE` inside the
transaction that made the change:

| Event | Revokes | Owned by |
|---|---|---|
| Sign out | that one session | this feature |
| Own password change | every other session of that person | this feature |
| Account deactivated | every session of that person | feature 10 |
| Role changed | every session of that person | feature 10 |
| Restaurant deactivated | every session in that restaurant | a later feature |

The last three have no code path yet. They are written here so feature 10 inherits the rule rather
than deciding it again.

**Request path and transaction boundaries**

This is the load bearing sequence, and it is written out because getting the order wrong fails
silently rather than loudly. `sessions` is a tenant scoped table under `FORCE ROW LEVEL SECURITY`,
so an `UPDATE` against it before `app.restaurant_id` is set matches zero rows and raises nothing.

Every request that carries the cookie runs these steps in this order:

1. **Resolve, unscoped.** `resolve_session(token_hash)` on a plain pooled connection. It is
   `SECURITY DEFINER` and owned by `auth_lookup`, so it needs no restaurant set and touches no
   policy of ours. No row means `401`, and nothing further happens.
2. **Check the role.** The `Actor<R>` extractor compares the resolved role against its marker type
   and returns `403` before the handler body is entered. No database work is done for a refusal.
3. **Slide, in its own scoped transaction, only when due.** When `last_seen_at` is more than five
   minutes old, the extractor opens a transaction scoped to the resolved `restaurant_id` and updates
   that one session row. This transaction commits on its own, separately from whatever the handler
   goes on to do: a handler that fails afterwards must not un slide a session that was genuinely
   used. The update asserts it affected exactly one row and logs loudly if it did not, because zero
   rows here means the scoping was set wrongly and is otherwise invisible.
4. **Run the handler.** The handler opens its own scoped transaction through
   `Database::begin_scoped` as every handler already does, and does its work there.

So a request costs one unscoped read always, one short scoped write at most once every five minutes,
and the handler's own transaction. The event stream does the same thing on each heartbeat, including
the slide, which is what keeps a screen that makes no other request alive.

**API surface**

| Endpoint | Method | Key inputs | Key outputs | Auth | Key errors |
|---|---|---|---|---|---|
| `/api/auth/register` | POST | `restaurantName:string` (req), `displayName:string` (req), `email:string` (req), `password:string` (req), `countryCode:string` (req) | the identity bundle, plus `Set-Cookie` | public, throttled | `400` with `fields.email=already_taken`, `400` with `fields.password=too_short`, `400` with `fields.countryCode=unknown_country`, `429` |
| `/api/auth/sign-in` | POST | `email:string` (req), `password:string` (req) | the identity bundle, plus `Set-Cookie` | public, throttled | `401 unauthenticated` (identical for a wrong password and an unknown address), `429` with `Retry-After` |
| `/api/auth/sign-out` | POST | none | `204` | any signed in role | `401` |
| `/api/me` | GET | none | the identity bundle | any signed in role | `401` |
| `/api/me` | PATCH | `language:string\|null` (opt), `displayName:string` (opt) | the identity bundle | any signed in role, own row only | `400` with `fields.language=not_in_catalogue`, `401` |
| `/api/me/password` | POST | `currentPassword:string` (req), `newPassword:string` (req) | `204` | any signed in role, own row only | `400` with `fields.currentPassword=incorrect`, `400` with `fields.newPassword=too_short`, `401` |
| `/api/restaurant` | PATCH | `name` (opt), `address` (opt), `timezone` (opt), `defaultLanguage` (opt), `formattingLocale` (opt) | the identity bundle | `admin` only | `400` with a field error per rejected field, `403`, `401` |
| `/api/events` | GET | none | the live stream | any signed in role | `401`, and the stream closes when its session dies |

The identity bundle, returned identically by register, sign in, `GET /api/me`, `PATCH /api/me`, and
`PATCH /api/restaurant`, so the browser has one type and one cache entry:

```
{ staff:      { id, displayName, email, role, language },
  restaurant: { id, name, address, countryCode, currencyCode, currencyDecimals,
                timezone, defaultLanguage, formattingLocale } }
```

The country list the registration form needs is **not** an endpoint: the web app imports
`locales/countries.json` directly, exactly as it already imports `locales/catalogue.json`, and Rust
compiles the same file in with `include_str!`. One file, both sides, no drift.

Field errors ride on the existing error body as an optional member, so every current response is
unchanged:

```
{ "error": "invalid", "message": "for logs only", "fields": { "email": "already_taken" } }
```

The codes are a closed set: `already_taken`, `too_short`, `too_long`, `invalid_format`,
`unknown_country`, `not_in_catalogue`, `incorrect`, `required`. Each maps to a translation key on the
web side; the English `message` is never rendered.

`required` needs one piece of plumbing, because Axum's `Json` extractor refuses a body with a
missing field before any handler runs and answers in its own shape, which would be the one response
in the product that does not look like every other one. A thin `JsonBody<T>` extractor wraps it,
turns the deserialisation failure into the body above with the offending field named, and becomes
the extractor every handler in the project uses from here on.

**Value sourcing**

| Action | Value produced or displayed | Source |
|---|---|---|
| Register | The restaurant's currency code, currency decimals, timezone, default language, formatting locale | The row for `countryCode` in `locales/countries.json`. Its `formattingLocale` must be one of the `formattingLocales` in `locales/catalogue.json`, checked by a test, so a country can never derive a locale the app cannot format with |
| Register | The country list on the form | `locales/countries.json`, imported directly by the web app. No endpoint |
| Register | The owner's role | The literal `admin`. Registration is the only path that creates an admin without an admin |
| Register | The new restaurant, staff, and session identifiers | UUID v7, generated in Rust as spec 0003 established |
| Register, sign in | The cookie value | 32 bytes from the operating system's cryptographic random source, base64url encoded for transport. Never stored |
| Register, sign in | `sessions.token_hash` | SHA-256 of those raw bytes. A fast hash on purpose: the value is already high entropy, and the column is looked up by index on every request |
| Register, sign in | `expires_at` | Postgres `now() + interval '14 days'` |
| Register, sign in | `absolute_expires_at` | Postgres `now() + interval '90 days'` |
| Register, sign in, every request | Every timestamp compared or written | Postgres `now()`, never a Rust clock, so skew between containers cannot make one instance honour a session another has expired |
| Sign in | Whether the password matches | `find_staff_for_login(email)` from spec 0003, verified with `argon2id`. When it returns no row, a dummy hash is still verified so the timing does not answer the question |
| Sign in, register | Whether this attempt is throttled | Two counts over `login_attempts` inside the last fifteen minutes, one for the lowered email and one for the client address, both taken under a transaction level advisory lock on the lowered email so the count and the attempt's own insert cannot interleave. No in process state anywhere, so the answer is the same on every container |
| Sign in, register | The client address recorded and throttled on | The `CloudFront-Viewer-Address` header outside development, the socket peer address in development |
| Register | The stored email address | Exactly what was typed. Only the uniqueness index and every lookup lower it, so the bundle and the sign in screen show the person their own capitalisation |
| Register | The stored country code | `CountryCode::try_from`, which upper cases before it matches `locales/countries.json`, so a form or a script sending `us` is accepted |
| Any mutating request | What the `Origin` header is compared against | The request's own `Host` header. Same origin is the definition, so there is no configured origin to keep in step across three environments |
| Every scoped request | Which restaurant this transaction may see | `resolve_session(token_hash)`, whose `restaurant_id` goes straight into `Database::begin_scoped` |
| Every scoped request | Who the audit actor is | `staff_id` from that same `resolve_session` answer |
| Every scoped request | Whether the role is allowed here | `role` from that same answer, compared against the marker type in the handler's `Actor<R>` |
| Any screen | The interface language | Unchanged from spec 0005: `staff.language` when set, else `restaurants.default_language`, else `en`, and the kitchen surface always follows the restaurant. Both values now arrive in the identity bundle instead of the placeholder |
| Any screen | Money, dates, and elapsed times | Unchanged from spec 0005, formatted with `restaurant.formattingLocale` and `restaurant.timezone` from the identity bundle |
| Sign in | Where the person lands | `staff.role`: `admin` to `/admin`, `waiter` to `/waiter`, `chef` to `/kitchen`, unless a `next` path was preserved by a `401` redirect, in which case that path wins if the role may see it |
| Change own password | Which sessions survive | The `session_id` from the current request's own `resolve_session` answer; every other row for that `staff_id` is revoked |
| Restaurant settings | Whether a timezone is acceptable | Parsed as an IANA zone in Rust, the same validation spec 0003 specified for the column |
| Restaurant settings, `PATCH /api/me` | Whether a language or locale is acceptable | `locales/catalogue.json`, the single catalogue spec 0005 established |
| Any failed request | The sentence a person reads | The stable `error` code, and where present each `fields` code, mapped to a translation key. The English `message` is for logs |

**Key invariants**

1. **A restaurant never exists without an admin.** Registration creates both in one transaction; a
   failure creates neither.
2. **The raw session token exists in exactly two places**: the response's `Set-Cookie` header and
   the browser's cookie jar. It is never logged, never stored, and never returned in a body.
3. **`absolute_expires_at` is immutable after insert.** Only sign in writes it.
4. **A resolved session yields all four of restaurant, staff, session, and role, or none of them.**
   There is no way to hold a restaurant id without knowing who is acting in it, which is what makes
   every audit row have a real actor.
5. **Exactly two paths read across restaurants**, still, and this feature adds no third: spec 0003's
   `find_staff_for_login` and `resolve_session`. `login_attempts` is not a third path because it
   holds no tenant data, has no `restaurant_id`, and can answer nothing about any restaurant.
6. **Email identifies exactly one account platform wide** (spec 0003), and the database index, not
   an application check, is what enforces it.
7. **Nothing locks permanently.** Every throttle is a time window that clears itself.
8. **A role check happens before a handler body runs**, or the handler does not compile with a role
   requirement in its signature.
9. **The slide commits on its own.** It is a use of the session, not part of the handler's work, so
   a handler that fails afterwards leaves the slide in place. It also asserts it touched exactly one
   row, because zero rows means the scoping was wrong and Postgres will not say so.
10. **Neither throttle bucket lives in a process.** Both are counted in `login_attempts`, so two
    containers cannot each grant a caller the full allowance.

**Security model**

| Who | May read | May write |
|---|---|---|
| Nobody signed in | the sign in and registration screens, `locales/*.json` | register, sign in, both throttled |
| Any signed in role | their own restaurant's data, through the scoped transaction only | their own `staff` row's `language` and `displayName`, their own password, their own session's revocation |
| `admin` | the same, plus the restaurant's settings | the restaurant's name, address, timezone, default language, formatting locale |
| `waiter`, `chef` | the same restaurant scoped data | nothing on the restaurant row, nothing on another person's row |

Enforcement is in three independent layers, and none of them is a convention: the `Actor<R>`
extractor refuses the wrong role before the handler runs; `Database::begin_scoped` sets
`app.restaurant_id` so row level security applies to every query; and the composite foreign keys
from spec 0003 make a cross restaurant reference physically impossible.

The cookie is `HttpOnly` (so a cross site scripting bug cannot read the token), `SameSite=Lax` (so a
cross site form post does not carry it), `Path=/`, `Secure` outside development, and carries no
`Domain` attribute, which keeps it a host only cookie that no subdomain can receive. The `Origin`
check on mutating methods closes the top level navigation gap that `Lax` leaves.

**Compliance scope**: this feature creates the platform's first personal data, so GDPR style rules
apply to a staff member's name, email address, and password hash. Concretely: no password and no
session token is ever written to a log; a failed sign in logs the outcome and a truncated hash of
the lowered email address as a correlation value, never the address itself; a successful one logs
`staff_id` and `restaurant_id`; and the audit rows for a password change record who and when, never
the old or new hash. Neither erasure path spec 0003 designed is built here (blanking a `staff` row
belongs with feature 10, deleting a restaurant with whatever feature lets an owner leave), so an
erasure request today is a manual database operation, and that is written down rather than assumed.
Audit logging is not negotiable in this feature: it holds access control.

**Configuration required**

No new environment variable, and nothing new in `.env.example` except the seeded development
credentials. Session lifetimes, throttle limits, the cookie name, and the cookie's attributes are
named constants beside the code that uses them, with a test pinning their relationships the way the
timeout ladder in `api/AGENTS.md` already is. The cookie's `Secure` flag reads the existing
`Environment` value, so local http development works and production cannot ship an insecure cookie
by omission.

Two infrastructure prerequisites, both edits in `infra/`, which is not deployed yet:

- The CloudFront distribution must forward `CloudFront-Viewer-Address` to the origin, through an
  origin request policy.
- The load balancer's security group must accept traffic only from CloudFront's managed prefix
  list, or the header above can be forged by reaching the load balancer directly.

The `auth_lookup` database role already exists from spec 0003 and needs no change; migration `0004`
alters a function it owns, so the migration runs as the schema owner and re asserts ownership the
way `0002` did.

**Critical test scenarios**

- Happy path: register with a country, land signed in as admin with the country's settings applied,
  then reload and get the same bundle from `GET /api/me`, verifies **AC-1**, **AC-3**.
- Happy path: sign in on a second device, both sessions work, sign out on one, the other still
  works, verifies **AC-13**.
- Failure case: two concurrent registrations with the same email address, one succeeds and one
  returns `fields.email=already_taken` with no orphan restaurant row left behind, verifies
  **AC-2**.
- Failure case: an unknown email address and a wrong password return identical responses, and the
  unknown address takes comparable time because a dummy hash is still verified, verifies **AC-4**.
- Failure case: five failed attempts for one address inside the window return `429` with
  `Retry-After`, while a different address in the same restaurant signs in normally, and the same
  limit applies to registration, verifies **AC-10**.
- Failure case: several sign in attempts for one address issued in parallel do not all pass the
  count, because the advisory lock serialises the count and the insert, verifies **AC-10**.
- Failure case: the slide's `UPDATE` run without `app.restaurant_id` set matches zero rows, and the
  assertion catches it rather than the request succeeding with a session that never slid, verifies
  **AC-7**.
- Happy path: a live stream held open with no other request for longer than the sliding window keeps
  working, because its heartbeat slides the session, verifies **AC-7**, **AC-18**.
- Failure case: a session revoked while a live stream is open sees the stream close within one
  heartbeat, and the browser's reconnect receives `401`, verifies **AC-18**.
- Failure case: a session past `absolute_expires_at` is refused even though `expires_at` is in the
  future because it has been used daily, verifies **AC-7**.
- Auth and permission: a waiter's session receives `403` from `PATCH /api/restaurant` and is
  redirected away from `/admin` in the browser, verifies **AC-8**, **AC-20**.
- Auth and permission: `PATCH /api/me` cannot write another staff member's row even when that row's
  id is supplied, verifies **AC-16**.
- Auth and permission: a mutating request with a foreign `Origin`, and one with no `Origin` at all,
  are both refused with `403`, verifies **AC-12**.
- Auth and permission: with a valid session for restaurant A, every read returns only restaurant A's
  rows, proven against a real Postgres as `app_api` the way spec 0003's isolation tests are,
  verifies **AC-6**, **AC-9**.

## Build plan

Ordered by the project's Tracer Bullet approach: a thin thread that pierces every layer and works
before anything is thickened. The thread here is register, get a cookie, have one scoped request
resolve it, sign out. Nothing about throttling, roles, or settings exists until that thread runs end
to end, because every one of them is a thickening of a thread that has to work first.

**Milestone 1: the thread, top to bottom.** _(done)_

1. [x] Write `locales/countries.json` beside the language catalogue, covering the countries to serve,
   each naming its currency code, decimals, primary timezone, and a formatting locale that exists in
   `locales/catalogue.json`, plus the test that pins that relationship. Compile it into Rust with
   `include_str!` the way `domain/language.rs` does. Satisfies **AC-1**.
2. [x] Write migration `0004`: the four column additions, the `login_attempts` table with its grants,
   indexes, and table comment, and the altered `resolve_session`. Refresh the `.sqlx` cache.
   Satisfies **AC-1**, **AC-7**, **AC-10**.
3. [x] Add the domain newtypes that validate on construction, `EmailAddress`, `Password`, and
   `CountryCode`, following the existing `LanguageCode` pattern, plus the `argon2id` hashing and
   verification behind an application port, run off the async runtime. Satisfies **AC-5**.
4. [x] Add the session repository operations: create a session, resolve one, slide one, revoke one,
   revoke every other one for a person, and the sign in sweep. Satisfies **AC-6**, **AC-13**,
   **AC-23**.
5. [x] Build `POST /api/auth/register`, `POST /api/auth/sign-in`, `POST /api/auth/sign-out`, and
   `GET /api/me` with the identity bundle, the cookie handling through `axum-extra`, the `fields`
   member on the error body, and the `JsonBody<T>` extractor that turns a missing field into
   `fields.<name>=required` instead of the JSON extractor's own body. Add each to
   `presentation/openapi.rs`. Satisfies **AC-1**, **AC-2**, **AC-3**, **AC-4**, **AC-13**.
6. [x] Replace `RestaurantScope` with the `Actor` extractor, following the four step sequence in
   **Request path and transaction boundaries**: resolve unscoped, check the role, slide in its own
   committed scoped transaction when due, then let the handler open its own. Delete the development
   header and query parameter, and move every existing handler and the event stream onto it.
   Satisfies **AC-9**.
7. [x] Build the sign in screen and the root loader that resolves the identity before anything renders,
   and prove the thread by signing in and reaching a surface with real data. Satisfies **AC-3**,
   **AC-20**.

**Milestone 2: the session's whole life.** _(done)_

8. [x] Add the sliding refresh with its five minute floor and its exactly one row assertion, and the
   absolute ceiling check, with the constants and the test that pins their relationships. Satisfies
   **AC-7**.
9. [x] Re resolve and slide the session on the event stream's heartbeat, and close the stream when the
   resolve fails. Satisfies **AC-7**, **AC-18**.
10. [x] Handle `401` once in the typed client and in the `EventSource` error path, clearing the cached
    identity and preserving the path for the return. Satisfies **AC-19**.

**Milestone 3: roles, made structural.** _(done)_

11. [x] Make the extractor generic over a role marker so a handler names `Actor<Admin>` or
    `Actor<AnyRole>`, refusing before the body runs, and reflect the requirement in the OpenAPI
    document. Satisfies **AC-8**.
12. [x] Gate the three web route groups on the role and add the landing redirect, so a role that does
    not hold a surface is sent to the one it does. Satisfies **AC-20**.

**Milestone 4: standing in front of the door.** _(done)_

13. [x] Add both throttle buckets over `login_attempts`, email and client address, counted under a
    transaction level advisory lock on the lowered address, applied to `/api/auth/register` and
    `/api/auth/sign-in` alike, with a `Throttled` domain error mapping to `429` and `Retry-After`.
    No in process limiter. Satisfies **AC-10**.
14. [x] Source the client address from `CloudFront-Viewer-Address` outside development, and make the two
    `infra/` changes: forward the header, and restrict the load balancer to CloudFront's prefix
    list. Satisfies **AC-11**.
15. [x] Add the check on mutating methods comparing the `Origin` host against the request's own `Host`,
    accepting `Sec-Fetch-Site: same-origin` as an equivalent proof. Satisfies **AC-12**.

**Milestone 5: the rest of the surface.** _(done)_

16. [x] Build the registration screen, the account panel reachable from all three shells (change
    password and the personal language switcher spec 0005 needs a home for), and the admin
    restaurant settings screen, all through the router's actions and the primitives feature 5 built.
    Satisfies **AC-14**, **AC-15**, **AC-16**, **AC-21**.
17. [x] Build `PATCH /api/me`, `POST /api/me/password`, and `PATCH /api/restaurant`, with their
    validation against the catalogue and the IANA zone list, and the other session revocation on a
    password change. Satisfies **AC-14**, **AC-15**, **AC-16**.
18. [x] Write the three audit rows, with no hash in any value. Satisfies **AC-17**.
19. [x] Add the two scoped sweeps on the sign in path, sessions and login attempts, both limited to the
    person signing in. Satisfies **AC-23**.
20. [x] Add `pnpm db:seed` and the integration test fixture that registers and signs in through the real
    endpoints, and delete both web placeholder modules. Satisfies **AC-9**, **AC-22**.

## Consequences

**Positive**

- The placeholder era ends. Every feature from 8 onward gets one typed answer to who is asking, what
  they may do, and which restaurant they are in, and gets it the same way every time.
- Revocation is genuinely instant, including inside an open live stream, which is what makes feature
  10's requirement about a deactivated account real rather than aspirational.
- A missing role check is a visible absence in a function signature rather than a missing line in
  the middle of a body, which is the same trick that already protects tenant scoping.
- Spec 0005's three owed items are all closed: the identity bundle carries the settings, `PATCH
  /api/me` writes the personal language, and both placeholder modules are deleted.
- Registration is one screen and one transaction, so the first minute of the product is a form and
  then work, not a settings wizard.
- No email provider, no identity vendor, and no new environment variable enter the project.

**Negative and tradeoffs**

- Authentication is now hand written code this project owns forever. The premise note in the
  rationale says why that is acceptable here and what keeps it small, but the residual risk is real:
  a wrong cookie attribute or a missed gate is quiet, and the only things standing between that and
  a breach are the tests listed above and a careful review.
- Every request costs one indexed database read, and every open live stream costs one every fifteen
  seconds. On a busy evening with a dozen screens open that is a steady background load which exists
  purely to keep revocation instant.
- Sliding the session adds a second, short database transaction to a request, at most once every
  five minutes per session. It commits separately from the handler's own transaction, which is
  correct but does mean a request can now touch the database twice in two different scopes, and the
  ordering between them is load bearing rather than incidental.
- Every sign in attempt, successful or not, writes a row to `login_attempts` and takes an advisory
  lock on the address. That is the price of a throttle that holds across containers, and it makes
  the sign in path a write path even when nothing is wrong.
- An owner who forgets their password and is the only admin is locked out, and the answer today is a
  manual database operation by whoever runs the platform. That is a real product hole, accepted to
  avoid taking on an email provider for it, and it grows more painful with every restaurant that
  signs up.
- A stolen or borrowed phone holds a live session for up to fourteen days unless somebody revokes
  it, and there is no sign out everywhere control to reach for.
- The per email throttle adds a table with no restaurant column, which is a small dent in the
  otherwise absolute statement that every table is tenant scoped. It holds no tenant data, and the
  table comment says so, but somebody auditing the schema will stop on it.
- Two infrastructure changes are now load bearing for a security control. If the load balancer is
  ever reachable outside CloudFront, the client address the throttle trusts can be forged, and
  nothing in the application will notice.
- Every existing handler and both web placeholder consumers change in this feature. It is mechanical
  while there are still only a handful of each, which is exactly why it happens now.

**Neutral**

- Two new crates: `argon2` and `axum-extra` with its cookie feature. Both were named in spec 0001,
  so neither is a new decision, only a new dependency line.
- Spec 0001 also named `tower-governor` for rate limiting the auth endpoints. It is not used, because
  its limiter state lives in the process: with several Fargate tasks behind the load balancer, each
  container would grant a caller the full allowance, so the address bucket's real ceiling would
  multiply by the instance count. Both buckets are counted in `login_attempts` instead, which is
  shared by construction and lets one advisory lock make both of them race free. That line in spec
  0001 is now out of date and is flagged below.
- Spec 0001 named the `validator` crate for server side validation. This feature uses domain
  newtypes that validate on construction instead, so the rules live where the layer rules put
  invariants. That line in spec 0001 is now out of date and is flagged below.
- The countries file follows the language catalogue's pattern exactly, so the project now has two
  shared reference files compiled into Rust and imported by the browser, and one more reason the
  Docker build must run from the repository root.
- No community Agent Skill was installed for this feature. The candidates found were general web
  auth guidance built around signed tokens and OAuth, which is the opposite of what was chosen here,
  so they were declined rather than installed and argued with.

## Follow-up

- [ ] Password recovery for an owner who is the only admin is a manual database operation today.
      Revisit before the first restaurant that is not yours signs up, and before feature 21 charges
      anybody money. The likely answer is an email provider (AWS SES on this stack) and a single use
      token, which is a feature of its own.
- [ ] Feature 10 (staff accounts) inherits three revocation rules written in this spec's state
      transitions table: deactivating an account, changing a role, and resetting somebody's password
      each revoke that person's sessions. It also owns the admin resets a password endpoint and the
      staff blanking erasure path.
- [ ] Neither erasure path spec 0003 designed is built. Blanking a `staff` row belongs with feature
      10; deleting a restaurant belongs with whatever feature lets an owner leave. Until one exists,
      an erasure request is a manual operation and should be treated as one.
- [ ] Spec 0001's stack table names the `validator` crate for server side validation. This feature
      chose domain newtypes instead. That row wants updating, or an explicit note that validation
      lives in the domain.
- [ ] Spec 0001's load bearing notes say to rate limit the authentication endpoints with
      `tower-governor`. This feature counts both buckets in Postgres instead, because an in process
      limiter does not hold across several containers. That note wants updating.
- [ ] `api/AGENTS.md` and `web/AGENTS.md` both describe `RestaurantScope` and
      `currentRestaurantId()` as placeholders feature 7 will replace. Both lines stop being true
      when this ships, and `/sync` owns updating them.
- [ ] Consider connecting a Postgres MCP server so the agent reads the live schema, now sixteen
      tables plus this feature's migration, rather than trusting the migration files. It is added in
      your own MCP settings, not by any skill here. `AGENTS.md` already carries the recommendation.
- [ ] `sessions` carries no device label, so a future account screen listing somebody's other
      devices would show opaque rows. If sign out everywhere is ever wanted, that column and that
      screen arrive together.
