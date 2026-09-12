# Review, feat/menu-management, 2026-09-12

**Reviewed by**: Claude Sonnet 5 (author on Claude Sonnet 5)
**Scope**: 136 files changed, branch vs `main` (merge base `c774967`)
**Verdict**: Approve

## Summary

This slice gives the admin a real, live-edited menu over spec 0003's schema: categories and dishes
with create/edit/move/reorder/remove/restore, a required veg/non-veg/egg mark, a version-stamped
optimistic-concurrency story, and a chef-facing availability switch that closes the "we ran out" gap
to a couple of seconds end to end. The engineering is unusually thorough and matches the spec's
`Feature design` section almost line for line: the two lock-ordering invariants (`FOR UPDATE` on an
archiving category vs `FOR SHARE` on a category taking a dish, and `ORDER BY id` on every reorder's
sibling lock) are implemented exactly as designed and proved with real concurrent-transaction tests
(`a_dish_created_while_its_category_is_removed_never_leaves_a_live_dish_in_it`), `send_round` now
checks every line before writing anything so a dish that went off refuses the whole ticket with no
partial round, and the web side keeps spec 0007's "no cache optimism" rule intact throughout (held
drag order, held availability value, refetch-after-confirm). Domain/application/infrastructure/
presentation layering is clean — no `axum` or `sqlx` type leaks inward — and every menu endpoint
carries its role in its own `Actor<R>` type, backed by a test that walks the OpenAPI document.
`cargo check`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`, `tsc -b`, `eslint .`,
and `pnpm --filter web test` (331/331) were all run against this branch during review and passed
clean. I found nothing worth blocking on; two small inefficiencies are noted below for the author's
discretion.

## Minor

### 🟡 An edit that keeps its category still pays for a `next_dish_position` query, `api/src/infrastructure/db/repository/catalog.rs:1374-1375`
**Problem**: `update_dish` unconditionally calls `next_dish_position(tx, edit.category_id)` to get
`end_of_target`, even when the dish is not moving (the subsequent `UPDATE`'s `CASE` only uses that
value when `d.category_id <> $2`). Every dish rename, reprice, or diet change — the common case —
pays for a query whose result it then throws away.
**Why it matters**: Not a correctness issue (the `CASE` guards it), just an avoidable round trip on
the hottest write path in the feature. At current scale (a few hundred dishes per restaurant) this is
invisible, but it is a pattern worth not repeating as the menu grows other conditional moves.
**Suggested fix**: Only compute `end_of_target` when `edit.category_id` differs from the dish's
current category (known from the same `old` subquery the statement already reads), or accept it as
one cheap query traded for the statement's simplicity — either is fine, just worth a conscious call
rather than an unconditional one.

## Nits

- ⚪ `api/src/infrastructure/db/repository/catalog.rs:1086-1088` and `:1586-1588`, both reorder
  functions notify with only `ids.first()` rather than the full set — harmless today since the web
  fan-out map invalidates by entity kind and never reads the id, but worth a one-line comment saying
  so explicitly (the file's own header comment explains the *locking* rationale well but not this
  choice).
- ⚪ `docs/scope/scope.md:171`, feature 9's checklist has `[ ] Review it` still unchecked — expected,
  since this review is what checks it; no action needed, just noting the file will need one more
  edit after this lands.

## Strengths

- The concurrent-transaction tests in `api/tests/menu.rs` (`a_dish_created_while_its_category_is_removed_never_leaves_a_live_dish_in_it`,
  `a_ticket_carrying_a_dish_that_went_off_is_refused_whole`) actually spawn two transactions and
  assert on lock ordering and the database's final state, not just on the two call sites in isolation
  — this is the hardest part of the spec's invariants and it is genuinely proved, not just asserted.
- `send_round` was restructured to validate every line before any row is written (`api/src/infrastructure/db/repository/service.rs:570-601`),
  so `dish_not_orderable` now refuses the whole ticket cleanly instead of relying on the caller to
  roll back a partially-built round.
- The web side's "held value, dropped only after the confirmed refetch lands" pattern
  (`use-dish-availability.ts`, `reorderable.tsx`) is applied consistently and correctly threads the
  `onSuccess`/`onSettled` awaiting so a switch or a drag never flickers back to a stale value between
  the write landing and the refetch completing.
- Locale parity (en/hi) holds across all four namespaces post-change (179/132/18/41 keys, no
  mismatches), and the diet-mark and contrast-pair additions in `docs/design.md` and
  `web/scripts/contrast-pairs.ts` are exactly consistent with the CSS tokens actually shipped.

## Test coverage

Strong and scenario-driven, matching the spec's own "Critical test scenarios" list closely:
`api/tests/menu.rs` covers the full lifecycle, both stale-edit races (dish and category), the
category-not-empty and category-archived-race in both interleavings, stale reorder, live-name
uniqueness, restore-name-clash, the basket/ticket race (`dish_not_orderable` refuses the whole
ticket), sent-lines-are-untouched across every kind of menu edit, and cross-restaurant 404s. The
Rust unit layer (`domain::menu`, `domain::enums`, `presentation::openapi` tests) covers every field
validation case from AC-14 and the full role matrix. On the web side, `admin-menu.test.tsx`,
`kitchen-menu.test.tsx`, `table.test.tsx`, `basket.test.ts`, `switch.test.tsx`, `diet-mark.test.tsx`,
`format.test.tsx`, and `query-keys.test.ts` all assert real behavior (stale-form recovery, field
error placement, basket flagging, fan-out routing) rather than shallow rendering. The Playwright
addition (`menu.spec.ts`) drives three simultaneous browser contexts (admin, chef, waiter) and
asserts no-reload propagation for both a new dish and a switched-off one, plus a dedicated keyboard
reorder scenario with English/Hindi announcement assertions. I did not find any new branching logic,
error path, or security-relevant code left uncovered.
