# 0009. Staff accounts

**Date**: 2026-09-13
**Status**: In Progress

## Summary

An admin can now put real people on the floor. They create a waiter or a chef with a starting
password they hand over in person, and that person must choose their own password before they can
do anything else, so a password two people know is never valid for more than one sign in. An admin
can also rename somebody, change their role, reset their password, and switch an account off when
they leave, and every one of those actions cuts that person's live sessions immediately. Nothing new
is installed: no email provider, no crate, no web library, no environment variable.

Reasoning and options: see [rationale.md](rationale.md).

## Requirements

**User stories**:

- As a restaurant owner, I want to create an account for a new waiter in under a minute while they
  stand next to me, so that they can start taking orders on their first shift rather than their
  second.
- As a restaurant owner, I want the password I hand somebody to stop working the moment they have
  their own, so that I am not carrying around a list of my staff's live passwords.
- As a waiter, I want to sign in and be told plainly to pick my own password, so that I know the one
  my manager read out to me is not the one I keep.
- As a restaurant owner, I want an account switched off the instant somebody leaves, whatever they
  have open on the floor, so that ending access is never a negotiation with the dinner service.
- As a restaurant owner, I want to see at a glance who has never managed to sign in, so that I find
  out about a new chef's typo on Tuesday rather than on Friday night.
- As a restaurant owner, I want the platform to refuse to let me remove the last admin, so that one
  mistaken tap cannot lock my restaurant out of its own account.
- As a chef who forgot their password, I want my manager to be able to reset it, so that I am not
  waiting on a support request in the middle of a service.

**Acceptance criteria** (the contract, each criterion is IDed and independently checkable):

- **AC-1**: An admin creates a staff member with a display name, an email address, a starting
  password, and a role. On success exactly one `staff` row exists carrying that role,
  `must_change_password` true, `version` 1, a null `deactivated_at`, and a null `last_sign_in_at`.
  The response carries the new person's row (id, display name, email, role, active, last signed in,
  password owed, version) and never the password or its hash. The address is stored exactly as
  typed and matched case insensitively.
- **AC-2**: An email address that already has an account anywhere on the platform is refused with
  `400` and `fields: { "email": "already_taken" }`, and nothing is created. This includes an address
  held by a deactivated row and an address held in another restaurant, which the caller cannot tell
  apart from either.
- **AC-3**: The starting password obeys spec 0006's rules exactly: trimmed, at least 10 characters,
  at most 72 bytes, hashed with `argon2id` off the async runtime, and the trimmed value is the one
  hashed. A display name is trimmed by the API before it is written, and the trimmed value is the
  one stored, not merely the one measured; it is then not blank and at most 80 characters. A role
  outside `admin`, `waiter`, `chef` is refused. Each failure returns a field error and creates
  nothing.
- **AC-4**: A person whose `must_change_password` is true is refused every endpoint except
  `GET /api/me` and `POST /api/me/password`, with `403` and the code `password_change_required`,
  before the handler body runs, except where a role refusal applies first: the role is checked
  before the password gate, so a waiter who also owes a password change receives a plain `403` from
  an admin only endpoint, not `password_change_required`. `GET /api/events` is refused at open, and
  the event stream's own heartbeat applies the same gate on every re resolve, so a stream already
  running when the flag is set closes within one heartbeat rather than relying on the revocation
  that happens to accompany every path that sets it. In the browser every route sends them to the
  change screen, and no other screen renders first, not even for a frame.
- **AC-5**: That person satisfies the requirement through the existing `POST /api/me/password`,
  giving the password the admin handed them as the current one. On success the new hash is written,
  `must_change_password` becomes false, every other session of that person is revoked, the session
  that made the request keeps working, and they land on their role's surface. A wrong current
  password changes nothing and leaves the flag set.
- **AC-6**: `GET /api/staff` returns every staff member of the caller's restaurant in one response
  with no pagination, each carrying id, display name, email, role, whether they are active, when
  they last signed in (null meaning never), whether they still owe a password change, and the
  version. Order is active people first, then by display name case insensitively, then by id.
- **AC-7**: An admin edits a display name with `PATCH /api/staff/{id}`, naming the version they
  loaded. It bumps `version`, writes one audit row, and touches no session.
- **AC-8**: An admin changes a role with `PUT /api/staff/{id}/role`, naming the version. It bumps
  `version`, revokes every session that person holds so their next request is refused at once, and
  writes one audit row. Setting the role the person already has writes nothing, revokes nothing,
  bumps nothing, and still answers `200`.
- **AC-9**: An admin resets somebody's password with `POST /api/staff/{id}/password`. It writes the
  new hash, sets `must_change_password` true, revokes every session that person holds, and writes
  one audit row carrying no hash and no password in any value. It takes no version. The same
  password rules as **AC-3** apply.
- **AC-10**: An admin deactivates an account with `POST /api/staff/{id}/deactivate`. It sets
  `deactivated_at`, revokes every session that person holds, and writes one audit row. It is never
  refused because of anything on the floor: open visits, unserved rounds, and open bills keep
  pointing at that row and the action succeeds regardless. That person's very next request is `401`,
  and any live stream they hold closes within one heartbeat.
- **AC-11**: An admin brings an account back with `POST /api/staff/{id}/reactivate`. It clears
  `deactivated_at` and changes nothing else: the role, the password, and the password owed flag are
  all exactly as they were. It writes one audit row.
- **AC-12**: Two guard rails hold, both decided inside the writing transaction after a transaction
  level advisory lock keyed on the restaurant. An admin acting on their own row through role change,
  deactivate, or password reset is refused with `409 cannot_act_on_self`. An action that would leave
  the restaurant with no active admin, whether by demoting the last one or deactivating them, is
  refused with `409 last_admin`. Two admins demoting each other at the same instant cannot both
  succeed.
- **AC-13**: Every write against a deactivated row except reactivate is refused with
  `409 staff_inactive`, and reactivating an already active row is refused the same way. Deactivate
  and reactivate are each a single conditional update naming the state they expect
  (`deactivated_at IS NULL` and `IS NOT NULL` respectively), so a deactivate and a reactivate
  issued at the same instant resolve to exactly one winner and the loser matches zero rows and reads
  `409 staff_inactive`. The outcome is always one of the two intents, never a blend of them.
- **AC-14**: An edit naming a version the row no longer has is refused with `409 staff_changed` and
  writes nothing. An unknown id, and an id belonging to another restaurant, both read as `404`,
  which the caller cannot tell apart. When more than one refusal applies to the same request, every
  targeted action reports them in one fixed order, decided before any write: unknown id (`404`),
  then `cannot_act_on_self`, then `staff_inactive`, then `last_admin`, then `staff_changed`. The
  stale version comes last because it is the only one of the five a reload cures.
- **AC-15**: Every `/api/staff` endpoint is admin only. A waiter and a chef each receive `403`
  before the handler body runs, the restriction is visible in the handler's own signature, and it
  reaches the OpenAPI document.
- **AC-16**: An admin of one restaurant can neither read nor write a staff row of another, proven
  against a real Postgres as `app_api`, the way spec 0003's isolation tests are.
- **AC-17**: Each of the six actions writes exactly one `audit_log` row naming the acting admin, the
  staff member as entity, and the before and after values, in one shape shared by all six.
  `staff_created` carries a null `before` and an `after` holding the display name, the email address,
  and the role. The other five carry the same four mutable fields on each side: display name, role,
  active, and password owed, so a password reset reads as the owed flag moving false to true and a
  deactivation as active moving true to false. No password, no password hash, and no session token
  appears in any value. The six actions are `staff_created`, `staff_edited`, `staff_role_changed`,
  `staff_password_reset`, `staff_deactivated`, `staff_reactivated`.
- **AC-18**: The admin staff screen lists active people with a separate section below for
  deactivated ones, where the only available action is to bring somebody back. Creating opens a
  dialog whose password field can be revealed and can be filled with a suggested strong password,
  and on success the dialog shows the email address and the password together once, stating plainly
  that this person must change it at first sign in, and closes on acknowledgement. Editing, changing
  a role, resetting a password, and deactivating are each reachable from a person's row, with
  deactivation confirmed first.
- **AC-19**: The identity bundle carries `mustChangePassword` on its `staff` object, returned
  identically by register, sign in, `GET /api/me`, `PATCH /api/me`, and `PATCH /api/restaurant`, so
  the browser reads it from the one cache entry it already holds.
- **AC-20**: `pnpm db:seed` creates its waiter and its chef through this feature's real create path
  and leaves `must_change_password` false on both, so signing in locally and `pnpm e2e` are
  unchanged by this feature. Running the seed twice still changes nothing.
- **AC-21**: The forced password change screen renders outside the application shell, like sign in
  and register, because the shell opens the live stream and that stream is refused while the flag is
  set. Every screen this feature adds carries no hard coded user facing text, formats through the
  layer spec 0005 built, and meets the accessibility level spec 0004 set.

## Decision

**Chosen option**: Option 1: Admin managed accounts on the existing `staff` table, with a forced
password change.

An admin creates, edits, re roles, resets, deactivates, and reactivates staff on the row spec 0003
already designed; the starting password an admin hands over is marked as owed and is spent the first
time that person signs in, and every action that changes who somebody is or whether they may work
revokes their live sessions in the same transaction.

**Implementation skills**: `axum-web-framework` (`manutej/luxor-claude-marketplace`,
`.agents/skills/axum-web-framework/`) · `rust-backend` (`windmill-labs/windmill`,
`.agents/skills/rust-backend/`) · `rust-best-practices` (`apollographql/skills`,
`.agents/skills/rust-best-practices/`) · `sqlx-postgres` (`daiki48/dotfiles`,
`.agents/skills/sqlx-postgres/`) · `postgresql-table-design` (`wshobson/agents`,
`.agents/skills/postgresql-table-design/`) · `react-router-data-mode` (`remix-run/agent-skills`,
`.agents/skills/react-router-data-mode/`) · `tanstack-query` (`tanstack-skills/tanstack-skills`,
`.agents/skills/tanstack-query/`) · `shadcn` (`shadcn/ui`, `.agents/skills/shadcn/`) ·
`react-i18next` (`yildizberkay/skills`, `.agents/skills/react-i18next/`) · `vitest` (`antfu/skills`,
`.agents/skills/vitest/`) · `accessibility` (`addyosmani/web-quality-skills`,
`.agents/skills/accessibility/`)

**Settled here, not asked** (each with the runner up):

- **`resolve_session` returns `must_change_password`.** The `Actor` extractor already gets the
  restaurant, the staff id, the session id, and the role from one unscoped read; the gate in
  **AC-4** costs one more boolean on that same answer and no second query. Migration `0007` alters
  the function and re asserts the `auth_lookup` ownership and grant, because a `DROP` takes both
  with it. Runner up: a second read of `staff` inside the extractor, which doubles the per request
  cost of every request in the product to serve a flag that is false almost always.
- **The password gate is a second type level marker on the extractor, not a path match.**
  `Actor<R, P = PasswordSettled>` gains a `PasswordGate` parameter beside `RoleRequirement`, with two
  markers: `PasswordSettled`, the default, which refuses a caller who owes a password change, and
  `PasswordMayBeOwed`, named by exactly two handlers, `GET /api/me` and `POST /api/me/password`.
  Every other handler, `GET /api/events` included, is gated by writing nothing, so a new endpoint is
  covered by omission rather than by somebody remembering. This is the same trick that already makes
  a missing role check visible in a signature, and it reaches the OpenAPI document the same way.
  Runner up: matching `parts.uri.path()` inside the extractor, which duplicates the router table
  `api/AGENTS.md` calls the single source of truth and breaks on a path parameter or a trailing
  slash. The role check runs first and the password gate second, so a wrong role is still a plain
  `403`.
- **A new `DomainError::PasswordChangeRequired` mapping to `403` with the code
  `password_change_required`.** The browser has to tell this apart from an ordinary refusal, because
  one means go and change your password and the other means you may not do that. Runner up: reuse
  `Forbidden`, which makes the two indistinguishable on the wire.
- **Four new `ConflictKind` variants**: `StaffChanged`, `LastAdmin`, `CannotActOnSelf`,
  `StaffInactive`, each reaching the browser as a code mapped to a translation key, the way spec
  0008's conflicts do. `EmailTaken` already exists and is turned into a field error on the email box
  exactly as `handlers/auth.rs` already does. Runner up: sentences, which cannot be translated.
- **No new live event kind.** A staff change is invalidated on the acting admin's own screen in
  `onSuccess`, the way spec 0008 refreshes the admin menu, and everyone else learns of it the only
  way that matters: their session stops working. A second admin with the screen open sees a stale
  list until they act, which is a rare, harmless staleness. Runner up: a `staff` event kind, which
  adds a vocabulary entry, a query key map entry, and a notify trigger for one rarely open screen.
- **Version is required on the two edits and on nothing else.** The name edit and the role change
  both write a value the admin read off a form, so a stale one must be refused. Deactivate,
  reactivate, and the password reset set an absolute value the admin intends regardless of what they
  last saw, so they skip the check, exactly as spec 0008's availability switch does. Runner up:
  version everywhere, which makes shutting an account off fail because somebody renamed them.
- **The suggested password is generated in the browser** with `crypto.getRandomValues`, and the
  admin may overwrite it. No endpoint, and the plain password still exists only where it already
  had to: in the form the admin typed into. Runner up: a generate endpoint, which puts a plain
  password in a response body for no gain.
- **The screen lives at `/admin/staff`**, inside the existing admin route group, linked from the
  admin home beside the menu. Runner up: a tab on restaurant settings, which buries a daily task
  under a yearly one.
- **No pagination on the staff list.** A restaurant has tens of staff, never thousands, and the
  screen is one scannable list. The new `(restaurant_id)` index keeps the read cheap as the table
  grows across restaurants. Runner up: paginate now, which adds a control nobody will ever page.
- **No throttle on the `/api/staff` endpoints.** Every one is admin only, tenant scoped, and behind
  a resolved session, so there is no unauthenticated abuse surface to close. The one password
  guessing path, `POST /api/me/password`, already requires a live session. Runner up: a third
  bucket in `login_attempts`, which costs a write per admin action to prevent nothing.
- **Deactivating writes only `deactivated_at` and the revocations.** It does not blank a name, clear
  a password, or free the email address, because none of that is reversible and reactivation is.
  Runner up: clear the password on deactivation, which turns a reactivation into a reset.
- **Length limits are checked by the API as field errors and backed by check constraints**, the same
  pairing spec 0008 used.

## Rationale

Reasoning and options: see [rationale.md](rationale.md).

## Feature design

**Data model sketch**

One migration, `api/migrations/0007_staff_accounts.sql`. Everything not listed is spec 0003's schema
as spec 0006 left it.

| Table or object | Column, index, or constraint | Type and rule | Change |
|---|---|---|---|
| `staff` | `must_change_password` | `boolean NOT NULL DEFAULT false` | new. True whenever an admin wrote this person's password, false once they wrote their own |
| `staff` | `version` | `int NOT NULL DEFAULT 1` | new. Same shape spec 0008 gave `menu_categories` and `dishes` |
| `staff` | `staff_display_name_length` | check `char_length(btrim(display_name)) BETWEEN 1 AND 80` | new |
| `staff` | `staff_email_length` | check `char_length(email) <= 254` | new |
| `staff` | `staff_restaurant_idx` | `(restaurant_id)` | new. The staff list is the first query that reads this table by restaurant |
| `resolve_session` | altered | returns `must_change_password` alongside what it already returns | The function is `SECURITY DEFINER` and owned by `auth_lookup`. Postgres refuses `CREATE OR REPLACE` when the returned columns change, so this is a `DROP` and recreate, exactly as migration `0005` did when it added `last_seen_at`, and the migration runs as the schema owner and re asserts the owner and the grant, because a `DROP` takes both with it |
| `audit_log` | six new `action` values | `staff_created`, `staff_edited`, `staff_role_changed`, `staff_password_reset`, `staff_deactivated`, `staff_reactivated` | no column change. `action` is `text`, so this is a Rust `AuditAction` addition only |

Unchanged and reused as they stand: `staff.email` (unique platform wide on `lower(email)`, so a
deactivated row keeps its address claimed), `staff.role` (the `staff_role` enum, three values, still
the whole authorisation model), `staff.deactivated_at` (null means active), `staff.last_sign_in_at`
(added by spec 0006 for this screen), `staff.password_hash` (`argon2id`), and the composite
`(id, restaurant_id)` key every other table references.

Relationships are unchanged. `restaurants` 1:N `staff`; `staff` 1:N `sessions` (composite foreign
key, `ON DELETE CASCADE`); `staff` is referenced by `visits`, `rounds`, `order_lines`, `bills`, and
`audit_log` through composite foreign keys, which is why a row is deactivated and never deleted.

Rust side: `must_change_password: bool` and `version: i32` on the `Staff` entity; a `StaffEdit` for
the name edit; new `AuditAction` variants for the six actions; new `ConflictKind` variants
`StaffChanged`, `LastAdmin`, `CannotActOnSelf`, `StaffInactive`; a new `DomainError`
variant `PasswordChangeRequired` mapping to `403`.

**State transitions**

An account: `active` → `inactive` (deactivate) → `active` (reactivate). Neither end is terminal and
nothing else changes across either move. No row is ever deleted.

A password: `owed` (an admin wrote it, at creation or at reset) → `own` (that person wrote it). The
move happens only through `POST /api/me/password`. An admin reset moves it back to `owed`, and that
is the only way back.

Session revocation now has code behind three of the five events spec 0006 wrote down:

| Event | Revokes | Owned by |
|---|---|---|
| Sign out | that one session | spec 0006 |
| Own password change | every other session of that person | spec 0006 |
| Account deactivated | every session of that person | **this feature** |
| Role changed | every session of that person | **this feature** |
| Password reset by an admin | every session of that person | **this feature** |
| Restaurant deactivated | every session in that restaurant | a later feature |

Every revocation is one `UPDATE` inside the same scoped transaction that made the change, so a
failure after it leaves neither.

**API surface**

Every endpoint sits under the existing router, the same origin check, and the 30 second timeout,
carries `Actor<Admin>`, and is reached only through a resolved session. `{id}` is always a staff id
inside the caller's own restaurant; anything else is `404`.

| Endpoint | Method | Key inputs | Key outputs | Auth | Key errors |
|---|---|---|---|---|---|
| `/api/staff` | GET | none | `staff[]`: id, displayName, email, role, active, lastSignInAt, mustChangePassword, version | `admin` | `401`, `403` |
| `/api/staff` | POST | `displayName:string` (req), `email:string` (req), `password:string` (req), `role:"admin"\|"waiter"\|"chef"` (req) | the new staff row | `admin` | `400` `fields.email=already_taken`, `400` `fields.password=too_short`, `400` `fields.displayName=required`, `401`, `403` |
| `/api/staff/{id}` | PATCH | `displayName:string` (req), `version:int` (req) | the updated staff row | `admin` | `409 staff_changed`, `409 staff_inactive`, `404`, `400`, `403` |
| `/api/staff/{id}/role` | PUT | `role:"admin"\|"waiter"\|"chef"` (req), `version:int` (req) | the updated staff row | `admin` | `409 staff_changed`, `409 last_admin`, `409 cannot_act_on_self`, `409 staff_inactive`, `404`, `403` |
| `/api/staff/{id}/password` | POST | `password:string` (req) | `204` | `admin` | `400` `fields.password=too_short`, `409 cannot_act_on_self`, `409 staff_inactive`, `404`, `403` |
| `/api/staff/{id}/deactivate` | POST | none | the updated staff row | `admin` | `409 last_admin`, `409 cannot_act_on_self`, `409 staff_inactive`, `404`, `403` |
| `/api/staff/{id}/reactivate` | POST | none | the updated staff row | `admin` | `409 staff_inactive` (already active), `404`, `403` |
| `/api/me/password` | POST | unchanged from spec 0006 | `204` | any signed in role | unchanged, plus it now clears `must_change_password` |
| `/api/me` | GET | none | the identity bundle, now carrying `staff.mustChangePassword` | any signed in role | `401` |

The identity bundle gains exactly one member, so the browser keeps one type and one cache entry:

```
{ staff:      { id, displayName, email, role, language, mustChangePassword },
  restaurant: { ... unchanged ... } }
```

Field error codes are spec 0006's closed set, reused unchanged. Conflict codes are the four new
`ConflictKind` values plus `password_change_required`, each mapped to a translation key on the web
side; the English `message` is never rendered.

**Value sourcing**

| Action | Value produced or displayed | Source |
|---|---|---|
| Create | The new staff id | UUID v7, generated in Rust, as spec 0003 established |
| Create | `restaurant_id` | The `Actor`'s resolved session. Never an input, and there is no way to send one |
| Create | `password_hash` | `argon2id` of the trimmed password through spec 0006's hashing port, run off the async runtime |
| Create | `must_change_password` | The literal `true`. Set by the act of an admin writing a password, not by anything the caller sends |
| Create | `version` | The column default, `1` |
| Create, every write | `created_at`, `updated_at`, `deactivated_at` | Postgres `now()`, never a Rust clock, as spec 0006 fixed |
| Create | Whether the address is free | The `staff_email_key` unique index on `lower(email)`, whose violation becomes `ConflictKind::EmailTaken` and then the `already_taken` field error. Not an application pre check, which would race |
| Create | The stored email address | Exactly what the admin typed. Only the index and every lookup lower it |
| Create | The suggested password offered in the form | `crypto.getRandomValues` in the browser. Never the server, and never stored anywhere |
| Create | What the hand over panel shows | The email address and the password already in the form the admin submitted. The API returns neither |
| Every write | The audit actor | `staff_id` from the same `resolve_session` answer that scoped the transaction |
| List | Active or not | `deactivated_at IS NULL` |
| List | "Last signed in", and "never" | `staff.last_sign_in_at`, written by spec 0006's sign in path. Null renders as a translation key, never an English sentence |
| List | How that timestamp reads | `restaurant.formattingLocale` and `restaurant.timezone` from the identity bundle, through the layer spec 0005 built |
| List | The order rows appear in | `deactivated_at IS NULL DESC`, then `lower(display_name)`, then `id`, computed in SQL so every client agrees |
| Role change, deactivate | Whether this leaves the restaurant with no admin | A count of `staff` where `role = 'admin'` and `deactivated_at IS NULL`, taken inside the writing transaction after `pg_advisory_xact_lock` keyed on the restaurant id |
| Role change, deactivate, password reset | Whether the target is the caller | The `Actor`'s own `staff_id` compared with `{id}`, before any write |
| Role change, deactivate, password reset | Which sessions die | Every `sessions` row for that `staff_id`, `revoked_at = now()`, in the same scoped transaction |
| Name edit, role change | Whether the row is stale | The `version` the client sent, as `WHERE id = $1 AND version = $2`. Zero rows means stale or missing, told apart by one follow up read, exactly as spec 0008 does |
| Every request | Whether the caller owes a password change | `must_change_password` from `resolve_session`, on the same unscoped read that already yields the role |
| Every request | Whether this endpoint is exempt from the password gate | The handler's own `Actor` signature. `PasswordSettled` is the default and refuses; only `GET /api/me` and `POST /api/me/password` name `PasswordMayBeOwed`. Never the request path |
| Every request | Which refusal wins when two apply | The fixed order in **AC-14**, decided before any write. The role check runs ahead of the password gate, so a wrong role is a plain `403` |
| Every live stream heartbeat | Whether the flag has since been set | The same `resolve_session` answer, re read on each heartbeat by `resolve_and_slide`, which applies the gate rather than assuming the revocation that accompanies it |
| Every write | What the audit row's `before` and `after` hold | The staff row's four mutable fields read inside the same transaction, before and after the write: display name, role, active, password owed. `staff_created` has a null `before` and an `after` of display name, email, and role |
| Deactivate, reactivate | Which of two racing callers wins | The conditional update's own `WHERE` clause on the current state. The loser matches zero rows and reads `staff_inactive`; neither takes a version |
| Every screen | Whether the signed in person owes a password change | `identity.staff.mustChangePassword` from the bundle, the browser's one copy of the server's answer |
| Any refusal | The sentence a person reads | The stable error code, and each `fields` code, mapped to a translation key. The English `message` is for logs |
| Seed | Whether the seeded waiter and chef owe a change | Cleared deliberately by `seed.rs` after creation, with a comment saying why, so `pnpm e2e` and local sign in are unaffected |

**Key invariants**

1. **A restaurant always has at least one active admin.** Enforced by a count inside the writing
   transaction, under an advisory lock on the restaurant, so two admins cannot both pass it.
2. **`must_change_password` and a password an admin wrote always travel together.** Every path that
   writes somebody else's password sets it; only that person writing their own clears it.
3. **A person who owes a password change can reach exactly two endpoints.** Enforced in the `Actor`
   extractor by its default marker, before any handler body, so a non browser client is bound by it
   too, and a new endpoint is covered unless it deliberately opts out.
4. **A request that resolved as an admin completes as an admin.** The actor is resolved before the
   writing transaction opens, and nothing re reads the caller's own role inside it. An admin demoted
   or deactivated while one of their own requests is in flight finishes that request; revocation
   binds their next one. The window is one request long and is accepted deliberately: closing it
   would cost a lock and a scoped read on every admin write, forever.
5. **An account is never deleted.** Every reference from `visits`, `rounds`, `order_lines`, `bills`,
   and `audit_log` keeps naming a real person forever.
6. **An email address, once used, stays claimed platform wide.** Deactivating does not free it, and
   the unique index, not an application check, is what enforces it.
7. **Every write that changes a staff row bumps `version` in the same statement.**
8. **No admin acts on their own row through this feature.** Their own name, language, and password
   live on the account panel spec 0006 built, behind the current password check.
9. **A revocation and the change that caused it commit together, or neither does.** Both are in the
   one scoped transaction.
10. **Nothing in this feature reads or writes across restaurants.** Spec 0006's rule that exactly two
   paths do, both `SECURITY DEFINER` and owned by `auth_lookup`, still holds; altering
   `resolve_session` does not add a third.
11. **No password and no password hash reaches a log or an audit value**, on any path here.

**Security model**

| Who | May read | May write |
|---|---|---|
| `admin` | every staff row of their own restaurant, including email addresses and sign in history | create, rename, re role, reset a password, deactivate, and reactivate any row of their own restaurant **except their own** |
| `waiter`, `chef` | nothing on this surface | nothing on this surface |
| A person who owes a password change | `GET /api/me` only | their own password only |

Enforcement is the three independent layers spec 0006 established, and this feature adds no fourth:
`Actor<Admin>` refuses the wrong role before the handler runs, `Database::begin_scoped` sets
`app.restaurant_id` so row level security applies to every query, and the composite foreign keys
make a cross restaurant reference physically impossible. The two guard rails and the password owed
gate sit inside that, not beside it.

**Compliance scope**: GDPR style rules apply to a staff member's name, email address, and password
hash, and this feature is the first place one person writes another person's credentials. Audit
logging is therefore not negotiable: these six actions are access control changes. The erasure path
spec 0003 designed (blanking a `staff` row) is **not built here**, so an erasure request remains a
manual database operation, recorded as a follow up rather than assumed away.

**Configuration required**

None. No new environment variable, no new secret, and no `infra/` change. `.env.example` gains only
a note that the seeded waiter and chef are created through the staff path and do not owe a password
change. Length limits and the password rules are the constants spec 0006 already placed beside the
code that uses them.

**Critical test scenarios**

- Happy path: an admin creates a waiter, the waiter signs in, is sent to the change screen and
  refused everywhere else, sets their own password, and lands on `/waiter` with the flag cleared,
  verifies **AC-1**, **AC-4**, **AC-5**.
- Happy path: the staff list shows an active waiter, a chef who has never signed in, and a
  deactivated person in the section below, in the specified order, verifies **AC-6**.
- Happy path: an admin creates somebody and the hand over panel shows the address and the password
  together once, verifies **AC-18**.
- Failure case: creating with an address already held by a deactivated person in another restaurant
  returns `fields.email=already_taken` and creates nothing, verifies **AC-2**.
- Failure case: two admins each demoting the other at the same instant leave exactly one admin
  standing, because the advisory lock serialises the count and the write, verifies **AC-12**.
- Failure case: the last active admin cannot be demoted or deactivated, and their own row cannot be
  acted on at all, verifies **AC-12**.
- Failure case: a name edit naming a version somebody else already bumped is refused
  `409 staff_changed` and writes nothing, verifies **AC-14**.
- Failure case: a role change to the role the person already holds writes nothing and revokes no
  session, verifies **AC-8**.
- Failure case: deactivating a waiter who has two open visits and an unserved round succeeds, and
  their next request is `401` while the visits stay open and still name them, verifies **AC-10**.
- Failure case: a person who owes a password change is refused `GET /api/events` and every other
  endpoint with `password_change_required`, and a live stream open when the flag is set closes on
  its next heartbeat, verifies **AC-4**.
- Failure case: a waiter who also owes a password change receives a plain `403` from `/api/staff`,
  not `password_change_required`, because the role check runs first, verifies **AC-4**, **AC-15**.
- Failure case: a stale role change aimed at an already deactivated person reports `staff_inactive`
  rather than `staff_changed`, because the fixed order puts the thing a reload cannot fix first,
  verifies **AC-14**.
- Failure case: a deactivate and a reactivate issued at the same instant leave the row in one of the
  two intended states, with the loser reading `409 staff_inactive`, verifies **AC-13**.
- Failure case: a wrong current password on the forced change leaves the flag set and the person on
  the change screen, verifies **AC-5**.
- Auth and permission: a waiter's session and a chef's session each receive `403` from all seven
  `/api/staff` endpoints, verifies **AC-15**.
- Auth and permission: an admin of restaurant A sending a staff id from restaurant B receives `404`
  on every endpoint, proven against a real Postgres as `app_api`, verifies **AC-16**.
- Auth and permission: `pnpm db:seed` run twice leaves the seeded waiter and chef signing in with no
  password change, and `pnpm e2e` passes unchanged, verifies **AC-20**.

## Build plan

Ordered by the project's Tracer Bullet approach. The thread here is one real person joining the
restaurant: an admin creates a waiter, that waiter signs in, is forced to pick their own password,
and reaches the waiter screen. Nothing about roles, resets, deactivation, or the guard rails exists
until that thread runs end to end, because each of them is a thickening of it. The whole migration
lands in the first milestone rather than being sliced, because the thread needs the flag, the
altered lookup function, and the version column all at once, and a second migration touching
`resolve_session` again would re assert its ownership twice for no gain.

**Milestone 1: the thread, top to bottom.**

1. Write migration `0007_staff_accounts.sql`: the two columns, the two check constraints, the
   restaurant index, and the altered `resolve_session` re asserting its `auth_lookup` owner and
   grant. Refresh the `.sqlx` cache. Satisfies **AC-1**, **AC-4**, **AC-14**.
2. Add the domain and application pieces: `must_change_password` and `version` on `Staff`, the four
   new `ConflictKind` variants, `DomainError::PasswordChangeRequired` and its `403` mapping, and the
   create use case reusing spec 0006's `EmailAddress`, `Password`, and hashing port. Satisfies
   **AC-1**, **AC-3**.
3. Add the staff repository operations for create and list, both through `Database::begin_scoped`.
   Satisfies **AC-1**, **AC-6**.
4. Build `POST /api/staff` and `GET /api/staff` behind `Actor<Admin>`, with the `EmailTaken` to
   `already_taken` field error conversion, and add both to `presentation/openapi.rs`. Satisfies
   **AC-1**, **AC-2**, **AC-3**, **AC-6**, **AC-15**.
5. Gate the `Actor` extractor on `must_change_password` by adding the `PasswordGate` marker
   parameter beside `RoleRequirement`, with `PasswordSettled` as the default and
   `PasswordMayBeOwed` named by only `GET /api/me` and `POST /api/me/password`. Keep the role check
   ahead of it, apply the same gate in `resolve_and_slide` so the heartbeat closes a stream, reflect
   it in the OpenAPI document, and carry the flag through the identity bundle. Regenerate the typed
   client. Satisfies **AC-4**, **AC-19**.
6. Make `POST /api/me/password` clear the flag, and build the forced change screen outside the
   application shell with the router sending every other route to it. Satisfies **AC-5**,
   **AC-21**.
7. Build the first cut of `/admin/staff`: the list, the create dialog with its reveal and suggest
   controls, and the hand over panel. Prove the thread by creating a waiter, signing in as them, and
   reaching the waiter floor. Satisfies **AC-18**.

**Milestone 2: the rest of the actions.**

8. Build `PATCH /api/staff/{id}` and `PUT /api/staff/{id}/role` with the conditional version update
   and the role change revocation, including the no op role case. Satisfies **AC-7**, **AC-8**,
   **AC-14**.
9. Build `POST /api/staff/{id}/password` with its revocation and the flag set back to owed.
   Satisfies **AC-9**.
10. Build `POST /api/staff/{id}/deactivate` and `POST /api/staff/{id}/reactivate`, each as a single
    conditional update naming the state it expects, with their revocation and the inactive row rule,
    so two racing callers resolve to one winner. Satisfies **AC-10**, **AC-11**, **AC-13**.

**Milestone 3: the guard rails.**

11. Add the advisory lock and the active admin count to the role change and deactivate paths, the
    self action refusal to all three targeted actions, and the one fixed refusal order shared by
    every targeted action. Satisfies **AC-12**, **AC-14**.
12. Write the tenant isolation and concurrency tests against a real Postgres as `app_api`: a foreign
    staff id reading `404`, and two parallel demotions leaving one admin. Satisfies **AC-12**,
    **AC-16**.

**Milestone 4: the surface in full.**

13. Finish the admin screen: the edit dialog, the role control, the reset dialog, the deactivate
    confirmation, and the inactive section with reactivate, all through the router's actions and
    spec 0004's primitives, with every string translated and the pending value held in component
    state rather than the cache. Satisfies **AC-13**, **AC-18**, **AC-21**.
14. Write the six audit rows in the one shared `before` and `after` shape, with no hash and no
    password in any value. Satisfies **AC-17**.
15. Move the seed's waiter and chef onto the real create path and clear their flag deliberately,
    with the comment saying why, and note it in `.env.example`. Satisfies **AC-20**.

## Consequences

**Positive**

- The product can finally be staffed. Features 12 and 13 get real waiters and real chefs to be built
  and demonstrated against, instead of one owner account wearing three hats.
- A password two people know is valid for exactly one sign in, and that is enforced on the wire
  rather than in the browser, so it holds for anything that talks to the API.
- Three revocation rules spec 0006 wrote down but could not build now have code behind them, and the
  one thing that made them worth writing, an instant loss of access, is genuinely instant, including
  inside an open live stream.
- A locked out admin is now recoverable by another admin without touching the database, which
  partly closes the hole spec 0006 flagged.
- No new dependency, no new environment variable, no infrastructure change, and no new failure mode
  in the deploy.

**Negative and tradeoffs**

- The starting password crosses a human channel. It is said aloud in a kitchen or written on a note,
  and an admin who types `waiter2026` has picked a real password for the window before that person
  signs in. The forced change bounds the window, it does not remove it.
- The email address cannot be changed, so a mistyped one stays claimed platform wide forever, on a
  unique index that spans every restaurant. The recovery is to deactivate and create again, and the
  mistyped address is then unusable by anybody, ever.
- An admin account is worth more than it was. It can read every colleague's address and sign in
  history and can seize any non admin account by resetting its password. That is inherent to the
  role and is why every one of those actions writes an audit row.
- A restaurant with exactly one admin is still unrecoverable if that person forgets their password,
  and now has more to lose when it happens. Feature 24 is still the answer and is still unbuilt.
- Every request in the product now carries one more column out of `resolve_session` and one more
  branch in the extractor, to serve a flag that is false for almost every session.
- Revocation binds the next request, not the one already running. An admin demoted while their own
  request is in flight finishes that request with the privileges they resolved with. The window is
  one request long, it is written down as an invariant rather than closed, and closing it later
  would mean a lock and a scoped read on every admin write.
- `Actor` grows a second type parameter, so every handler signature in the project that names a role
  now sits in front of a slightly larger type. The default keeps existing signatures unchanged.
- A new waiter meets a password form before they meet the product, on what is probably a busy first
  shift.
- There is no erasure path. A staff member who asks to be forgotten is a manual database operation,
  the same as before, and this feature had the chance to close that and did not.
- A second admin with the staff screen open sees a stale list until they act, because no live event
  is emitted for a staff change.

**Neutral**

- `staff` becomes the third table with a `version` column, which makes the conditional update rule
  in `api/AGENTS.md` a pattern rather than a menu feature's quirk.
- The `/admin` route group gains a second screen, so the admin home stops being a page with one
  link on it.
- The seed grows a deliberate exception (clearing the flag) that a reader will stop on. The comment
  beside it is load bearing.
- `resolve_session` is altered for the second time (spec 0006 altered it first), which makes its
  ownership and grant re assertion a habit rather than a one off.

## Follow-up

- [ ] The erasure path spec 0003 designed, blanking a `staff` row while keeping it referenced, is
      still unbuilt and now has no owner. It was spec 0006's follow up, handed to this feature, and
      this feature scoped it out. Decide who owns it before a real restaurant that is not yours
      signs up.
- [ ] A mistyped email address is claimed platform wide forever. The cheap fix is to allow an
      address change while `last_sign_in_at` is null, which cannot surprise anyone because nobody
      has used it. Worth revisiting the first time it bites.
- [ ] A restaurant with exactly one admin who forgets their password is still locked out. Feature 24
      (password recovery by email) is the real answer, and until it exists the product's advice is
      to keep two admins.
- [ ] Spec 0006's follow up about `sessions` carrying no device label is unchanged and still open. It
      matters slightly more now that an admin can revoke somebody else's sessions without seeing
      what they are revoking.
- [ ] The `accessibility` community skill is installed at `.agents/skills/accessibility/` and shaped
      spec 0008's screens and this one, but it is listed in neither the root `AGENTS.md` nor
      `web/AGENTS.md`. It is a web surface concern, so it belongs in `web/AGENTS.md`'s
      `## Agent skills` list, not at root.
- [ ] Consider connecting a Postgres MCP server so the agent reads the live schema, now seven
      migrations deep, rather than trusting the migration files. `AGENTS.md` already carries the
      recommendation and spec 0006 repeated it.
