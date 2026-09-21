# 0012. Kitchen display

**Date**: 2026-09-21
**Status**: In Progress

## Summary

The kitchen pass exists but was built deliberately thin, and four earlier specs wrote down debts
against this feature. This spec thickens the same screen in place rather than building a second
one: tickets keep a cooking queue ordered oldest first, ready tickets move to their own area on
the same screen until a waiter collects them, and both age against two thresholds each restaurant
sets for itself (amber, then red). A chef can undo a tap, clear a whole ticket at once, and take
a dish off when the kitchen has run out. The rest of the work is making a screen nobody touches
tell the truth: a chime for new work, a loud banner when the live stream dies, a held scroll so
nothing moves under a hand mid tap, and a wake lock so the tablet does not go dark.

## Requirements

**User stories**:

- As a chef, I want new tickets to arrive on the pass by themselves, oldest first, so I cook in the
  order the tables ordered.
- As a chef, I want to see how long every ticket has waited and be warned before it is a problem,
  so I can move a table up rather than apologise for it later.
- As a chef, I want to tap each dish done as it leaves the pass, and take a tap back when I get it
  wrong, without leaving the screen or finding a waiter.
- As a chef, I want plated food I have finished to stay in front of me until somebody takes it, so
  a plate does not die under the lamp unnoticed.
- As a chef, I want to take a dish off a ticket when the kitchen has run out of it, so the waiter
  and the bill both learn straight away.
- As a chef, I want to know immediately when the screen has stopped receiving, because a quiet
  kitchen screen and a broken kitchen screen look identical.
- As an admin, I want to set what counts as late in my own restaurant, because fifteen minutes is
  slow for a salad and fast for a roast.

**Acceptance criteria** (the contract, each independently checkable):

- **AC-1**: A round a waiter sends appears on an already open pass with no refresh and no tap.
- **AC-2**: The cooking queue is ordered oldest first by the round's `sent_at`, and a ticket
  arriving takes its place at the end of that order without reordering the rest.
- **AC-3**: Every cooking ticket shows its age measured from `sent_at` against the server's clock,
  not the tablet's. It turns amber at the restaurant's warning threshold and red at its late
  threshold.
- **AC-4**: Tapping Done on a dish marks that dish ready and changes no other dish and no other
  ticket.
- **AC-5**: When the last still queued dish on a ticket is marked ready, the ticket becomes ready by
  itself, with no separate act, and leaves the cooking queue.
- **AC-6**: A ready ticket appears in a Ready area on the same screen and stays there until a waiter
  marks it served. It shows its own age measured from `ready_at`, going amber and red on the same
  two thresholds.
- **AC-7**: A chef can put a ready dish back to cooking. The dish returns to queued, its ready stamp
  and its ready by staff member are cleared, and a round that had gone ready returns to queued in
  the same transaction. A dish that is not ready is refused with `line_not_ready`.
- **AC-8**: An undo writes an audit row naming the chef, the dish, and the round.
- **AC-9**: An All done control on a ticket marks every still queued dish on it ready in one
  request and one transaction, so the ticket either fully flips or does not change at all.
- **AC-10**: A chef can void a dish from the pass with the reason kitchen unavailable. The dish is
  voided, the bill recomputes, every waiter screen updates, and the audit row records the chef as
  the actor.
- **AC-11**: A chef voiding with any reason but kitchen unavailable is refused with a named field
  error on `reasonCode`, and a dish that is already served or already cancelled is refused with the
  existing `line_not_voidable` conflict.
- **AC-12**: A voided dish leaves the cook list and appears in a clearly separated Cancelled strip
  at the bottom of its ticket, with its reason, flashing once at the moment it happens.
- **AC-13**: A dish note reaches the pass whole and wraps over as many lines as it needs. It is
  never truncated and never clipped.
- **AC-14**: A ticket arriving plays a kitchen chime audibly different from the waiter's ready
  chime, and marks the new card visibly.
- **AC-15**: While the browser has not yet allowed audio, the pass shows a kitchen sized prompt to
  enable sound, which goes away on the first interaction. Every alert still works with no sound.
- **AC-16**: A ticket arriving while the chef has scrolled does not move the scroll position. A
  marker says new work is above, with a control that returns to the top.
- **AC-17**: While the live stream is not connected, the pass shows a full width banner saying it is
  not live and dims the tickets. The banner clears on reconnect, together with the refetch.
- **AC-18**: The tablet's screen is held awake while the pass is open, and the hold is released when
  the chef leaves the pass.
- **AC-19**: The kitchen read returns at most 120 tickets, oldest first, and reports how many more
  it did not return, so the screen can say work is hidden rather than hide it silently.
- **AC-20**: An admin can set the warning and the late threshold, either one on its own or both at
  once. Each must be between 60 and 14400 seconds, and the warning must be below the late one when
  the submitted values are merged over the stored ones. Each refusal names the field that was wrong.
- **AC-21**: An edit of the restaurant naming a version that is no longer current is refused as a
  conflict, never merged over somebody else's change.
- **AC-22**: Only a chef may read the pass or act on it. A waiter or an admin gets a `403`. Only an
  admin may change the thresholds.
- **AC-23**: Every string on the pass is translated in English and in Hindi, and the screen has no
  serious or critical axe violation at kitchen density.
- **AC-24**: Two chefs tapping the same dish at once: one wins, the other gets a named refusal, and
  the losing screen refetches to the state the pass is really in.

## Decision

**Chosen option**: Option 1: thicken the existing pass in place, and settle every debt the earlier
specs recorded against this feature in the same pass.

The kitchen display stays the route and the component it already is. It grows a Ready area, a
second urgency level, an undo, a whole ticket clear, a chef void, and the four reliability
behaviours a wall mounted tablet needs. The only schema change is three columns on `restaurants`.

**Implementation skills**: `sqlx-postgres` (`daiki48/dotfiles`, `.agents/skills/sqlx-postgres/`) · `postgresql-table-design` (`wshobson/agents`, `.agents/skills/postgresql-table-design/`) · `axum-web-framework` (`manutej/luxor-claude-marketplace`, `.agents/skills/axum-web-framework/`) · `rust-backend` (`windmill-labs/windmill`, `.agents/skills/rust-backend/`) · `rust-best-practices` (`apollographql/skills`, `.agents/skills/rust-best-practices/`) · `tanstack-query` (`tanstack-skills/tanstack-skills`, `.agents/skills/tanstack-query/`) · `react-router-data-mode` (`remix-run/agent-skills`, `.agents/skills/react-router-data-mode/`) · `shadcn` (`shadcn/ui`, `.agents/skills/shadcn/`) · `tailwind-4-docs` (`lombiq/tailwind-agent-skills`, `.agents/skills/tailwind-4-docs/`) · `react-i18next` (`yildizberkay/skills`, `.agents/skills/react-i18next/`) · `vitest` (`antfu/skills`, `.agents/skills/vitest/`)

## Rationale

Reasoning, the options weighed, and the premise note: see [rationale.md](rationale.md).

## Feature design

**Design source**: `docs/design.md` and the current UI, which is what root `AGENTS.md` names as the
source of truth for all UI. The kitchen's density comes from `data-surface="kitchen"` on the
document, set by `RootLayout`, which spec 0004 established. No size class specific to the kitchen
belongs on these screens, and none is added here.

**Data model sketch**

Three columns added to `restaurants`, the table that already holds currency, timezone, and service
charge. No new table, no new enum value, no new index on a new column.

| Column | Type | Null | Default | Notes |
|---|---|---|---|---|
| `kitchen_warning_after_seconds` | `int` | not null | `600` | amber threshold, ten minutes |
| `kitchen_late_after_seconds` | `int` | not null | `900` | red threshold, fifteen minutes, the value the component hardcodes today |
| `version` | `int` | not null | `1` | optimistic concurrency, the project rule for an editable row |

Check constraints, named, on `restaurants`:

- `kitchen_warning_after_seconds BETWEEN 60 AND 14400`
- `kitchen_late_after_seconds BETWEEN 60 AND 14400`
- `kitchen_warning_after_seconds < kitchen_late_after_seconds`

The third is a constraint and not only a Rust check on purpose. Amber after red is a state the
screen cannot render sensibly, and a future handler that writes this row must not be able to
create it by forgetting a line. It is a backstop, not the expected error path: the handler checks
all three rules in Rust first so a refusal can name the field that was wrong (see AC-20).

**Indexes.** `kitchen_queue` already filters `WHERE r.status IN ('queued', 'ready')` and already
orders everything by `sent_at`, so reading ready rounds is not new. What is new is that ready
tickets now sort by `ready_at` and the whole read is capped. Two index changes follow:

- `order_rounds_queue_idx` is partial on `WHERE status = 'queued'`, which has never covered the
  ready half of a read that has selected it since spec 0007. Widen the predicate to
  `WHERE status IN ('queued', 'ready')` so the filter is indexed at all.
- Add `order_rounds_ready_idx` on `(restaurant_id, ready_at) WHERE status = 'ready'`. The widened
  index above still sorts only by `sent_at`, so without this one the Ready area's own ordering is
  a sort Postgres does after the fact. The ready set is small in practice; the index is cheap and
  makes the cap's `ORDER BY ... LIMIT` an index read rather than a sort of everything.

Read and not changed: `order_rounds` (`sent_at`, `ready_at`, `status`, `sequence_no`),
`order_lines` (`status`, `note`, `void_reason_code`, `ready_at`, `ready_by_staff_id`), `audit_log`
(two new `action` values).

**State transitions**

Line: `queued` → `ready` on a tap or an All done. `ready` → `queued` on an undo, allowed only while
the round is not served. `queued` or `ready` → `voided`. `ready` → `served` stays the waiter's act
and is untouched.

Round: `queued` → `ready` when its last queued line goes ready. `ready` → `queued` when an undo
takes a line back. `ready` → `served` stays the waiter's. Every round status write happens in the
same transaction as the line write that caused it, which is the rule spec 0003 set when it chose to
store a derived status.

**Interface surface**

| Endpoint | Method | Key inputs | Key outputs | Auth | Key errors |
|---|---|---|---|---|---|
| `/api/kitchen/tickets` | GET | none | `tickets` (at most 120, cooking oldest by `sentAt` then ready oldest by `readyAt`), `serverTime`, `warningAfterSeconds`, `lateAfterSeconds`, `truncatedCount` (number, `0` means nothing hidden) | chef | 401, 403, 503 |
| `/api/order-lines/{id}/ready` | POST | path id | 204 | chef | 401, 403, 404, 409 `line_not_queued` |
| `/api/order-lines/{id}/unready` | POST | path id | 204 | chef | 401, 403, 404, 409 `line_not_ready` |
| `/api/rounds/{id}/ready` | POST | path id | 204 | chef | 401, 403, 404, 409 `round_not_queued` |
| `/api/order-lines/{id}/void` | POST | path id, `reasonCode`, optional text | 204 | waiter or chef | 400 `fields.reasonCode = not_allowed_for_chef`, 401, 403 (wrong role), 404, 409 `line_not_voidable` |
| `/api/restaurant` | PATCH | `version` (required), optional `kitchenWarningAfterSeconds`, optional `kitchenLateAfterSeconds` | identity bundle, carrying the new `version` | admin | 400 per field, 401, 403, 409 stale version |

New routes go into `paths(...)` in `presentation/openapi.rs`, or they never reach the generated
client.

Three notes on the error shapes above, each chosen to match what the codebase already does rather
than to invent a fourth convention:

- **A chef voiding with the wrong reason is a `400`, not a `403`.** Every `403` in this API is the
  `Actor<R>` role refusal, decided before the handler body runs. A chef calling void is allowed to
  call it; the value of one field is what is refused, and a refused field value is a named field
  error everywhere else in this codebase.
- **Voiding a served or already cancelled dish reuses `line_not_voidable`**, the conflict the void
  handler already returns for exactly this. No second, narrower code is added.
- **The undo has no round level refusal.** `round_status_from_lines` yields `Served` only when every
  non voided line is served, so a round cannot be served while any line is still `ready`. The undo
  only ever targets a `ready` line, so its own conditional update, which yields `line_not_ready` on
  zero rows, already makes undo on a served round impossible. A separate round check would be a
  branch no interleaving can reach.

**Value sourcing**

| Action | Value produced or displayed | Source |
|---|---|---|
| read the pass | a cooking ticket's age | `order_rounds.sent_at`, corrected by `clockOffset(serverTime)` in `shared/events/server-clock.ts` |
| read the pass | a ready ticket's age | `order_rounds.ready_at`, same correction. The column already exists, added by spec 0002 |
| read the pass | when amber starts | `restaurants.kitchen_warning_after_seconds`, returned as `warningAfterSeconds` on this response |
| read the pass | when red starts | `restaurants.kitchen_late_after_seconds`, returned as `lateAfterSeconds` on this response |
| read the pass | that work is hidden | a count of matching rounds beyond the 120 taken, returned as `truncatedCount`, where `0` means nothing was left out |
| read the pass | a ticket's table and round number | `tables.label` through the visit, and `order_rounds.sequence_no`. Both already returned |
| read the pass | why a dish was cancelled | `order_lines.void_reason_code`, shown through a translation key per code, never the free text unless the code is `other` |
| read the pass | which cards are new | the ticket ids already rendered, held in component state and seeded from the first successful load. Not a server value: a ticket is new to this screen, not new in the database |
| mark a dish ready | who marked it | `order_lines.ready_by_staff_id` from the session's staff id, set with `ready_at` |
| undo a dish | the audit actor | the session's staff id, through `Actor` |
| undo a dish | the audit action name | the constant `order_line.ready_undone` |
| change thresholds | the `version` the admin screen submits | `restaurants.version`, carried to the client as a new field on `RestaurantDto` inside the identity bundle. The bundle is returned by `GET /api/me` and by every successful `PATCH /api/restaurant`, so a screen that just saved holds the new version without a second read |
| change thresholds | the other threshold, when only one is submitted | the stored column, read in the same transaction. Both fields are independently optional and the ordering rule spans both, so the handler merges what was sent over what is stored and checks the merged pair |
| change thresholds | the audit action name | the constant `restaurant.kitchen_thresholds_changed` |
| chime on a new ticket | the sound itself | generated through the existing `AudioContext` in `shared/ui/audio-unlock.ts`, at a pitch and pattern constant held in the kitchen alert module, deliberately unlike the waiter's |
| any alert | whether sound is allowed yet | `isAudioUnlocked()` from `shared/ui/audio-unlock.ts` |

**Key invariants**

1. The warning threshold is always below the late threshold. The handler checks the merged pair so
   it can name the wrong field, and the check constraint holds the line for any future writer.
2. A line can be undone only while it is `ready`. That is the whole rule, and it is enforced by the
   conditional update naming the status, not by a second check on the round.
3. A round's status is always derived from its lines and written in the same transaction as the
   line write that caused it. Nothing writes a round status on its own.
4. `ready_at` and `ready_by_staff_id` are set together and cleared together. A dish with one and
   not the other is a bug.
5. A chef's void is only ever `kitchen_unavailable`. The restriction lives in the handler, checked
   against the actor's role, not in the client.
6. The pass never writes a status into the query cache. An event invalidates and the screen asks
   again, which is the rule in `web/AGENTS.md` and the reason row level security stays the
   authority.
7. The service lock order is unchanged: visit, then line or round, then bill, then the bill number
   counter. `unready` and the round level ready take the line or round lock and touch no bill at
   all, because neither changes a price, a quantity, or which dishes are on the bill. Only the
   chef's void reaches the bill, and it does so on the path void already takes. Nothing here takes
   a visit lock after a bill lock.

**Security model**

The pass and every act on it are `Actor<Chef>`, carried in the handler signature so a wrong role is
refused before the body runs. The thresholds are `Actor<Admin>` on the existing restaurant patch.
An admin does not get a read only pass; the admin's live view over the floor is feature 17.

Every query goes through `Database::begin_scoped`, and the kitchen read keeps
`begin_scoped_snapshot`, which spec 0011 established for exactly this read: without one, a ticket
can be read as cooking while every dish on it reads ready, and the ticket then sits in the wrong
area of the screen.

No new personal data. The audit rows name a staff member, which `audit_log` already does
throughout. No compliance scope is triggered by this feature.

**Configuration required**

None. The thresholds are per restaurant data, set by an admin in the app, not environment
variables. Nothing here reads the environment, so `infrastructure/config.rs` is untouched.

**Critical test scenarios**

- Happy path: a waiter sends a round, it appears on an open pass with no refresh, ages, each dish is
  tapped, the last tap moves the whole ticket to the Ready area, verifies **AC-1**, **AC-3**,
  **AC-4**, **AC-5**, **AC-6**.
- Ordering: three rounds sent a minute apart show oldest first, and a fourth arriving lands at the
  end without reordering the others, verifies **AC-2**.
- Undo: a ready dish on a ready round goes back to cooking, the round returns to queued, the ready
  stamp is cleared, and an audit row exists, verifies **AC-7**, **AC-8**.
- All done atomicity: a four dish ticket cleared in one tap is one transaction. A forced failure
  part way leaves every dish queued, not two and two, verifies **AC-9**.
- Failure case, concurrency: two chefs tap the same queued dish at the same instant. One gets 204,
  the other `line_not_queued`, and the losing screen refetches, verifies **AC-24**.
- Failure case, concurrency: an undo and a waiter's serve of the same round race. Whichever loses is
  refused with a named code and no round ends up in a state its lines do not support. The undo's
  loss shows as `line_not_ready`, verifies **AC-7**.
- Failure case, stale edit: two admins read the restaurant, both save thresholds, the second is
  refused with a conflict, verifies **AC-21**.
- Validation: a warning above the late value, a value below 60, and a value above 14400 are each
  refused naming the field. Submitting only the warning, high enough to cross the stored late
  value, is refused the same way, which is the case the merge exists for, verifies **AC-20**.
- Auth: a waiter and an admin reading the pass or posting an undo each get `403`; a chef patching
  the restaurant gets `403`, verifies **AC-22**.
- Isolation: a chef of one restaurant never sees another restaurant's ticket, through the scoped
  transaction, verifies **AC-22**.
- Chef void: a chef voids with kitchen unavailable, the bill recomputes and the waiter sees it. The
  same chef voiding with guest changed mind is refused, verifies **AC-10**, **AC-11**.
- Cancelled presentation: a voided dish moves to the Cancelled strip with its reason and a long note
  renders whole across lines, verifies **AC-12**, **AC-13**.
- Cap: a restaurant with more than 120 open rounds gets 120 and a `truncatedCount` above zero, and
  the screen says work is hidden. A normal restaurant gets `0`, verifies **AC-19**.
- Stream death: the event stream is cut, the banner appears and the tickets dim; on reconnect the
  banner clears and the queue refetches, verifies **AC-17**.
- Scroll: a ticket arrives while scrolled down. The scroll does not move and the new work marker
  appears, verifies **AC-16**.
- Sound: with audio locked the prompt shows and the visual alert still fires; after an interaction
  the chime plays and the prompt goes, verifies **AC-14**, **AC-15**.
- Wake lock: the lock is requested when the pass mounts and released when the chef leaves it, and a
  browser that refuses the lock does not break the screen, verifies **AC-18**.
- Language and access: every new string exists in English and Hindi, and axe reports no serious or
  critical violation at kitchen density, verifies **AC-23**.

## Build plan

Tracer Bullet, which for this feature means the thinnest end to end thread that visibly changes the
pass goes first, and the reliability layer that a wall mounted tablet needs goes last, because each
of its four parts is independently useful and independently testable. The migration is one
migration at the front, because three columns with defaults are not worth slicing and every later
task reads them.

1. Migration `0010_kitchen_display.sql`: the three columns on `restaurants` with their defaults and
   the three named check constraints, widen `order_rounds_queue_idx` to
   `WHERE status IN ('queued', 'ready')`, and add `order_rounds_ready_idx` on
   `(restaurant_id, ready_at) WHERE status = 'ready'`. Run `pnpm sqlx:prepare`, satisfies **AC-3**,
   **AC-19**, **AC-20**
2. The thread, top to bottom: extend the kitchen read to carry the two thresholds and the 120 cap
   with its `truncatedCount`, order ready rounds by `ready_at` after the cooking ones (the read
   already filters on both statuses; what changes is the ordering and the cap), add the routes to
   the OpenAPI document, regenerate the client, and redraw the pass with the two urgency levels and
   the Ready area, satisfies **AC-1**, **AC-2**, **AC-3**, **AC-5**, **AC-6**, **AC-19**
3. Undo: the `unready` handler with its one refusal (`line_not_ready`), the round recompute, the
   cleared ready stamp, the audit row, and the control on a ready dish, satisfies **AC-7**, **AC-8**
4. All done: the round level ready endpoint in one transaction, and the control on the ticket
   header, satisfies **AC-4**, **AC-9**
5. The chef's void and the cancelled look: widen the void endpoint to a chef, refusing any reason
   but kitchen unavailable as a named field error on `reasonCode` and reusing the existing
   `line_not_voidable` for a served or already cancelled dish, then replace spec 0011's stopgap with
   the Cancelled strip carrying the reason and whole wrapped notes, satisfies **AC-10**, **AC-11**,
   **AC-12**, **AC-13**
6. The admin's thresholds, in two parts because the first is a contract change on its own:
   (a) add `version` to `restaurants` reads and to `RestaurantDto` so the identity bundle carries
   it, then make the restaurant patch require it and refuse a stale one. That check lands on the
   whole statement, so the settings fields that endpoint already writes (name, address, timezone,
   and the two language fields) come under it in the same change, not only the new ones.
   (b) the two optional threshold fields, merged over the stored row and checked as a pair so a
   refusal names the field, plus the section on the admin settings screen, satisfies **AC-20**,
   **AC-21**
7. The screen made honest: the distinct chime with the locked audio prompt, the held scroll with the
   new work marker, the not live banner with the dimmed queue, and the wake lock, satisfies
   **AC-14**, **AC-15**, **AC-16**, **AC-17**, **AC-18**
8. Finish: role and isolation tests, the two race tests, English and Hindi keys, axe at kitchen
   density, and the three metre legibility check spec 0005 asked this feature to make, satisfies
   **AC-22**, **AC-23**, **AC-24**

## Migration plan

**Strategy**: one migration, one deploy.

**Phases**:

1. The migration adds three columns, each not null with a default, so every existing row is valid
   the moment it lands and no backfill is needed. The two thresholds default to the values the
   screen already behaves as though it has, so nothing seeded changes meaning.
2. The API and the web app ship together from one repository in the same commit, which is what makes
   the required `version` on the restaurant patch safe to do in one step.

**Rollback**: revert the commit, drop the three columns and their constraints, and restore the
partial index to `WHERE status = 'queued'`. Nothing in the migration destroys data, so a rollback
loses only the thresholds an admin had set.

**Risks**: the required `version` on `PATCH /api/restaurant` is a breaking change to an endpoint
that already ships and already writes five fields (name, address, timezone, and the two language
fields). The version check is one conditional update, so those five come under it too, and this is
the change most likely to break something that currently works. Its only caller is the admin
settings screen in this repository, and the generated client stops compiling if that screen is not
updated in the same commit, which is the intended behaviour of the generated seam. A development
database whose restaurant row somehow carries a warning above the late value would fail the check
constraint on migration; there is no such row, because neither column exists yet.

## Consequences

**Positive**

- The kitchen finally has a screen built for a kitchen rather than a thread proving a pipe. Every
  behaviour a wall mounted tablet needs is present: it stays awake, it makes a noise, it says when
  it has stopped listening, and it does not move under a hand.
- Four recorded debts close in one place: spec 0004's and 0007's late threshold, spec 0007's
  unbounded kitchen read, spec 0011's cancelled dish and long note stopgap, and spec 0011's open
  question about chefs voiding with kitchen unavailable.
- Plated food has somewhere to be seen. Until now a ticket vanished at the moment it most needed
  watching, and the only thing tracking uncollected food was a waiter's phone in an apron.
- Undo removes the one irreversible act on the screen. The tap most worth taking back, the one that
  flips the whole ticket, is now the one a chef can take back.
- `restaurants` gets optimistic concurrency at last. The row has been editable through
  `PATCH /api/restaurant` since spec 0003 with last write wins, and the check lands on those five
  existing settings fields as well as the two new ones. Feature 14 inherits the pattern rather than
  adding it while also introducing currency and tax.
- Nothing new to operate. No dependency, no environment variable, no background job, no second
  source of truth. Three columns and a handful of handlers.

**Negative and tradeoffs**

- **The Ready area is a second alarm for something the kitchen cannot fix.** Feature 12 already
  chimes the waiter, badges it, and reminds every two minutes with an acknowledge. A kitchen side
  age that also goes red means two screens nagging about one plate, and the chef's only remedy is
  to shout. If waiters or chefs report the noise, soften this one, not the waiter's.
- **Undo until served lets a chef pull back food a waiter is already walking towards.** The waiter's
  screen corrects itself through the event, and the waiter is told nothing beyond the state change.
  Telling them explicitly was offered and left out.
- **Requiring `version` on the restaurant patch is scope from a kitchen feature landing on an admin
  screen, and it reaches further than the two new fields.** The same conditional update guards the
  five settings that endpoint already writes, so a kitchen feature changes how restaurant name and
  timezone edits behave. It is the right place for the column and it is still a breaking change to
  something already working, made while nothing is deployed, which is the only reason it is cheap.
- **Two thresholds per restaurant judge a two minute salad and a forty minute roast by one clock.**
  A kitchen doing both will see red tickets that are not late. Per dish prep minutes are the real
  answer and are deliberately not built here.
- **The chime cannot be promised.** Browsers block audio until somebody touches the page and a
  kitchen tablet may sit untouched for an hour. The prompt makes the silence visible and fixable; it
  does not make the sound reliable, and nothing on this screen may depend on it.
- **The wake lock is not universal.** Some browsers refuse it and some drop it when the tab is
  hidden. The fallback is a device setting somebody has to get right at install time.
- **The 120 cap can hide real work.** A restaurant with a stuck old round that nobody ever served
  accumulates, and `truncatedCount` says so without anybody being responsible for clearing it.
- **Auditing every undo adds rows on a path that may be common.** Done and Undo sit near each other
  on a screen operated with gloves, so `audit_log` will carry mis taps as well as real corrections.
- **The screen grows a lot at once.** A Ready area, two urgency levels, three new controls, a
  banner, a prompt, a marker and a chime all land on one surface. It is more state to hold on one
  screen than any other in the product.

**Neutral**

- No station routing, no course timing, and no per dish prep times. A split kitchen with a cold
  section and a grill is not modelled, and nothing here blocks modelling it later.
- Paper tickets stay with feature 16 and the admin's live view stays with feature 17. Neither is
  touched.
- `order_rounds.ready_at` already existed, added by spec 0002, so the Ready area's age needed no
  column. That is luck from a good data model, not planning.
- The kitchen Menu tab, where a chef marks a dish unavailable for future orders, is unchanged. It
  and the new void answer two different questions about the same shortage.
- `DEFAULT_LATE_AFTER_SECONDS` in `shared/ui/elapsed-time.tsx` stops being the real value and
  becomes at most a test fallback.

## Follow-up

- [ ] Per dish prep minutes, so a salad and a roast are not judged by one clock. It is a column on
      `dishes`, an admin field, and a change to how a ticket picks its threshold. Not enrolled;
      revisit when a restaurant with a wide menu complains about false red.
- [ ] Spec 0004's follow up that feature 13 owns the real late threshold is resolved here. `/sync`
      should tick it when this feature closes, and `DEFAULT_LATE_AFTER_SECONDS` should be removed
      from the component's public contract or clearly marked as a test only fallback.
- [ ] Spec 0007's follow ups on the kitchen late threshold and on the unbounded kitchen read are
      both resolved here. `/sync` should tick them when this feature closes.
- [ ] Spec 0011's follow up on cancelled dish presentation, long notes, and whether a chef may void
      with kitchen unavailable is resolved here. `/sync` should tick it when this feature closes.
- [ ] Spec 0005 asked this feature to confirm the type scale still reads at three metres. That is a
      measurement, not a code change; `/check verify` should carry it as a manual step rather than
      it being assumed by the token layer.
- [ ] Telling the waiter explicitly when a ready round is pulled back by an undo. Offered and left
      out. Revisit if waiters report walking to an empty pass.
- [ ] A station concept (cold, grill, pass) if a restaurant with a split kitchen signs up. It would
      change the read, the screen, and the menu, so it is its own feature and its own spec.
- [ ] Feature 14 inherits `restaurants.version`. Its currency and tax edits must name it too, or
      they reintroduce the last write wins this spec removes.
- [ ] An approval step before a chef writes off cooked food, if a restaurant asks. Spec 0011 raised
      the same question for waiters and parked it with features 17 and 18; a chef void widens who
      can do it, so the two should be answered together.
