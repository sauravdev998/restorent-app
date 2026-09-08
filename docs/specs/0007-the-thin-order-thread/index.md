# 0007. The thin order thread

**Date**: 2026-09-06
**Status**: In Progress

## Summary

This is the first slice where the platform takes a real order. A waiter opens a table, picks dishes
from the menu into a basket, and sends them as one ticket; the ticket appears on the kitchen screen
across the room within a second or two with nobody refreshing anything; the chef taps each dish done
and the ticket flips to ready by itself; the waiter's screen alerts, announces, and chimes, and one
tap marks the food served; then the bill closes with a number and a total and the table is free
again. Everything underneath it already exists, so this slice adds no database change at all: it
adds the HTTP endpoints over the operations spec 0003 already built, the two screens, the rows the
development database needs to have anything to order, and one browser test that drives a waiter and
a chef at the same time to prove the live path is real rather than assumed.

Reasoning and options: see [rationale.md](rationale.md).

## Requirements

**User stories**:

- As a waiter, I want to open a table and send a dish to the kitchen in a few taps, so that ordering
  is faster than writing it down.
- As a waiter, I want to be told the moment food is ready without watching a screen, so that food
  leaves the pass hot.
- As a chef, I want new tickets to appear by themselves in the order they were sent, so that I never
  have to touch a screen to find out what to cook.
- As a chef, I want to mark one dish done without touching the rest of the ticket, so that food
  leaves the pass as it is ready.
- As a waiter, I want to close the meal with a correct total and free the table at once, so that the
  next party can sit down.
- As the engineer who builds features 9 through 15, I want the whole pipe proven end to end on two
  devices, so that every later slice thickens something that is known to work.

**Acceptance criteria** (the contract, each independently checkable):

- **AC-1**: A signed in waiter sees the floor: every live table, in section then table order, marked
  free or occupied, an occupied one naming who opened it and when. Opening a free table, with an
  optional guest count, creates one open visit and one open bill on it in a single action, and the
  table reads as occupied on every other waiter's screen without a refresh.
- **AC-2**: A table that already has an open visit cannot be opened a second time. The attempt is
  refused with `409` and the code `table_occupied`, and the refused screen shows the table's true
  state without a reload.
- **AC-3**: The waiter's ordering screen lists the restaurant's live menu categories with their
  dishes and prices, in position order. An archived dish never appears. A dish marked unavailable
  does appear, visibly marked as unavailable and impossible to add to the basket, because a waiter
  who can see it is off can tell the customer so.
- **AC-4**: The waiter builds a basket in the browser and sends it once. Sending creates exactly one
  order round, one line per basket entry with its quantity, with the dish's name and price copied
  onto the line, and assigns every one of those lines to the visit's open bill inside the same
  transaction. Nothing about an unsent basket ever reaches the database.
- **AC-5**: A sent round appears on a signed in chef's kitchen screen within about two seconds, with
  no refresh and no user action, showing the table label, the round number within the visit, every
  dish with its quantity, and how long the ticket has been waiting.
- **AC-6**: Kitchen tickets show every round whose status is `queued` or `ready`, oldest first by
  when it was sent, and each ticket's waiting time is computed against the server's clock rather
  than the device's, so a tablet whose clock is wrong still shows a true age.
- **AC-7**: A chef marking one dish ready changes only that dish. When the last dish on the ticket
  that is neither served nor voided is marked, the round becomes `ready` by itself, with nothing
  setting a round status directly.
- **AC-8**: When a round becomes `ready`, every waiter screen raises a visible alert naming the
  table and the round, announces it through the live region, and plays the ready chime when the
  browser has allowed audio. Each round alerts once per screen, never repeatedly, and the alert
  carries the action that marks the round served.
- **AC-9**: Marking a round served sets every line on it that is not voided to `served`, removes the
  ticket from the kitchen screen, and updates both screens without a refresh.
- **AC-10**: The waiter's table screen shows every round on the visit with its status, its dishes
  with quantity and line total, and the bill's running subtotal, written in the restaurant's own
  currency and decimals.
- **AC-11**: Closing in one action closes the bill and then the visit. The screen then shows the
  allocated bill number and the figures the close wrote: subtotal, service charge, each tax
  component, and total. The table reads as free on every waiter's screen without a refresh and can
  be opened again straight away.
- **AC-12**: Closing is refused with `409` and the code `bill_has_unserved_lines` while any line on
  the bill is neither served nor voided, and the message the waiter reads is translated from that
  code, never rendered from the API's English text.
- **AC-13**: Two people acting on the same thing at once leave exactly one winner. The loser
  receives `409` with the code naming what happened, and their screen refetches to the true state
  rather than showing what they attempted. This holds for two chefs marking the same dish and for
  two waiters opening the same table.
- **AC-14**: Role limits hold on the server, not only in the browser. A chef calling any waiter
  endpoint, a waiter calling any kitchen endpoint, and an admin calling either receive `403` before
  the handler body runs, and every one of these endpoints appears in the OpenAPI document with its
  role requirement.
- **AC-15**: A live event invalidates only the query key prefixes its own entity kind feeds, through
  a written map, rather than every active query. A stream open or reopen still refetches everything
  active.
- **AC-16**: While the live stream is closed, both screens show a prominent, politely announced
  connection warning and stay usable: sending, marking, and closing all still work, and the refetch
  on reopen catches up everything missed.
- **AC-17**: `pnpm db:seed` creates, in development only, one table section with four tables, two
  menu categories with six dishes, one of them marked unavailable so the greyed state is visible,
  and a waiter and a chef account beside the existing admin, with their credentials recorded in
  `.env.example`. Running it twice does not duplicate anything.
- **AC-18**: One Playwright run drives two browser contexts at once, one signed in as the waiter and
  one as the chef, and observes the whole thread: the sent ticket appearing in the chef's context
  with no reload, the dish being marked done, and the waiter's context alerting.
- **AC-19**: Both new screens carry no user facing string written into a component, render at their
  own surface's density, and pass the accessibility gates already in place (lint, axe over the base
  components, and the contrast script).

## Decision

**Chosen option**: Option 1: a real thread over the operations that exist, no schema change.

Build the missing presentation layer and the two screens on top of spec 0003's repository, seed the
rows the thread needs into development only, and prove the two device claim with one Playwright run.
The slice adds no migration, no new dependency on the API side, and no new tool other than
Playwright, which root `AGENTS.md` already says joins continuous integration once slice 1 exists.

**Implementation skills**: `axum-web-framework` (`manutej/luxor-claude-marketplace`, `.agents/skills/axum-web-framework/`) · `rust-backend` (`windmill-labs/windmill`, `.agents/skills/rust-backend/`) · `rust-best-practices` (`apollographql/skills`, `.agents/skills/rust-best-practices/`) · `sqlx-postgres` (`daiki48/dotfiles`, `.agents/skills/sqlx-postgres/`) · `react-router-data-mode` (`remix-run/agent-skills`, `.agents/skills/react-router-data-mode/`) · `tanstack-query` (`tanstack-skills/tanstack-skills`, `.agents/skills/tanstack-query/`) · `shadcn` (`shadcn/ui`, `.agents/skills/shadcn/`) · `react-i18next` (`yildizberkay/skills`, `.agents/skills/react-i18next/`) · `vitest` (`antfu/skills`, `.agents/skills/vitest/`)

## Rationale

Why a full thread rather than screens first or endpoints only, and the reasoning behind the seed and
the conflict codes: see [rationale.md](rationale.md).

## Feature design

**Data model sketch**

No migration. Every table, column, index, and policy this thread needs was built by spec 0003 and is
used here exactly as it stands. What the thread touches:

| Entity | Written here | Read here |
|---|---|---|
| `restaurants` | nothing | currency code, currency decimals, service charge percent, timezone |
| `staff` | nothing | display name and role of whoever opened a table or marked a dish |
| `table_sections`, `dining_tables` | nothing | the floor list, in section position then table position order, archived rows excluded |
| `menu_categories`, `dishes` | nothing | the ordering menu, in position order, archived and unavailable dishes excluded |
| `visits` | opened, closed | the floor's occupancy and the table screen's header |
| `order_rounds` | created by `send_round` | the kitchen queue and the table screen's rounds |
| `order_lines` | created at send, then `ready`, then `served` | every dish on both screens |
| `bills` | opened with zero figures, closed with number and figures | the running subtotal and the closed bill |
| `bill_taxes` | written by `close_bill` | the closed bill's tax lines. Empty in this slice, since no tax component is seeded |
| `bill_number_counters` | incremented by `close_bill` | nothing directly |
| `payments` | nothing | nothing. Feature 15 owns it |

The one value this slice leaves at a placeholder is the kitchen's late threshold. `ElapsedTime`
defaults to 900 seconds and `docs/design.md` records that as feature 13's decision; this slice
passes nothing and inherits the default rather than inventing a second placeholder.

**State transitions**

Every transition below already exists in spec 0003 and is reached, not redefined, by this slice.
This slice uses a strict subset: no move, no void, no payment.

- **Visit**: `open` → `closed`, by the closing action, immediately after its bill closes. Refused
  while a bill is still open or a non voided line is unassigned, neither of which can happen here
  because every line is assigned at send.
- **Order line**: `queued` → `ready` (a chef's tap) → `served` (the waiter's serve action). No line
  reaches `voided` in this slice.
- **Order round**: never set directly. It is the total function of its lines, recomputed inside the
  same transaction as every line write. In this slice that means `queued` while any dish is still
  cooking, `ready` once none is, `served` once every dish has been carried out.
- **Bill**: `open` → `closed`, allocating the number and writing every figure once. Refused while a
  line on it is neither served nor voided, and refused for a bill with no lines.

**Interface surface**

Ten endpoints, all under the existing router, all inside the same origin check and the same 30
second timeout as every other request. Each carries its role requirement in the handler's own type
through the `Actor` extractor, and each appears in `presentation/openapi.rs`, without which it is
absent from the generated TypeScript client. Handlers live in three new modules named for the
repository modules they reach: `handlers/menu.rs`, `handlers/service.rs`, `handlers/billing.rs`.

| Endpoint | Method | Key inputs | Key outputs | Auth | Key errors |
|---|---|---|---|---|---|
| `/api/floor` | GET | none | sections with tables; per table the open visit's id, opener name, opened at, and whether food is ready | waiter | `401`, `403` |
| `/api/menu` | GET | none | categories with their dishes: id, name, price, currency, available | waiter | `401`, `403` |
| `/api/visits` | POST | `table_id` (req), `guest_count` (opt) | visit id, bill id | waiter | `409 table_occupied`, `404` unknown or archived table |
| `/api/visits/{id}` | GET | path id | visit, its rounds oldest first with their lines, the bill's figures, and `server_time` | waiter | `401`, `403`, `404` |
| `/api/visits/{id}/rounds` | POST | `lines`: array of `dish_id` + `quantity` (req, at least one) | the created round with its lines | waiter | `409 visit_not_open`, `422` empty basket or unavailable dish |
| `/api/visits/{id}/close` | POST | path id | the closed bill: number, subtotal, service charge, taxes, total, currency | waiter | `409 bill_has_unserved_lines`, `409 bill_already_closed`, `409 bill_has_no_lines` |
| `/api/rounds/{id}/served` | POST | path id | the round with its lines after the write | waiter | `409 round_not_ready`, `404` |
| `/api/kitchen/tickets` | GET | none | tickets (round id, sequence number, table label, sent at, status, lines with dish name, quantity, status) plus `server_time` | chef | `401`, `403` |
| `/api/order-lines/{id}/ready` | POST | path id | the line, and the round's status after the write | chef | `409 line_not_queued`, `404` |
| `/api/events` | GET | none | the existing live stream, unchanged | any signed in | unchanged |

Four new read operations are needed in the repository, because a handler cannot reach the pool and
every read must run inside a `ScopedTx`: the floor (live tables with their open visit and its
opener), the visit document (the visit with its rounds, lines, and open bill), the kitchen queue
(rounds in `queued` or `ready` with their lines and table label, ordered by `sent_at`), and the
closed bill document (the bill with its taxes and lines). Four write paths are more than one
existing operation, and each runs as one scoped transaction so no half done state can exist: the
open (`open_visit` then `open_bill`), the send (`send_round` then `assign_lines_to_bill`), the serve
(`mark_line_served` once per line on the round), and the close (`close_bill` then `close_visit`).

The serve endpoint is the one whose refusal rule is not simply a repository operation's own. It
reads the round first and refuses with `round_not_ready` unless the round is `ready` at that moment.
Inside the loop it marks every line that is `ready`, skips any already `served` rather than treating
it as a conflict, and skips voided lines. A line level conflict raised inside that loop (two waiters
serving the same round at the same instant) is reported as `round_not_ready` as well, because that
is what it means to the person reading it: somebody has already served this.

Conflicts stop being one anonymous code. `DomainError::Conflict` changes from carrying a free
English string to carrying a closed `ConflictKind` enum whose `Display` gives the English log
sentence and whose `as_code()` gives the stable wire code, so the error body's `error` field names
what actually happened and the web maps that code to a translated sentence the way it already maps
every other code. The variant stays a single field, so existing `matches!(.., Conflict(_))` tests
keep compiling. Fourteen variants, producing the fifteen codes below, cover every conflict the
repositories already raise, which is seventeen call sites across four repository files, not only the
ones this thread reaches. Two variants carry the status they expected, which is what lets one
variant produce both line codes:

| Kind and wire code | Raised by | Reached by this thread |
|---|---|---|
| `table_occupied` | `open_visit`, and `move_visit`'s destination | yes |
| `visit_not_open` | the visit helper carrying its expected status, used by `move_visit`, `send_round`, `close_visit`, `open_bill` | yes |
| `visit_has_open_bill` | `close_visit` | no |
| `visit_has_unbilled_line` | `close_visit` | no, because every line is assigned at send |
| `line_not_queued` | the line helper when the expected status is `queued`, used by `mark_line_ready` | yes |
| `line_not_ready` | the same helper when the expected status is `ready`, used by `mark_line_served` | yes, inside the serve loop |
| `round_not_ready` | the serve handler's own precheck, per the rule above | yes |
| `bill_not_open` | `assign_lines_to_bill`, target bill | yes |
| `line_on_closed_bill` | `assign_lines_to_bill`, a line already on a closed bill | no |
| `bill_already_closed` | `close_bill` | yes |
| `bill_has_unserved_lines` | `close_bill` | yes |
| `bill_has_no_lines` | `close_bill` | yes |
| `bill_not_closed` | `record_payment` | no, feature 15 |
| `session_collision` | `sessions::open` | no |
| `email_taken` | `accounts::register` | see below |

`email_taken` is the one kind that never reaches the wire. `taken_email` in `handlers/auth.rs`
already catches that conflict and turns it into an `already_taken` field error so the message lands
beside the email box rather than at the top of the form. That interception stays exactly as it is,
`handlers/auth.rs` is not touched by this slice, and no translation key is added for the code. Every
other kind in the table gets a key in the `common` namespace and a row in
`shared/api/error-message.ts`, including the ones this thread does not reach, so features 12 and 15
inherit them rather than discovering the gap.

**Value sourcing**

| Action | Value produced or displayed | Source |
|---|---|---|
| every request | which restaurant this is | the session cookie resolved by `Actor` on every request, passed to `Database::begin_scoped`. Nothing else may name a restaurant |
| every request | who acted | `Actor`'s staff id, used for `opened_by_staff_id`, `sent_by_staff_id`, `ready_by_staff_id`, `closed_by_staff_id` |
| floor read | whether a table is occupied | its `visits` row with `status = 'open'`, which the partial unique index guarantees is at most one |
| floor read | who opened an occupied table | `staff.display_name` joined through `visits.opened_by_staff_id` |
| floor read | whether a table has food ready | any round on the open visit whose status is `ready` |
| floor read | table order | `table_sections.position`, then `dining_tables.position`, then label |
| menu read | which dishes appear | `live_dishes`, which excludes archived rows and deliberately keeps unavailable ones. `is_available` rides out on each dish and decides whether the screen greys it and blocks the tap |
| menu read | which category a dish sits under | `dishes.category_id`. `live_dishes` returns a flat list in position order, so the handler groups it under `live_menu_categories` and drops a category with no live dish |
| menu read | a dish's price and currency | `dishes.price`, written with `restaurants.currency_code` and `currency_decimals` |
| open a table | the guest count | the optional field on the open form, stored as `visits.guest_count`, null when not given |
| send a round | the line's price and dish name | copied from the dish by `send_round` at send time. The client never sends a price |
| send a round | the round's number within the visit | `max(sequence_no) + 1` under the visit's row lock, allocated by `send_round` |
| send a round | which bill the lines belong to | the visit's one open bill, assigned by `assign_lines_to_bill` in the same transaction |
| kitchen read | how long a ticket has waited | `order_rounds.sent_at`, corrected by the clock difference below, never the device's own reading of `sent_at` |
| kitchen read, visit read | the server's current time | a `server_time` field on the response. At each fetch the screen computes `skew = Date.now() - Date.parse(server_time)` and hands `ElapsedTime` a `since` of `Date.parse(sent_at) + skew`, so a device clock that is wrong by any amount cancels out |
| kitchen read | when a ticket is late | `ElapsedTime`'s own 900 second default, which `docs/design.md` records as feature 13's decision to replace |
| mark a dish ready | the round's status afterwards | the total function over that round's lines, recomputed by the repository in the same transaction |
| mark a round served | which lines change | every line on the round whose status is `ready`, each through `mark_line_served`. Already served and voided lines are skipped, not refused |
| table screen | the running subtotal | `bills.subtotal`, which is true throughout the meal because lines are assigned at send |
| table screen, closed bill | how money is written | the restaurant's `currency_code` and `currency_decimals` from the cached identity, through the existing `shared/format` money formatter |
| close | the bill number, subtotal, service charge, tax lines, total, and all rounding | `close_bill` alone, unchanged from spec 0003. Nothing in this slice computes money |
| ready alert | which round to announce | a round id observed in `ready` that this screen has not yet announced, held in a per screen set so a refetch does not re announce |
| any screen | every user facing word | `t()` against the `waiter`, `kitchen`, and `common` namespaces. No string is written into a component |
| a failed request | the sentence shown | the response's `error` code mapped to a translation key by `shared/api/error-message.ts`. The API's English `message` is never rendered |
| live update | which queries to refetch | the event's entity kind, through the fan out map below |

The fan out map, which replaces the current invalidation of everything under `[event.entity]` and is
the narrowing `use-live-events.ts` already says feature 8 owes. Query keys start with the entity
kind they are built from: `['visit', 'floor']`, `['visit', visitId]`, `['order_round', 'kitchen']`,
`['dish', 'menu']`.

| Event entity | Invalidates |
|---|---|
| `visit` | `['visit']` |
| `order_round` | `['order_round']`, `['visit']` |
| `order_line` | `['order_round']`, `['visit']` |
| `bill` | `['visit']` |
| `dish` | `['dish']`, `['visit']` |
| `dining_table` | `['visit', 'floor']` |
| `staff` | `['visit', 'floor']` |
| `probe` | nothing |

**Key invariants**

1. No client ever names a restaurant. Every endpoint here scopes from the session and nothing else,
   and there is no path that accepts a restaurant id in a header, a query, or a body.
2. No client ever names a price. The send payload carries a dish id and a quantity, and the price
   and the name are copied from the dish inside `send_round`.
3. A visit and its bill are created together or not at all, and a bill closes and its visit closes
   together or not at all. Each pair is one scoped transaction.
4. Every line created by a send is assigned to the visit's open bill in that same transaction, so
   `bills.subtotal` is true at every moment of the meal and a close never has assignment left to do.
5. Nothing in this slice writes a round status, a money figure, a bill number, or a timestamp that
   spec 0003 already owns. Every such value comes out of a repository operation.
6. Every state change is a conditional update that names the state it expects, so a losing writer
   changes zero rows and receives a conflict rather than overwriting a colleague.
7. A round is announced to a waiter at most once per screen per transition into `ready`.
8. An event invalidates; it never writes. A screen that receives one goes back and asks for the
   rows, and row level security decides what it may have.
9. Every one of these endpoints is listed in `presentation/openapi.rs`, and the generated client is
   regenerated and committed in the same change.

**Security model**

- **Roles are strict and structural.** Waiter only: the floor, the menu, opening a table, sending a
  round, the visit document, marking a round served, and closing. Chef only: the ticket list and
  marking a dish ready. An admin reaches none of it in this slice, because feature 17 is the admin's
  own read only monitor and is scoped as its own row. The requirement is carried by the `Actor`
  extractor's type in each handler signature, so it is refused before the handler body runs and it
  reaches the OpenAPI document.
- **Any waiter may act on any table.** The schema records who opened a visit and never restricts on
  it, and a real floor hands tables over at a shift change. The floor shows the opener's name so the
  information is present without being a rule.
- **Tenant separation is unchanged and untouched.** Every read and write in this slice runs inside a
  `ScopedTx`, so the row level security policies apply as they already do. This slice adds no
  security definer function and no new bypass.
- **The browser's role gates are convenience, not control.** The three route groups already send a
  person to their own surface; the server refuses regardless of what the browser decided.
- **Compliance scope**: none new. The thread stores no personal data beyond what spec 0006 already
  stores about staff, and no payment data at all.
- **Audit log**: no new entries. Spec 0003 requires audit rows for voids, bill closes, price edits,
  and access control changes; the one of those this slice performs, closing a bill, already writes
  its audit row inside `close_bill`.

**Configuration required**

No new environment variables and no new secret. The seeded waiter and chef credentials are
constants in `api/src/bin/seed.rs` beside the existing admin's, documented in `.env.example`, for
the same reason the admin's are: a seeded password read from the environment is a password that
eventually reaches a real environment.

**Critical test scenarios**

- Happy path, two devices, one Playwright run: a waiter opens a table, sends two dishes, and the
  chef's context sees the ticket appear with no reload; the chef marks both dishes done, the ticket
  flips to ready, the waiter's context alerts and announces; the waiter marks it served, the ticket
  leaves the kitchen screen, and the bill closes with a number and a total while the table returns
  to free. Verifies **AC-1**, **AC-4**, **AC-5**, **AC-7**, **AC-8**, **AC-9**, **AC-11**,
  **AC-18**.
- Happy path, API level: the same sequence through the endpoints against a real Postgres, asserting
  the rows and the figures at each step. Verifies **AC-3**, **AC-4**, **AC-10**, **AC-11**.
- Failure case, occupancy: two concurrent opens of the same table leave one open visit, and the
  loser receives `409 table_occupied`. Verifies **AC-2**, **AC-13**.
- Failure case, the same dish twice: two concurrent ready marks on one line leave one winner, and
  the loser receives `409 line_not_queued`. Verifies **AC-13**.
- Failure case, the same round served twice: two concurrent serves of one round leave every line
  served exactly once, and the loser receives `409 round_not_ready` rather than a line level
  message. Verifies **AC-9**, **AC-13**.
- Failure case, an unavailable dish: an unavailable dish appears on the menu marked as such and
  cannot be added to the basket, and a send that names one anyway is refused by `send_round`.
  Verifies **AC-3**.
- Failure case, closing too early: closing while a dish is still queued is refused with
  `409 bill_has_unserved_lines`, and the waiter's screen shows the translated sentence for that
  code rather than the API's English. Verifies **AC-12**.
- Failure case, the stream drops: with the stream closed, the kitchen screen shows the connection
  warning and a mark still succeeds; on reopen, everything active refetches and the screen matches
  the database. Verifies **AC-15**, **AC-16**.
- Failure case, a wrong device clock: with the client clock offset by twenty minutes, a ticket sent
  one minute ago still reads about one minute. Verifies **AC-6**.
- Auth and permission: a chef calling every waiter endpoint, a waiter calling every kitchen
  endpoint, and an admin calling both, each receive `403`; a signed out caller receives `401`.
  Verifies **AC-14**.
- Isolation: a waiter of restaurant A cannot read or act on any visit, round, line, or bill of
  restaurant B, receiving `404` rather than `403` so the two are indistinguishable. Verifies
  **AC-14**.
- Web unit tests: the fan out map invalidates exactly the listed keys and nothing else; the alert
  fires once per round and not again on a refetch; the clock offset is applied to the age.
  Verifies **AC-8**, **AC-15**.
- Seed: running `pnpm db:seed` twice leaves one of each seeded row and three accounts. Verifies
  **AC-17**.
- Accessibility: both screens pass the lint rules, the axe pass, and render at their surface
  density; no literal user facing string exists in either. Verifies **AC-19**.

## Build plan

Tracer Bullet, and this feature is the thread itself, so the ordering is unusually literal: get the
narrowest possible path working end to end through every layer first, on one dish and one table,
then widen it to the whole loop, then harden it. Nothing below is a layer built out in full before
the next one starts. There is no migration task, because there is no migration.

1. Extend `pnpm db:seed` so the thread has something to run on: one table section with four tables,
   two menu categories with six dishes, one of them marked unavailable so the greyed state has
   something to draw, and a waiter and a chef account beside the admin, all created through the same
   repository operations the product uses. Make each part skip itself if it already exists rather
   than failing, and record the two new credentials in `.env.example`. Satisfies **AC-17**.
2. Turn `DomainError::Conflict` into the closed `ConflictKind` enum above, carrying the English
   sentence and a stable wire code, map that code into the error body in `presentation/error.rs`,
   convert all seventeen conflict construction sites across the four repository files to the
   matching kind, and add one translation key per wire code to the `common` namespace and to
   `shared/api/error-message.ts`, except `email_taken`, which stays intercepted by `taken_email` in
   `handlers/auth.rs` and never reaches a screen. Satisfies **AC-12**, **AC-13**.
3. The thread, top to bottom, on the narrowest path: the floor and menu reads, `POST /api/visits`
   and `POST /api/visits/{id}/rounds` (open plus bill, and send plus assign, each in one scoped
   transaction), the kitchen ticket read, and `POST /api/order-lines/{id}/ready`, each with its
   role in the handler's type and each listed in `presentation/openapi.rs`; then the waiter floor
   and ordering screens and the kitchen ticket list, wired through the generated client, so a dish
   sent on one device appears on the other. Regenerate and commit both the `.sqlx` cache and the
   TypeScript client. Satisfies **AC-1**, **AC-3**, **AC-4**, **AC-5**, **AC-7**, **AC-14**.
4. Narrow the live updates and make the clock honest: replace the blanket invalidation in
   `use-live-events.ts` with the entity keyed query keys and the fan out map, and add `server_time`
   to the kitchen and visit reads with the client side offset that corrects every displayed age.
   Satisfies **AC-6**, **AC-15**.
5. Close the loop back to the waiter: the visit document read, `POST /api/rounds/{id}/served` with
   its ready precheck and its skip rather than refuse rule for lines already served, the
   waiter's table screen with its rounds and running subtotal, and the ready alert that announces
   once per round, chimes when audio is unlocked, and carries the serve action. Satisfies **AC-8**,
   **AC-9**, **AC-10**.
6. End the meal: `POST /api/visits/{id}/close` running `close_bill` then `close_visit` in one
   transaction, the review panel listing the lines and the running subtotal, and the closed bill
   view showing the number, the service charge, each tax, and the total in the restaurant's
   currency. Satisfies **AC-11**, **AC-12**.
7. Make both screens honest when things go wrong: the prominent announced connection warning on
   both surfaces with every action still working, single flight mutations with a pending state on
   the tapped dish and no cache optimism, skeletons on first load, and empty states for a free floor
   and a quiet pass. Satisfies **AC-13**, **AC-16**.
8. Install Playwright, add its configuration and its continuous integration job against the Postgres
   service container, and write the one two device scenario that drives a waiter context and a chef
   context through the whole thread in a single run. Satisfies **AC-18**.
9. Fill in the rest of the checks and the words: the API integration tests for the failure cases and
   the isolation case, the web unit tests for the fan out map, the alert, and the clock offset, the
   English and Hindi keys for both new namespaces, and a pass over both screens for density,
   landmarks, and the lint and axe gates. Confirm `pnpm check`, `pnpm sqlx:check`, and
   `pnpm client:check` all pass. Satisfies **AC-2**, **AC-6**, **AC-14**, **AC-19**.

## Consequences

**Positive**

- The project's central claim stops being an argument and becomes a test that runs on every push.
  After this slice, any change that breaks the live path fails continuous integration rather than
  being discovered on a busy Friday.
- Every later slice thickens something proven. Features 9 through 15 change one segment of a working
  thread instead of assembling their own.
- The generated client, the OpenAPI document, and the entity keyed query cache all get their first
  real workout, on a surface small enough to fix cheaply if any of the three is awkward.
- Conflicts become legible to the person who has to act on them, in their own language, and every
  refusal features 12 and 15 add inherits that.
- A developer who clones the repository and runs two commands has a restaurant with a menu, tables,
  and three accounts, and can watch the whole loop work.

**Negative and tradeoffs**

- **Sending a round is not idempotent.** A timed out response and a second tap create a second
  ticket, and this slice ships no defence beyond a single flight button and a basket that clears
  only on success. The real fix is an idempotency key, which is a column, which is a migration this
  slice deliberately does not take. It is enrolled against feature 12.
- **Both screens are deliberately crude and will be visibly rebuilt.** The waiter's screen is
  replaced by feature 12 and the kitchen's by feature 13. That is the cost of a thread that is thin
  within each layer, and it will look like rework to anybody judging the app rather than the risk.
- **The conflict refactor touches a shared error type** used by features 6 and 7, at seventeen
  construction sites, and it defines codes for conflicts this thread never raises so that features
  12 and 15 inherit a complete set. It is contained and the tests keep compiling, and it is still a
  change to something already shipped and working, made for a feature that has not shipped.
- **Neither list endpoint paginates.** The floor is bounded by a restaurant's tables and the kitchen
  queue by the food a kitchen can physically have open, so both are small by nature rather than by
  design. That is an assumption about restaurants, not a guarantee about the API, and feature 13
  should give the kitchen read an explicit limit.
- **Playwright joins continuous integration and slows it down.** Browser tests are the slowest and
  flakiest thing a pipeline runs, and this one starts a database, an API, and a web build to check
  one scenario. Kept to one scenario for exactly that reason.
- **The waiter alert is per screen and lives in memory.** A waiter who reloads mid service may be
  alerted again about a round they already know about, and a waiter whose screen was closed when the
  food became ready is never alerted at all. Feature 12 owns making that reliable.
- **No dish can be cancelled.** With voiding out of scope, a dish sent by mistake stays on the
  ticket and must be marked served before the bill can close. It is survivable on a seeded
  development restaurant and would not be on a real one, which is why features 12 and 15 own it.

**Neutral**

- The kitchen's late threshold stays at the component's 900 second default, so the late emphasis is
  visible and the real per restaurant value is still feature 13's to choose.
- The development only `/api/dev/notify` endpoint and the `Probe` event kind stay exactly as they
  are. They prove the pipe with no data behind them, which stays useful even once real entities flow.
- Taxes and the service charge come out at zero, because no tax component is seeded and no
  restaurant has a service charge set. The close path still computes and writes them, so feature 14
  changes the numbers rather than the code that produces them.
- Three new handler modules and two new web feature folders appear, matched to the repository
  modules and to the surfaces that already exist. No new folder convention.

## Follow-up

- [ ] Sending a round needs an idempotency key so a retried send cannot create a second ticket.
      Enrol against feature 12 (waiter service flow), which owns the phone that will be used all
      evening on restaurant wifi, and which needs the column anyway.
- [ ] The kitchen read should carry an explicit limit and a defined behaviour past it. Belongs to
      feature 13 (kitchen display), together with the real late threshold that replaces the 900
      second placeholder.
- [ ] The ready alert survives neither a reload nor a closed screen. Feature 12 should decide
      whether a waiter is told about food that became ready while they were not looking.
- [ ] Features 9, 10, and 11 should delete the rows this slice seeds, or make the seed create them
      through the real administration paths once those exist, so the seeded restaurant does not
      quietly become the one restaurant whose menu was never created by a person.
- [ ] Spec 0003's `Notifies` column is the contract this slice relies on. Any endpoint added later
      that writes one of these entities without calling `notify_entity_change` will look correct and
      leave a screen stale. Worth a line in `api/AGENTS.md` when `/sync` next runs.
- [ ] Specs 0003, 0004, and 0005 are still `In Progress` while every box on features 4, 5, and 6 is
      ticked. This slice builds on all three as though they are done. Worth reconciling with `/sync`
      before this feature closes.
- [ ] Spec `0002-coding-standards-and-tooling` still has a `verify.md` and no `index.md`. Carried
      over from spec 0003's follow ups and still true.
