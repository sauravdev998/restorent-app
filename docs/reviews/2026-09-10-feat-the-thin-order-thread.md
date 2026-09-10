# Review, feat/the-thin-order-thread, 2026-09-10

**Reviewed by**: Claude Sonnet 5 (author on Claude Sonnet 5)
**Scope**: 76 files (72 tracked + 4 untracked), branch vs `main` (merge base `7488bf0`)
**Verdict**: Changes requested

## Summary

This slice threads the whole order flow — floor, menu, send, kitchen queue, ready, serve, close —
through all four API layers and two new web screens, on top of spec 0003's existing schema and
repositories, and proves the live path with a real two-context Playwright run. The engineering is
careful throughout: tenant scoping is structural and well tested (`none_of_the_new_reads_can_see_another_restaurants_service`),
`REPEATABLE READ` snapshots fix a real document-consistency bug with a regression test to prove it,
money is decimal-string end to end, conflicts are a closed `ConflictKind` enum with full en/hi
translation coverage, and the fan-out invalidation map matches the spec exactly. Two real gaps keep
this from a clean approve: the waiter's ready-alert can permanently miss a second round while a
first one is still un-served, and the spec's own "two waiters serving the same round" scenario —
required by the spec's own **Critical test scenarios** — has no test anywhere in the change.

## Blockers

None.

## Major

### 🟠 A second round going ready can never alert the waiter while an earlier one is still unserved, `web/src/waiter/routes/table.tsx:73-81`
**Problem**: `readyRound` is computed with `visit.data?.rounds.find((round) => round.status === 'ready')`,
which returns only the *first* round in the array that is currently `ready`. The `announced` ref and
the `useEffect` below it only ever look at this single value. If round 1 goes ready and the waiter
dismisses the banner (or is just slow to serve it) without marking it served, round 1 stays in the
`rounds` array with `status: 'ready'`. When round 2 also goes ready, `find` still returns round 1
first (already in `announced`), so the effect's guard (`if (announced.current.has(readyRound.id)) return`)
exits immediately and round 2 is **never** announced — no banner, no chime, no live-region
announcement — for as long as round 1 remains unserved. This is a plausible restaurant scenario
(starters sent, then mains sent before starters are cleared), not an edge case.
**Why it matters**: AC-8 promises "every waiter screen raises a visible alert... Each round alerts
once per screen, never repeatedly." The implementation satisfies "never repeatedly" but silently
violates "every round" whenever two rounds are ready at once. This directly undercuts the feature's
headline promise ("told the moment food is ready without watching a screen") for a normal multi-round
table, and nothing in `table.test.tsx` or the Playwright scenario exercises two simultaneous rounds,
so it shipped unnoticed.
**Suggested fix**: Track every currently-ready, not-yet-announced round (e.g. `rounds.filter(r => r.status === 'ready' && !announced.current.has(r.id))`) and either queue/stack alerts or announce each newly-ready round as it appears, rather than gating on a single `find`. Add a test with two rounds ready at once, one already dismissed.

### 🟠 The spec's own "same round served twice" scenario is untested anywhere, `api/src/presentation/handlers/service.rs:578-627`
**Problem**: `mark_round_served` reads the round's status with a plain, unlocked `service::round` read,
then serves every `ready` line one at a time via `serve_every_ready_line`, remapping any line-level
conflict to `ConflictKind::RoundNotReady`. This is exactly the logic spec 0007 flags as needing proof:
"Failure case, the same round served twice: two concurrent serves of one round leave every line served
exactly once, and the loser receives `409 round_not_ready` rather than a line level message. Verifies
AC-9, AC-13." Searching the whole diff (`grep -rn "round_not_ready\|mark_round_served" api/tests`)
turns up nothing — no repository/handler-level test, and the added `verify.md` UI checklist covers the
"two chefs mark the same dish" race but never the "two waiters serve the same round" race. `concurrency.rs`
gained `two_dishes_on_one_ticket_marked_at_once_still_leave_it_ready` but nothing analogous for serving.
**Why it matters**: This is branching, error-handling logic that decides what a losing waiter is told
(AC-13's "the loser receives 409 with the code naming what happened"), reached only under concurrency,
and it is the one piece of this slice's own required scenario list that shipped with zero coverage.
Manual tracing suggests the deterministic line ordering (`lines_for_round` is `ORDER BY created_at, id`
for both racers) makes split-serving actually impossible today, but that guarantee is incidental to
`serve_every_ready_line`'s current shape and nothing pins it down — a later refactor (e.g. parallelising
the per-line loop, an early-return reorder) could silently reintroduce it with no test to catch it.
**Suggested fix**: Add the scenario the spec names: two concurrent `POST /api/rounds/{id}/served` (or
the repository calls a handler test drives directly) on the same ready round, asserting exactly one
`200` and one `409 round_not_ready`, and that every line ends up `served` exactly once.

## Minor

### 🟡 Kitchen screen imports a waiter-owned type across the feature boundary, `web/src/kitchen/routes/kitchen-home.tsx:17`
**Problem**: `import { ApiCallError } from '@/waiter/api/orders'`. `ApiCallError` is a generic
"what a failed request threw" wrapper with nothing waiter-specific about it, but it is defined inside
`waiter/api/orders.ts` and the kitchen surface reaches across into the waiter folder to use it.
**Why it matters**: Root `AGENTS.md` and `web/AGENTS.md` are explicit that `web/` groups by feature and
"the three surfaces barely share a screen" — `shared/` is the intended seam between them. This import
makes the kitchen surface depend on the waiter surface's internals, which is exactly what the
feature-folder convention exists to prevent (a change to `waiter/api/orders.ts` can now break the
kitchen screen for reasons that have nothing to do with waiters).
**Suggested fix**: Move `ApiCallError` (and the pattern of throwing it from a failed `api.GET`/`api.POST`)
into `shared/api/`, and have both `waiter/api/orders.ts` and `kitchen/api/tickets.ts` import it from
there.

### 🟡 `test-preferences.json` still records no end-to-end tool, `test-preferences.json:5`
**Problem**: The repo-level `test-preferences.json` (read by `/test`) still says `"e2eTool": "none"`,
but this change adds Playwright as a first-class, CI-wired e2e tool (`web/playwright.config.ts`,
`web/e2e/order-thread.spec.ts`, the `pnpm e2e` script, the CI job). The file was not touched by this
change.
**Why it matters**: Root `AGENTS.md` correctly anticipated this ("Playwright joins continuous
integration once slice 1 exists"), so adding Playwright here is right per the project's own plan — but
leaving the preferences record saying "none" means the next `/test` invocation (or anyone reading that
file as the source of truth for tooling) is working from stale information.
**Suggested fix**: Update `"e2eTool"` to `"playwright"` (and note `web/e2e/*.spec.ts` as the pattern) in
the same change that introduces it.

## Nits

- ⚪ `api/src/infrastructure/db/repository/catalog.rs:280-364`, `create_table_section` and
  `create_menu_category` do not call `Database::notify_entity_change`, while the otherwise-parallel
  `create_dining_table` and `create_dish` (added in the same diff, for the same seed use case) do.
  Currently harmless (only the seed script calls these, and `table_section`/`menu_category` aren't
  entity kinds in the fan-out map), but the inconsistency will look like a missed notify once feature 9/11
  puts an admin screen over these.
- ⚪ `web/src/shared/api/error-message.ts:46-73`, the `MESSAGE_KEYS` map (and its mirrored `CODES` list
  in `error-message.test.ts`) already carries keys for `visit_not_closed`, `line_not_served`, and
  `line_not_voided` — codes `ConflictKind::as_code()` can produce but that spec 0007's own table says
  this thread never raises. This is more complete than the spec asked for, which is a genuine strength,
  but worth a one-line note in the spec or a comment here so a future reader doesn't wonder why the
  counts don't match the "fifteen codes" the spec's Feature design table describes.

## Strengths

- Tenant isolation for every new read/write is proven at the repository layer, not just asserted:
  `none_of_the_new_reads_can_see_another_restaurants_service` deliberately rescopes a raw connection to
  write into a second restaurant, then confirms the first restaurant's `floor`, `kitchen_queue`,
  `visit`, `rounds_for_visit`, and `open_bill_of` all come back empty/`NotFound` rather than leaking rows.
- `begin_scoped_snapshot` (`REPEATABLE READ`) is a genuinely good catch of a real bug class — a
  multi-statement document read under `READ COMMITTED` producing an internally inconsistent answer — and
  `a_document_read_never_shows_a_ticket_disagreeing_with_its_dishes` reproduces the exact failure mode
  deterministically (fixed-order commit between two of the reader's statements) rather than relying on
  timing.
- The `FOR UPDATE` lock added to `recompute_round_status` directly targets the two-chefs-mark-at-once
  race, and `two_dishes_on_one_ticket_marked_at_once_still_leave_it_ready` in `concurrency.rs` proves it.
- Money and timestamps are disciplined everywhere touched: every DTO figure is a decimal string with a
  dedicated serialization test (`every_money_figure_leaves_as_an_exact_decimal_string`), and
  `server_time` plus the client-side `clockOffset`/`onDeviceClock` pair is a clean, well-tested answer to
  a wrong device clock (`server-clock.test.ts`, and the kitchen test that sets the fake clock 20 minutes
  fast and asserts the displayed age stays near one minute).
- en/hi locale files are fully in sync for every new key (`common.json`'s `apiError.*`, `waiter.json`,
  `kitchen.json`), including every conflict code the API can emit.

## Test coverage

Well covered overall for `TESTS = configured`: the domain (`round_status_from_lines`, `ConflictKind`
code/no-clash/English-sentence invariants), the DTO wire-shape guarantees, the repository layer
(tenant isolation, snapshot consistency, the happy path, every named conflict), `concurrency.rs`'s races,
and the web side (fan-out map, clock offset, alert-once-per-round, both screens' accessible/error/pending
states) are all exercised with tests that assert real behaviour rather than mocks. The two gaps are
specific and named above: no coverage for two simultaneous ready rounds on the waiter screen, and no
coverage — despite being explicitly listed in spec 0007's own "Critical test scenarios" — for two
concurrent serves of the same round.
