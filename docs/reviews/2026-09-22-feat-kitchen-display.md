# Review, feat/kitchen-display, 2026-09-22

**Reviewed by**: Claude Sonnet 5 (author on Claude Sonnet 5)
**Scope**: 78 files (76 tracked + 2 untracked), branch vs main (merge base 3c928a1)
**Verdict**: Changes requested

## Summary

This lands spec 0012 end to end: three columns and two widened indexes on `restaurants`/
`order_rounds`, a Ready area with its own `ready_at`-ordered read, undo, All done, a chef's
kitchen-unavailable void, optimistic concurrency on the whole restaurant settings row, and the four
reliability behaviours (chime, scroll marker, not-live banner, wake lock). The domain layer is
genuinely careful: `merge_kitchen_thresholds` is exhaustively unit tested for the exact case it
exists for (raising one threshold across the other's stored value), the two race tests use the
existing `until_blocked` harness correctly, tenant scoping and the snapshot-read requirement are
both respected, money/timestamp/layering rules all hold, and the translation files are in exact key
parity across English and Hindi (checked programmatically). No correctness bug survived tracing
through the transaction and locking logic. The gap is test coverage on two pieces of genuinely new
branching logic: the admin settings screen's stale-conflict handling (AC-21) and the new-ticket
chime/scroll-marker behaviour (AC-14, AC-16), both exercised nowhere in the diff despite being
explicit spec test scenarios and despite the project's own Vitest conventions (proven elsewhere in
this same diff) being fully capable of testing them.

## Major

### 🟠 The admin settings stale-conflict path (AC-21) is untested, and its own behaviour looks questionable, `web/src/admin/routes/restaurant-settings.tsx:143-173`

**Problem**: `submit()` sets `stale` (owned by the parent `RestaurantSettings`, not by
`SettingsForm`) to `true` on a `409 restaurant_changed`, then invalidates the identity query so the
form remounts (keyed on `restaurant.version`) with the winner's data. `onStale(false)` is only
called at the top of the *next* `submit()` — nothing clears it when the remount itself lands. So
once an admin loses a race, the "Somebody saved these settings first" banner (`data-testid=
"settings-stale"`) appears to stay up indefinitely above the now-fresh form, even though the form
underneath it is already showing current data, until the admin submits again. `restaurant-settings.
test.tsx` has no test that mocks a `restaurant_changed` response, so neither the banner's appearance
nor this persistence question is pinned anywhere.

**Why it matters**: this is exactly the kind of new branching UI logic (AC-21's whole reason for
being) that a test would nail down. As written, an admin who loses a race and then looks back at the
screen a minute later still sees "somebody saved these settings first" sitting over values that are,
in fact, now correct and current — a stale-looking warning about data that is no longer stale. The
project's own test infrastructure is fully capable of exercising this (see `kitchen-home.test.tsx`'s
`'tells a chef who lost a race what happened, in words they can read'`, an equivalent conflict-UI
test for the kitchen screen, in this same diff), so this isn't a case of the pattern being
untestable — it simply wasn't written.

**Suggested fix**: Add a test that mocks `api.PATCH` returning `restaurant_changed`, asserts the
banner appears and the form remounts with the refetched values. Decide (and encode in the test)
whether the banner should clear once the remount lands — if the intent really is "stays up until the
next attempt", say so in the component's doc comment, because right now it reads as an oversight.

### 🟠 The chime and the scroll-preserving new-work marker (AC-14, AC-16) have no automated test anywhere, `web/src/kitchen/alerts/use-new-tickets.ts`

**Problem**: `useNewTickets` is the hook that announces, chimes, and computes `aboveTheFold` for the
"New work above" banner — the whole mechanism behind AC-14 ("marks the new card visibly" / plays the
chime) and AC-16 ("the scroll does not move... a marker says new work is above"). There is no
`use-new-tickets.test.ts` (unlike its sibling `use-fresh-ids.test.ts`, which is thoroughly tested),
and `kitchen-home.test.tsx` never simulates a ticket-set transition: every test in that file sets one
static `api.GET` response via `respondWith`/`mockResolvedValue` and never re-resolves with a
different ticket set mid-test, so the actual arrival path — `isNew` badge from a real refetch,
`playKitchenChime()` firing, `new-work-above` rendering, scroll position being left alone — is never
exercised end to end.

**Why it matters**: the spec's own "Critical test scenarios" section calls this out explicitly
("Scroll: a ticket arrives while scrolled down. The scroll does not move and the new work marker
appears, verifies AC-16" / "...after an interaction the chime plays..., verifies AC-14, AC-15") —
this is not a gap the review is inventing, it's one the build plan asked to be closed and wasn't. The
underlying primitive (`useFreshIds`) is well tested and the mechanism is sound by inspection, but a
future change to the 120-second `SCROLLED_AWAY_PX` threshold, the `alerted` ref guard, or the
ordering between "seen" and "heard" (the module doc's own stated invariant) would ship with nothing
in CI to catch a regression.

**Suggested fix**: A `use-new-tickets.test.ts` mirroring `use-fresh-ids.test.ts`'s style (render the
hook, rerender with a changed id list, assert `playKitchenChime`/`announce` were called and
`aboveTheFold` reflects a mocked `scrollY`) would close most of this cheaply. A `kitchen-home.test.
tsx` case that resolves `api.GET` twice with different ticket sets covers the integration wiring on
top.

## Minor

### 🟡 AC-11's chef-void-reason restriction is untested past the repository layer, and `verify.md` overstates it, `api/src/presentation/handlers/service.rs:1093-1101`

**Problem**: the `actor.role() == StaffRole::Chef && code != VoidReason::KitchenUnavailable` check
that refuses a chef's void with anything but `kitchen_unavailable` (`fields.reasonCode =
not_allowed_for_chef`) lives entirely in the presentation handler. No test in `api/tests/` drives an
Axum handler directly anywhere in this codebase (confirmed: no `tower::ServiceExt`/`reqwest` usage
exists in `api/tests/`), so this is consistent with the project's established convention rather than
a regression specific to this PR — the prior review of spec 0011 flagged the identical shape of gap
(a handler-only validation, untested past the pure/domain functions) as a Minor for the same reason.
What is specific to this PR: the test at `api/tests/kitchen_display.rs:579-609` is titled `covers:
AC-11` and its own doc comment says outright "the other three are refused above this layer... which
the handler's own doc comment and the document record" — i.e. it documents that it does *not* test
the refusal. `docs/specs/0012-kitchen-display/verify.md`'s "Acceptance-criteria coverage" line still
credits "AC-11 a refused reason... (API suite)" for this, which overstates what the suite actually
proves.

**Why it matters**: low risk on its own (the check is simple and reads the session-resolved role, not
anything client-supplied), but the verify document now asserts coverage that doesn't exist, which is
the kind of drift that compounds — the next person trusting `verify.md` won't re-derive that this
path is manual-only.

**Suggested fix**: either add the (admittedly non-trivial, given no HTTP test harness exists yet)
coverage, or soften the `verify.md` line to say "checked manually" the way the chime line on the row
below it already does.

### 🟡 `mark_round_ready` is N sequential single-line writes and N audit rows for what the UI presents as one action, `api/src/infrastructure/db/repository/service.rs:1347-1387`

**Problem**: "All done" loops over every still-queued line and calls `mark_line_ready` once per
dish, each of which does its own `UPDATE`, its own round-status recompute, and its own audit
insert. It is correctly atomic (one transaction, a conflict anywhere aborts the whole request) and
correct at any ticket size this product will ever see, so this is not a bug. But `audit_log` ends up
with one `line_ready` row per dish rather than one row that says "all done, four dishes" the way
`unmark_line_ready`'s own audit entry names the round it affected — a chef reading the log later
sees four taps that happened to land in the same transaction, not one "All done" action, which is a
slightly worse trail than the one this feature builds for undo.

**Why it matters**: purely a maintainability/audit-legibility observation at current scale (a kitchen
ticket has a handful of dishes); worth a thought, not a blocker.

**Suggested fix**: optional — nothing required at this ticket size. If the pattern is reused for a
future bulk action on a bigger collection, revisit with a set-based update.

## Nits

- ⚪ `api/src/main.rs:59-131` wires `EventRegistry::close_all()` into `shutdown_signal`, fixing a real
  (and previously true) bug — graceful shutdown would otherwise hang waiting for open event streams
  to end themselves. Good fix, but it's scope outside spec 0012 (not mentioned in `index.md` or
  `verify.md`) bundled into this feature branch; a one-line mention in `verify.md`'s build-hygiene
  section would save the next reader from wondering why `main.rs` is in this diff at all.
- ⚪ `web/src/admin/routes/restaurant-settings.tsx:132-138`, `secondsToSend` treats a non-numeric typed
  value (`Number(typed)` is `NaN`) as "unchanged" and silently drops it rather than surfacing a
  validation error. `type="number"` inputs make this hard to reach in practice, so low priority.

## Strengths

- `merge_kitchen_thresholds` (`api/src/domain/catalog.rs`) is exhaustively unit tested for exactly
  the case it exists to catch — raising only the warning threshold across a stored late value it
  never mentions — plus both bounds, both directions of a crossed pair, and the boundary values
  themselves.
- The two race tests (`two_chefs_tapping_one_dish_leave_one_winner_and_a_named_refusal`,
  `an_undo_and_a_serve_of_one_dish_leave_one_answer`) correctly reuse the existing `race!` macro and
  `until_blocked` harness, and assert the *state after*, not just the error code, which is what
  actually proves no half-applied write survived the race.
- The `restaurants.version` optimistic-concurrency retrofit is done properly end to end: the `SELECT
  ... FOR UPDATE` happens before the conditional `UPDATE ... WHERE version = $9`, an empty patch is
  deliberately exempted from the staleness check with the reasoning written down, and the same
  conditional update now covers the five settings fields that predate this feature, closing a real
  last-write-wins gap spec 0003 left open.
- Translation coverage is exact: `common`, `admin`, `waiter`, and `kitchen` namespaces have identical
  key sets between `en` and `hi` (verified programmatically), and the `void.reason.*` → `common:
  voidReason.*` migration correctly removed the old keys rather than leaving them dead.
- The two-statement kitchen read (`cooking` capped and ordered by `sent_at`, `plated` capped and
  ordered by `ready_at`, each against its own partial index) is documented with the reasoning for why
  it isn't one statement, and the `truncated_count` math is provably non-negative from the shared
  snapshot rather than merely assumed to be.

## Test coverage

Backend: `api/tests/kitchen_display.rs` (18 tests) covers the two-area ordering and cap, the
thresholds carried on the response, undo with its audit row and round recompute, All-done atomicity
(including the refusal when nothing is queued), the chef void and its bill effect, the served/voided
double-void refusal, both threshold-merge validation paths (Rust and the database backstop), the
stale-version refusal (including that it doesn't regress the five pre-existing settings fields), and
both race pairs. `openapi.rs` pins that every new/widened route documents the right role. Existing
suites (`floor.rs`, `order_thread.rs`, `waiter_service.rs`) were mechanically updated to the new
`KitchenQueue` return shape. Web: `kitchen-home.test.tsx` (24 tests) is thorough for static-state
rendering, pending/busy states, the not-live and truncated banners, the sound-off prompt and its
one-tap dismissal, and axe at kitchen density in both appearances — but, as detailed above, never
simulates a ticket-set transition, so the chime, the `isNew` badge from a real refetch, and the
scroll marker are unexercised. `restaurant-settings.test.tsx` gained the new required `version`
field to its fixtures and one assertion that an address-only save omits the threshold fields, but has
no test for the `restaurant_changed` conflict path or for actually submitting a changed threshold.
`urgency.test.ts`, `use-fresh-ids.test.ts`, and `field-errors.test.ts` are all solid, focused unit
tests for the pieces they cover.
