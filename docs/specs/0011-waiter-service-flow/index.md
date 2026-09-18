# 0011. Waiter service flow

**Date**: 2026-09-18
**Status**: In Progress

## Summary

This turns the waiter's thin demo screens into the screen a waiter uses all evening. Beside the
floor there is now an Orders list of every open table with each round's live status, with ready
food at the top. Each table has a responsible waiter, who hears a chime (and feels a vibration)
the moment any of their dishes is ready, on whatever screen they are on, with a reminder every two
minutes until it is served. Waiters can add a note to each dish, serve dishes one at a time, cancel
a dish with a reason, take over or move a table, and resend safely on bad Wi Fi without making a
second ticket. It needs one small migration and no new infrastructure.

Reasoning and options: see [rationale.md](rationale.md).

## Requirements

**User stories**:

- As a waiter, I want one list of all my open tables with what is cooking and what is ready, so
  that I know where to walk next without opening each table.
- As a waiter, I want to hear that food is ready at one of my tables whatever screen I am on, so
  that food leaves the pass hot.
- As a waiter, I want to write "no onions" on a dish, so that the kitchen cooks what the guest asked
  for.
- As a waiter, I want to serve a starter the moment it is ready, even while the main course cooks.
- As a waiter, I want to cancel a dish sent by mistake, with a reason, so that the bill is right and
  the kitchen stops cooking it.
- As a waiter taking over at a shift change, I want to take a colleague's tables, so that their
  alerts come to me.
- As a waiter on patchy Wi Fi, I want to tap Send again after a timeout without making a second
  ticket.

**Acceptance criteria** (the contract, each independently checkable):

- **AC-1**: The waiter surface has two views, Floor and Orders, one tap apart. Floor stays the
  landing screen. Each occupied table card on Floor shows its responsible waiter's name and, when
  any dish on it is ready, a badge with the number of ready dishes.
- **AC-2**: Orders lists every open visit in the restaurant. For each: the table label, the
  responsible waiter's name, every round with its status and time since it was sent (against the
  server's clock), and the number of ready dishes. Tables with a ready dish come first, the one
  whose dish has waited longest at the top. Then tables with a dish still cooking, by their oldest
  unfinished round. Then the rest, by when they were opened. The list updates live without a
  refresh.
- **AC-3**: A "Mine" filter on both views shows only tables whose responsible waiter is the person
  signed in. The choice is remembered on that phone.
- **AC-4**: Whoever opens a table becomes its responsible waiter. Any waiter can take over a table:
  the request names who they expect to take it from, they become responsible, the change shows on
  every waiter's screen without a refresh, and one audit row records who took it from whom. If the
  responsible waiter changed in the meantime, the request is refused with `409 table_taken_over`
  and the screen refetches to show the real owner.
- **AC-5**: A waiter can send a second, third, or later round to an open visit. Each goes to the
  kitchen as its own ticket with its own round number within the visit, and every round shows on
  the table screen and the Orders list with its own status.
- **AC-6**: Each basket line can carry a free text note of at most 140 characters (counted as
  Unicode code points on both sides). Surrounding spaces are trimmed, and a blank note is stored as
  none. The same dish with two different notes is two basket lines and two order lines. A longer
  note is refused with `422` and a field error, and the database refuses it too. The note reaches
  the kitchen ticket exactly as typed and is shown in full, wrapped, never cut off.
- **AC-7**: A table's unsent basket and its send key are saved on the phone per visit (session
  storage). Leaving the table for another screen, or reloading, keeps it. It is cleared when a send
  succeeds, whether `201` or a `200` replay. If the send is refused with `visit_not_open`, it is
  discarded and a toast explains why. Because it is saved per visit, it follows a party that is
  moved to another table.
- **AC-8**: Every send carries a client made key (`clientKey`, a UUID) that stays the same until
  that basket is sent. Sending again with a key the server already holds for that visit creates no
  second round and no second kitchen ticket, and answers `200` with the first round. Two sends with
  one key at the same moment create exactly one round. A key already used on a different visit is
  refused with `409 client_key_reused`.
- **AC-9**: When a dish turns ready on a table whose responsible waiter is the person signed in, and
  the waiter app is open on any waiter screen, that phone shows a visible alert naming the table and
  the dish, announces it through the live region, plays the ready chime when audio is unlocked, and
  vibrates where the browser supports it. Dishes on that phone that turn ready within 3 seconds of
  the first join the same alert without a second chime. Every other waiter sees the ready badge
  with no sound and no alert. Each dish alerts at most once per phone.
- **AC-10**: While any ready and unserved dish on the signed in waiter's tables has not been
  acknowledged on that phone, the chime and vibration repeat every 2 minutes. Tapping Acknowledge
  on the alert, or opening that table's screen, acknowledges its ready dishes on that phone. Serving
  a dish ends its reminders everywhere.
- **AC-11**: After a reload or reopen, dishes that are already ready show their badges and their
  place at the top of Orders at once, with no chime on load. They join the 2 minute reminder cycle.
- **AC-12**: A waiter can serve one ready dish on its own, which moves only that line to `served`.
  Serving a dish that is not `ready` is refused with `409 line_not_ready`. "Serve all ready" on a
  round serves every ready dish on it, even while others still cook, and is refused with
  `409 nothing_ready` only when no dish on it is ready. The round's status always follows its lines
  (spec 0003), so the ticket leaves the kitchen queue once every dish is served or voided.
- **AC-13**: Any waiter can void (cancel) a dish that is `queued` or `ready`, choosing one reason:
  guest changed mind, entered by mistake, kitchen can't make it, or other. Text is required for
  "other" and optional for the rest, at most 200 characters. The void takes the line off the bill's
  running subtotal, recomputes the round's status, and writes one audit row with the reason, in one
  transaction. Voiding a dish that is served or already voided is refused with
  `409 line_not_voidable`.
- **AC-14**: On the kitchen screen, a voided dish stays on its ticket, struck through and labelled
  Cancelled, and is not tappable. A round whose every dish is voided leaves the queue. Notes are
  shown whole, wrapped.
- **AC-15**: Any waiter can move an open visit to a free live table. The party, its rounds, its bill,
  its responsible waiter, and any saved basket go with it. The move shows on every floor and on the
  kitchen tickets (the new table label) without a refresh, and writes one audit row. Moving to an
  occupied table is refused with `409 table_occupied`, and to an archived or unknown table with
  `404`.
- **AC-16**: Closing a visit whose bill has no line that is not voided voids that empty bill (no bill
  number used, one audit row), closes the visit, and frees the table. The waiter sees "Nothing to
  charge" instead of a total. Closing a bill that has dishes works as it does today.
- **AC-17**: Two people acting on the same dish or table at once leave exactly one winner, and the
  loser gets `409` with the code for what happened and a screen refetched to the truth. This holds
  for mark ready arriving after a void (`line_not_queued`), void arriving after a serve
  (`line_not_voidable`), two serves, two voids, two take overs, and two moves to one table. A void
  arriving after mark ready is not a conflict: a ready dish may be voided, so it succeeds.
- **AC-18**: Every new or changed endpoint is waiter only: a chef or an admin gets `403` before the
  handler body runs, and each appears in the OpenAPI document with its role. An id from another
  restaurant gets `404`.
- **AC-19**: Every new user facing word, including the four void reasons and every new conflict
  code, comes from the `waiter`, `kitchen`, or `common` namespaces, in English and Hindi. The new
  screens render at the waiter surface's density and pass the lint, axe, and contrast gates. The
  Floor and Orders switch and the alert work by keyboard.
- **AC-20**: The two device Playwright run covers the new loop. The waiter sends a round with a
  note, and the chef sees the note in full. The waiter then goes to Orders. The chef marks one dish
  ready. The waiter's Orders view alerts for that table without a reload, and the waiter serves that
  one dish. A second round sent to the same table appears as a separate ticket.

## Decision

**Chosen option**: Option 1: thicken the existing waiter screens in place, with every ready signal
worked out from server state.

One migration, one new read (`GET /api/orders/open`), five new writes, and three changed ones. The
existing repository operations are reused wherever they fit. The alert, the badges, and the
reminders all come from line status that is already streamed live. There is no push service, no
new table, and no new environment variable.

**Implementation skills**: `axum-web-framework` (`manutej/luxor-claude-marketplace`, `.agents/skills/axum-web-framework/`) · `rust-backend` (`windmill-labs/windmill`, `.agents/skills/rust-backend/`) · `rust-best-practices` (`apollographql/skills`, `.agents/skills/rust-best-practices/`) · `sqlx-postgres` (`daiki48/dotfiles`, `.agents/skills/sqlx-postgres/`) · `postgresql-table-design` (`wshobson/agents`, `.agents/skills/postgresql-table-design/`) · `react-router-data-mode` (`remix-run/agent-skills`, `.agents/skills/react-router-data-mode/`) · `tanstack-query` (`tanstack-skills/tanstack-skills`, `.agents/skills/tanstack-query/`) · `shadcn` (`shadcn/ui`, `.agents/skills/shadcn/`) · `react-i18next` (`yildizberkay/skills`, `.agents/skills/react-i18next/`) · `vitest` (`antfu/skills`, `.agents/skills/vitest/`)

## Rationale

Why in place over Web Push or a parallel rebuild, and each call made inside the design: see
[rationale.md](rationale.md).

## Feature design

**Data model sketch**

One migration, `api/migrations/0009_waiter_service_flow.sql`. No new table.

| Table | Change | Type and rule | Relationship |
|---|---|---|---|
| `visits` | add `responsible_staff_id` | `uuid`, added nullable, filled from `opened_by_staff_id`, then set `not null` | composite FK `(responsible_staff_id, restaurant_id)` to `staff (id, restaurant_id)`, many visits to one staff |
| `visits` | add index | `(restaurant_id, responsible_staff_id)` where `status = 'open'` | the Mine filter |
| `order_rounds` | add `client_key` | `uuid null`; unique index named `order_rounds_one_per_client_key` on `(restaurant_id, client_key)` where `client_key is not null`, so `conflict_on` can recognise it by name | none |
| `order_lines` | add `void_reason_code` | new enum `void_reason`: `guest_changed_mind`, `entered_by_mistake`, `kitchen_unavailable`, `other`; null unless voided | none |
| `order_lines` | replace check `order_lines_void_is_explained` | `status <> 'voided' OR (void_reason_code IS NOT NULL AND voided_by_staff_id IS NOT NULL AND voided_at IS NOT NULL AND (void_reason_code <> 'other' OR void_reason IS NOT NULL))`. Any row already voided is given `other` before the check is added. `void_reason` text becomes optional | none |
| `order_lines` | add checks | `note IS NULL OR char_length(note) BETWEEN 1 AND 140`; `void_reason IS NULL OR char_length(void_reason) BETWEEN 1 AND 200` | none |

The Rust side gains `VoidReason` in `domain/enums.rs` (mapped through `sqlx::Type`),
`responsible_staff_id` on the `Visit` entity, `client_key` on `OrderRound`, `void_reason_code` on
`OrderLine`, three `AuditAction` variants (`VisitTakenOver`, `VisitMoved`, `BillVoided`), and three
`ConflictKind` variants (`TableTakenOver`, `ClientKeyReused`, `LineNotVoidable`), plus `NothingReady`
replacing `RoundNotReady` (the wire code `round_not_ready` is retired along with its translation key).
The grants in `0002` already cover these columns. The migration changes no policy.

**State transitions**

Nothing new is invented. Every transition below is spec 0003's, now reached from the waiter.

- **Order line**: `queued` to `ready` (chef), `ready` to `served` (waiter, per dish or through serve
  all ready), `queued` or `ready` to `voided` (waiter, with a reason). `served` and `voided` are
  final.
- **Order round**: still never written directly. It is the total function of its lines, in spec
  0003's order: all voided makes it `voided`; otherwise all not voided lines served makes it
  `served`; otherwise no not voided line queued makes it `ready`; otherwise `queued`. So voiding the
  last cooking dish can make a round `ready`, and that raises the alert for its ready dishes.
- **Visit**: `open` to `closed` as before. While open, `responsible_staff_id` and `table_id` may
  change (take over, move).
- **Bill**: `open` to `closed` as before, or `open` to `voided` when the close finds no line on it
  that is not voided (new use of an existing state).

**Interface surface**

All endpoints are `Actor<Waiter>`, run inside one `ScopedTx`, are listed in
`presentation/openapi.rs`, and sit in `handlers/service.rs` or `handlers/billing.rs` beside their
neighbours.

| Endpoint | Method | Key inputs | Key outputs | Auth | Key errors |
|---|---|---|---|---|---|
| `/api/orders/open` | GET, new | none | `visits[]`: visit id, table id and label, opened at, responsible staff id and name, `rounds[]` (id, sequence no, status, sent at, `lines[]`: id, dish name, quantity, note, status, ready at); plus `server_time` | waiter | `401`, `403` |
| `/api/floor` | GET, changed | none | per occupied table adds responsible staff id and name; `foodReady` becomes `readyDishCount` | waiter | unchanged |
| `/api/visits/{id}` | GET, changed | path id | visit adds responsible staff id and name; each line adds `readyAt`, `voidReasonCode`, `voidReason` | waiter | unchanged |
| `/api/visits/{id}/rounds` | POST, changed | `clientKey` uuid (req); `lines[]`: `dishId`, `quantity`, `note` (opt, at most 140) | the round with its lines; `201` new, `200` replay | waiter | `409 visit_not_open`, `409 dish_not_orderable`, `409 client_key_reused`, `422` note too long |
| `/api/visits/{id}/take-over` | POST, new | `expectedStaffId` uuid (req) | visit id, responsible staff id and name | waiter | `409 table_taken_over`, `409 visit_not_open`, `404` |
| `/api/visits/{id}/move` | POST, new | `tableId` uuid (req) | visit id, table id and label | waiter | `409 table_occupied`, `409 visit_not_open`, `404` unknown or archived table |
| `/api/visits/{id}/close` | POST, changed | path id | the bill: closed with figures as today, or `status: voided` with zero figures and no number | waiter | `409 bill_has_unserved_lines`, `409 bill_already_closed` (also when the empty bill was already voided or closed by someone else) |
| `/api/order-lines/{id}/served` | POST, new | path id | the line, and its round's status after the write | waiter | `409 line_not_ready`, `404` |
| `/api/order-lines/{id}/void` | POST, new | `reasonCode` (req, one of four), `reason` (opt, required for `other`, at most 200) | the line, the round's status after, the bill's subtotal after | waiter | `409 line_not_voidable`, `422` missing or long reason, `404` |
| `/api/rounds/{id}/served` | POST, changed meaning | path id | the round with its lines | waiter | `409 nothing_ready`, `404` |
| `/api/kitchen/tickets` | GET, unchanged | none | already returns every line of a queued or ready round, `voided` ones included, through `lines_for_round`. Only the kitchen screen changes | chef | unchanged |

Repository work, each in one scoped transaction:

- **`open_orders`** (new read): open visits joined to table and responsible staff, their rounds and
  lines. Two queries (visits, then rounds and lines for those visit ids), grouped in the handler.
- **`send_round`** gains `client_key`. Under the visit row lock it already takes, it first looks for
  a round with that key in the restaurant. If it is found on this visit, it returns that round and
  its lines with a replay flag, sending no notify. If found on another visit, it raises
  `ClientKeyReused`. The unique index `order_rounds_one_per_client_key` is the final guard, mapped
  to `ClientKeyReused` by `conflict_on`. On a replay the handler skips `assign_lines_to_bill`
  entirely (so no subtotal recompute and no `bill` notify), commits, and answers `200`. Each stored
  note is `note.trim()`, and a note blank after trimming is stored as null; today's handler only
  tests for blank and stores the untrimmed text, which changes here.
- **`take_over_visit`** (new): `UPDATE visits SET responsible_staff_id = actor WHERE id = $1 AND
  status = 'open' AND responsible_staff_id = expected`. Zero rows means reading the row again to
  raise `TableTakenOver` or `VisitNot(Open)`. Then the audit row, then notify `visit`.
- **`move_visit`** (exists): the handler adds the `VisitMoved` audit row and notifies
  `order_round` for each round on the visit in `queued` or `ready`, so kitchen tickets show the new
  label. This is the one deliberate notify with no write to that table: `kitchen_queue` reads the
  label live through the visit, so the rounds did change as the kitchen sees them. Say so in a
  comment at the call.
- **`void_line`** (changed): takes `VoidReason` plus optional text, audits the code and text, then
  recomputes the line's bill subtotal itself, in the repository, before returning. The zero rows
  case goes through a new `void_conflict` resolver (the shared `line_conflict` assumes one expected
  status, and a void expects either of two), which returns `LineNotVoidable` when the line exists
  and `NotFound` when it does not. The bill recompute reuses `billing::recompute_subtotal`, made
  `pub(crate)` for this: it locks the bill row, sums `line_total` over its lines that are not
  voided, writes `subtotal`, and notifies `bill`.
- **`mark_line_served`** (exists): reached per dish by the new endpoint.
- **Serve all ready** (changed handler): refuses with `NothingReady` unless at least one line on the
  round is `ready`; then marks each `ready` line served, skipping others. A line conflict inside the
  loop is reported as `NothingReady`.
- **`void_empty_bill`** (new): `UPDATE bills SET status = 'voided' WHERE id = $1 AND status =
  'open' AND NOT EXISTS (a line on it that is not voided)`, under the bill row lock. Audit
  `BillVoided`, notify `bill`. The close handler calls it instead of `close_bill` when the bill has
  no line that is not voided, then `close_visit`. If it changes zero rows, the bill is read again
  under its lock: already `closed` or `voided` gives `409 bill_already_closed`; still `open` means
  a line that is not voided exists after all, so the handler falls through to `close_bill`, which
  closes it or refuses with `bill_has_unserved_lines`.

**Lock order.** Every path that takes more than one lock takes them in this order: visit, then
round or line, then bill, then the bill number counter. Send takes visit then bill. Void and
serve take line and round then bill. Close today takes the bill, then the counter, then updates
the visit row (an implicit visit lock after a bill lock), which can deadlock against a send on the
same visit. So the close handler now first takes `SELECT ... FROM visits WHERE id = $1 FOR UPDATE`,
before `close_bill` or `void_empty_bill`, and the order holds everywhere. After this change no path
takes a visit lock after a bill lock. A race test runs a send and a close on one visit at once
and expects a clean outcome (one refused with `409`), never a deadlock error. Write this order in a comment in `service.rs` and in
`api/AGENTS.md` when `/sync` runs, because feature 23 (splitting bills) will lock two bills at
once and must fit into it (lower bill id first).

**Value sourcing**

| Action | Value produced or displayed | Source |
|---|---|---|
| every request | the restaurant, and who acted | `Actor`, as in spec 0007. No client names a restaurant |
| open a table | the responsible waiter | the actor, written to both `opened_by_staff_id` and `responsible_staff_id` by `open_visit` |
| Orders, Floor | responsible waiter's name | `staff.display_name` joined through `visits.responsible_staff_id` |
| Mine filter | who "me" is | `StaffDto.id` from the cached identity (`/api/me`), compared with each visit's `responsibleStaffId` |
| Mine filter | remembering the choice | `localStorage` key `waiter.mineOnly`, read and written in try/catch; defaults to off |
| Orders | ready dish count per table | count of lines with `status = 'ready'` across the visit's rounds |
| Orders | sort order | derived on the client from the response: first the oldest `readyAt` among ready lines, then the oldest `sentAt` among rounds with a queued line, then `openedAt` |
| Orders, table screen | time since sent, time since ready | `sentAt` and `readyAt` corrected by `clockOffset(server_time)`, the existing `shared/events/server-clock` |
| alert | which dishes are newly ready | line ids in `ready` on visits whose `responsibleStaffId` is me, not in this phone's in memory "seen" set. On the first successful Orders load, every line already ready is added to "seen" with no chime (AC-11) |
| alert | whether this phone should chime | `responsibleStaffId` equals me. Otherwise badge only |
| alert | grouping | the first new line chimes and opens the alert. New lines within 3000 ms join it silently. A constant in the waiter alert module |
| reminder | when to chime again | one 120000 ms interval, running while the set (ready lines on my tables) minus (acknowledged on this phone) is not empty |
| reminder | acknowledged dishes | a set of line ids in `sessionStorage` key `waiter.acknowledged`, added to by Acknowledge or by opening that visit's table screen, pruned of ids no longer ready. Read and written in try/catch; if storage fails the set lives in memory, so acknowledging still stops reminders until a reload |
| alert | the chime, the announcement, the vibration | `playReadyChime` and the live region through the existing `Alert`; `navigator.vibrate([200, 100, 200])` when the function exists |
| send | `clientKey` | `crypto.randomUUID()` made when the basket gets its first line, saved with it, replaced only after a successful send |
| send | the saved basket | `sessionStorage` key `waiter.basket.<visitId>` holding lines (dish id, quantity, note) and the key. Read and written in try/catch; if storage fails the basket lives in memory |
| send | a line's note | the basket line's note, trimmed, blank sent as absent; length measured with `[...note].length` on the phone and `char_length` in Postgres |
| send, replay | the answer | the stored round whose `client_key` matches, with its lines |
| serve one | which line | the path id; the round's status after comes from `recompute_round_status` |
| void | reason code and text | the request body; the four codes map to translation keys `waiter:void.reason.<code>` |
| void | bill subtotal after | sum of `line_total` over the bill's lines that are not voided, recomputed in the same transaction |
| take over | whom the waiter expects to replace | `responsibleStaffId` from the visit or Orders data the screen is showing |
| move | which tables can be chosen | the Floor read, free live tables only |
| move | the new table label on kitchen tickets | `dining_tables.label` through the round's visit, refetched after the `order_round` notify |
| close, empty bill | "nothing to charge" | the close response's `status: voided` |
| kitchen | whether a dish is cancelled | the line's `status = 'voided'` |
| any screen | every word | `t()` in the `waiter`, `kitchen`, and `common` namespaces; conflict codes through `shared/api/error-message.ts` |
| live update | which queries refetch | the fan out map. The Orders key is `['visit', 'open']`, already under `['visit']`, so `visit`, `order_round`, `order_line`, `bill`, and `dish` events reach it. The `staff` entry changes from `floorKey` to `['visit']`, so a renamed waiter updates Orders too |

**Key invariants**

1. At most one round per `(restaurant, client_key)`. A retried send never makes a second kitchen
   ticket.
2. `visits.responsible_staff_id` is never null and always names a staff member of the same
   restaurant (composite foreign key).
3. `bills.subtotal` equals the sum of `line_total` over its lines that are not voided, after every
   send and every void.
4. A voided line always has a reason code, a person, and a time, and has text when the code is
   `other`. Enforced by the check constraint.
5. Every state change is a conditional update naming the state it expects: take over names the
   expected owner, void names `queued` or `ready`, serve names `ready`.
6. An empty bill is voided, never closed, so bill numbers stay gapless.
7. No alert state is stored on the server. Ready means `order_lines.status = 'ready'`, and a phone
   keeps only what it has chimed for and what it has acknowledged.
8. The chime and the pop up only go to the responsible waiter's phones. Everyone sees the badge.
9. Every write here notifies its entity inside its own transaction, as spec 0003's Notifies column
   requires.

**Security model**

- **Waiter only**, carried in the handler type, on every endpoint in the table except the kitchen
  read, which stays chef only. Admin reaches none of it, as in spec 0007.
- **Any waiter may act on any table**, including voiding, taking over, and moving. Responsibility
  decides who hears the chime, not who may act. This is the engineer's choice, and every void, take
  over, and move is audited with the actor, so the admin can review it later (feature 18).
- **Tenant separation unchanged**: every read and write runs in a `ScopedTx`, and row level security
  applies. Another restaurant's visit, line, round, or table id gives `404`.
- **Audit**: `LineVoided` (existing, now with the code), `VisitTakenOver` (before and after
  responsible staff), `VisitMoved` (before and after table), `BillVoided`.
- **Compliance scope**: none new. Notes and void texts are free text typed by staff. The screens say
  they are for kitchen instructions, not guest personal details.

**Configuration required**

None. No new environment variable or secret.

**Critical test scenarios**

- Two devices, one Playwright run: a note reaches the chef in full; the chef marks one dish ready;
  the waiter, on Orders rather than that table, is alerted without a reload and serves that one
  dish; a second round shows as a separate ticket. Verifies **AC-5**, **AC-6**, **AC-9**,
  **AC-12**, **AC-20**.
- API, idempotency: the same key sent twice gives one round, `201` then `200` with the same round
  id; two concurrent sends with one key give one round; the key on another visit gives
  `409 client_key_reused`. Verifies **AC-8**.
- API, notes: 140 Hindi characters are accepted and read back unchanged; 141 give `422`; a direct
  insert of 141 is refused by the check. Verifies **AC-6**.
- API, void: voiding a queued line lowers the bill subtotal by its line total and writes one audit
  row with the code; voiding the last queued line on a round with ready lines flips the round to
  `ready`; voiding every line makes the round `voided` and drops it from the kitchen queue;
  `other` without text gives `422`; voiding a served line gives `409 line_not_voidable`. Verifies
  **AC-13**, **AC-14**.
- API, serving: serving one ready line leaves the others; serve all ready with nothing ready gives
  `409 nothing_ready`; serving a queued line gives `409 line_not_ready`. Verifies **AC-12**.
- Race: void then mark ready (mark ready loses with `line_not_queued`), mark ready then void (both
  succeed, line ends `voided`), serve then void (void loses with `line_not_voidable`), two serves,
  two voids, two take overs naming the same owner, and two moves to one table each end as stated
  (using `until_blocked`, as the existing race tests do). Also a void of the last chargeable line
  racing an empty close: exactly one of them wins and the bill ends consistent. Verifies **AC-4**,
  **AC-15**, **AC-16**, **AC-17**.
- API, replay: a replayed send writes nothing, sends no `bill` notify, and returns `200`; a note
  sent as `"  no onions  "` is stored as `no onions`. Verifies **AC-6**, **AC-8**.
- API, empty close: a visit whose only line is voided closes with the bill `voided`, no number used,
  and the next closed bill takes the next number. Verifies **AC-16**.
- API, move: the move carries rounds and bill; the kitchen ticket reads the new label; moving to an
  occupied table gives `409 table_occupied`. Verifies **AC-15**.
- Auth: a chef and an admin get `403` on every new endpoint; a waiter of restaurant B gets `404` on
  restaurant A's ids. Verifies **AC-18**.
- Web unit: the Orders sort; the Mine filter; the alert chimes once, groups within 3 seconds,
  stays silent for another waiter's table and for lines already ready on first load; the reminder
  fires at 2 minutes and stops on acknowledge and on serve (fake timers); the basket survives a
  remount from session storage, clears on `200` replay, and is dropped on `visit_not_open`.
  Verifies **AC-2**, **AC-3**, **AC-7**, **AC-9**, **AC-10**, **AC-11**.
- Web unit: the fan out map sends `staff` to `['visit']`; the kitchen renders a voided line struck
  through and a 140 character note unclipped. Verifies **AC-14**, **AC-19**.
- Accessibility: Floor, Orders, the void dialog, the move picker, and the alert pass axe and the
  lint rules; every new key exists in Hindi. Verifies **AC-1**, **AC-19**.

## Build plan

Tracer Bullet. The first task pushes the new thread (a second round with a note, one dish ready, an
alert on a different screen, one dish served) through the migration, the API, both screens, and the
browser test, on the smallest surface that proves it. Then each later task thickens one segment.
The whole migration lands in the first task because every column in it is small and the thread
needs two of them (the responsible waiter for the alert, the client key for a safe send).

1. The thread, top to bottom: migration `0009` in full; `responsible_staff_id` written by
   `open_visit`; `client_key` on `send_round` with replay and `client_key_reused`; the note limit
   on the send request; `GET /api/orders/open`; `POST /api/order-lines/{id}/served`; the Floor and
   Orders switch with the Orders list in plain form; a note field on basket lines; the waiter alert
   module mounted in the waiter shell, chiming for the responsible waiter; the kitchen showing
   notes unclipped; and the Playwright scenario extended as AC-20 describes. Regenerate and commit
   `.sqlx` and the TypeScript client. Satisfies **AC-5**, **AC-6**, **AC-8**, **AC-9**, **AC-12**,
   **AC-20**.
2. The alert made reliable: grouping within 3 seconds, vibration, the 2 minute reminder, Acknowledge,
   the acknowledged set in session storage, seeding "seen" on first load, the ready badge count on
   Floor (`readyDishCount`), and the Orders sort. Satisfies **AC-1**, **AC-2**, **AC-9**,
   **AC-10**, **AC-11**.
3. Ownership: responsible waiter names on Floor and Orders, the Mine filter, `take-over` with its
   conditional update and audit, the `staff` fan out change, and the take over race test.
   Satisfies **AC-1**, **AC-3**, **AC-4**, **AC-17**.
4. The basket that survives: per visit session storage with its key, clearing on `201` and `200`,
   discarding on `visit_not_open`, and serve all ready with its new meaning and `nothing_ready`.
   Satisfies **AC-7**, **AC-12**.
5. Voids, moves, and the empty close: the void endpoint with the reason dialog and bill recompute,
   the kitchen's Cancelled line, `move` with its free table picker and audit, `void_empty_bill` in
   the close path, the visit lock taken first in the close handler, and their race tests. Satisfies **AC-13**, **AC-14**, **AC-15**, **AC-16**,
   **AC-17**.
6. Finish: `403` and `404` tests for every endpoint, the retired `round_not_ready` key removed, every
   new key in English and Hindi, axe and density passes, and `pnpm check`, `pnpm sqlx:check`,
   `pnpm client:check`, and `pnpm e2e` green. Satisfies **AC-18**, **AC-19**.

## Migration plan

**Strategy**: one migration, one deploy. Nothing is deployed yet (`infra/AGENTS.md`), so there is no
live traffic to protect.
**Phases**:
1. `0009` adds `responsible_staff_id` nullable, fills it from `opened_by_staff_id`, sets it
   `not null`, and adds the foreign key and index. It gives any already voided line the code
   `other`, cuts any existing `void_reason` longer than 200 characters down to 200 and any existing
   `note` longer than 140 down to 140 (both were unlimited before), then replaces the void check
   and adds the length checks. `client_key` is added nullable, with no backfill.
2. The API and web ship in the same commit, because the send body and the floor response change
   shape together through the generated client.
**Rollback**: revert the commit and drop the added columns, enum, index, and checks, restoring the
old void check. Data typed into the new columns (take overs, reason codes, keys) is lost, and the
old `void_reason` text stays.
**Risks**: a development database with a voided line whose `void_reason` is null would fail the
new check. The old check forbids that, so none can exist. Old text longer than the new limits is
cut rather than refused, which loses its tail; only development data exists, so this is accepted.

## Consequences

**Positive**

- The waiter hears about ready food wherever they are in the app, and so does nobody else, so the
  floor stays quiet.
- A reload, a second screen, or a handover loses nothing, because "ready" is read from the database
  every time.
- The Wi Fi double ticket named in spec 0007 is closed by the database, not by hope.
- Feature 15 gets voids and an empty table close it can build on, and feature 18 gets void reasons
  it can group.

**Negative and tradeoffs**

- **A locked phone is silent.** Web Push was declined, so a waiter whose screen is off learns of
  ready food only when they look, and then within 2 minutes by reminder. This is the biggest gap
  left, and it is deliberate.
- **Any waiter can void any dish, including cooked food**, with only the audit log behind it. A
  restaurant that sees write offs will want an approval step, which this does not have.
- **The Orders read returns every open line on every line change.** A busy floor with 30 open
  tables refetches a few hundred rows whenever any dish anywhere changes. That is fine at one
  restaurant's volume and is the first thing to measure if the waiter phones feel slow.
- **Alert memory is per browser tab, not per phone.** A waiter using two phones, or two tabs of the
  app on one phone, gets two chimes, and acknowledging in one leaves the other reminding. Session
  storage belongs to a tab, and some browsers copy it into a duplicated tab. Accepted: one tab per
  phone is how the app is used.
- **The kitchen screen is touched ahead of feature 13**, only to show whole notes and cancelled
  dishes. Feature 13 will redraw both.
- **The serve all ready endpoint changes meaning under the same path.** Any code or test relying on
  "only when the whole round is ready" changes with it, and `round_not_ready` disappears.

**Neutral**

- Close keeps working exactly as spec 0007 built it for a bill with dishes. Tax, service charge, and
  payment stay with features 14 and 15.
- Guest count on open, changing a sent dish, and "same again" were offered and left out.
- The waiter shell now owns one always mounted query (Orders) and one timer (reminders), both
  stopped when the waiter leaves the waiter surface.

## Follow-up

- [ ] Web Push for a locked phone, if waiters report missing food. It needs a service worker, VAPID
      keys in config, and a subscriptions table, and on iOS it works only for home screen installs.
      Not enrolled; revisit after real use.
- [ ] An approval step for voiding food that is already cooked, if a restaurant asks for it.
      Belongs with feature 17 or 18, where the admin already looks.
- [ ] Feature 13 (kitchen display) should design how voided dishes and long notes look properly,
      replacing the stopgap here, and decide whether chefs may void with `kitchen_unavailable`.
- [ ] Feature 18 (sales reports) can group voids by `void_reason_code` and by staff member.
- [ ] Spec 0007's follow ups on idempotency and on the alert surviving a reload are resolved by this
      spec. `/sync` should tick them when this feature closes.
- [ ] The dev database has tables 1, 3, and 4 occupied and the e2e needs table 2 free. The extended
      Playwright scenario must still pick table 2, or free what it uses at the end.
