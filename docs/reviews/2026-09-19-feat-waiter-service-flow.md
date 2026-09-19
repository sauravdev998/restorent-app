# Review, feat/waiter-service-flow, 2026-09-19

**Reviewed by**: Claude Sonnet 5 (author on Claude Opus 5)
**Scope**: 89 files, branch vs main (merge base 6d216d4), plus the uncommitted working tree changes on top
**Verdict**: Approve with nits

## Summary

This lands spec 0011 end to end: one migration, five new writes, three changed ones, and the full
waiter Orders screen, ready alert, basket-with-notes, void, take over, and move flows on the web
side. The work is careful. Lock ordering is documented once and followed everywhere (visit, then
line or round, then bill, then the counter), every conflict has a race test using `until_blocked`,
`client_key` idempotency is solid and tested for both the sequential and concurrent replay cases,
money stays `rust_decimal`/string end to end, DTOs never leak a domain type, and Hindi and English
stay in lockstep. The uncommitted changes fix a real bug found by `/check verify` itself (staff
renames never sent a `NOTIFY`, so Floor, Orders, and the table screen would show a stale name) and
add a dedicated regression test for it. No blockers and no majors. Two minors are worth fixing:
the spec text still promises `422` for a too-long note or bad void reason where the shipped code
(deliberately, and disclosed in `verify.md`) answers `400`, and that specific field-validation path
has no test beyond the pure functions. One nit on a React key in the basket panel.

## Minor

### 🟡 Spec still promises `422`, the API returns `400`, `docs/specs/0011-waiter-service-flow/index.md`

**Problem**: AC-6, AC-13, and the interface table (the rows for `POST /api/visits/{id}/rounds` and
`POST /api/order-lines/{id}/void`) all say a too-long note or a bad void reason is refused with
`422`. The shipped handlers return `400` with a `fields` body instead
(`api/src/presentation/handlers/service.rs`, mapped in `api/src/presentation/error.rs:83-91` via the
pre-existing `DomainError::InvalidFields` pattern every other field error in this API already uses).

**Why it matters**: `docs/specs/0011-waiter-service-flow/verify.md:7-9` already discloses this as a
deliberate choice made during `/develop` and says "the spec still owes that one word fix," so the
team knows. But the spec file itself was never corrected, so anyone who reads `index.md` alone (the
build spec, not the verify log) still gets the wrong contract, and a future client built against the
written spec would check for the wrong status code.

**Suggested fix**: Edit the five `422` mentions in `index.md` to `400`, closing the loop `verify.md`
already opened.

### 🟡 The too-long-note / missing-void-reason path is untested past the pure functions, `api/src/domain/service.rs`, `api/src/presentation/handlers/service.rs`

**Problem**: `normalize_note` and `void_reason_text` (`api/src/domain/service.rs`) are well unit
tested, including the 140/141 character Hindi boundary. But nothing drives a request through
`send_round` or `void_line` in `api/src/presentation/handlers/service.rs` far enough to see the
actual `400` + `fields` response those functions produce when wired to the handler. `verify.md`
line 20 records this as checked manually in the browser, not by an automated test.

**Why it matters**: this is consistent with the rest of the suite (no file under `api/tests/`
exercises the Axum router directly; everything calls the repository layer straight against a
`ScopedTx`, matching the project's stated test strategy), so it is not a regression in how this PR
tests things. But it does mean the `422`/`400` mismatch above, and any future drift in the field
name or error code for these two paths, can only be caught by a human running `/check verify`, not
by `pnpm check` or `cargo test`.

**Suggested fix**: Optional given the project's convention, but worth a thought: a small test that
builds a `SendRoundRequest`/`VoidLineRequest` with an over-long note or reason and asserts the
`ApiError` it produces, without needing a live server, would close the gap cheaply.

## Nits

- ⚪ `web/src/waiter/components/basket-panel.tsx:64`, each basket line is keyed
  `${line.dishId}-${String(index)}`. Removing or splitting a line above another shifts every later
  index, so React can remount a note `<Input>` that was not touched (a visible focus jump while
  typing) instead of preserving it. A key derived from something assigned once per line, rather than
  its position, would avoid it.
- ⚪ `api/src/infrastructure/db/repository/billing.rs`, `end_visit` discards the `VisitStatus` that
  `service::lock_visit` returns. Functionally fine (the bill lookup right after is what actually
  decides the answer), but a one-line comment saying why the status is not checked here would save
  the next reader from re-deriving it.

## Strengths

- The lock order (visit, then line or round, then bill, then the bill number counter) is written
  once in `api/src/presentation/handlers/service.rs`'s module doc and in `api/AGENTS.md`, and every
  path that takes more than one lock visibly follows it; `a_send_and_a_close_on_one_table_never_deadlock`
  and the rest of `api/tests/waiter_service.rs`'s race suite prove it under `until_blocked` rather
  than by inspection alone.
- The `client_key` replay design is genuinely careful: checked before the visit-open check (so a
  reply after the table closed still returns the original ticket), guarded by both a row lock and a
  named unique index mapped through `conflict_on`, and covered by sequential, cross-visit, and
  concurrent test cases.
- The uncommitted fix to `staff::rename` and `accounts::set_display_name` (missing `NOTIFY`) was
  caught by the developer's own `/check verify` run and lands with a real regression test
  (`a_rename_by_the_admin_or_by_the_waiter_tells_every_screen`, plus its negative counterpart) rather
  than a silent patch — a good sign the verify pass was substantive.
- Hindi and English `waiter.json`/`common.json` are in exact key parity (checked programmatically),
  and the two device Playwright scenario in `web/e2e/order-thread.spec.ts` was extended to match
  AC-20 almost line for line, including the "table label must not match table 10" regex care that
  was already there.

## Test coverage

Backend: `api/tests/waiter_service.rs` (21 tests) covers idempotent send/replay, notes at the
character boundary, per-dish and serve-all serving, void with subtotal recompute and audit, take
over, move, the Orders read, the empty-bill voiding, cross-restaurant isolation, and every AC-17
race pair (mark-ready-vs-void, serve-vs-void, two serves, two voids, two take-overs, two moves, and
a void racing an empty close). Existing suites (`billing.rs`, `concurrency.rs`, `floor.rs`,
`isolation.rs`, `menu.rs`, `order_thread.rs`, `service_flow.rs`) were mechanically updated to the new
`send_round`/`void_line` signatures via a `common::send_round` test helper. Web: `basket.test.ts`,
`alerts/ready.test.ts`, `alerts/use-ready-alerts.test.tsx`, `routes/orders.test.tsx`,
`routes/table.test.tsx`, and `routes/floor.test.tsx` cover the sort order, the alert's grouping and
silence rules, the Mine filter, and storage round trips, including failure paths (storage refused,
garbage JSON). The one real gap is the field-validation status/body for a too-long note or missing
void reason discussed above, which is unit tested at the pure-function level only.
