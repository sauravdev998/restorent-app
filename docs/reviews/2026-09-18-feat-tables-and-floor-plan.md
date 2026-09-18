# Review, feat/tables-and-floor-plan, 2026-09-18

**Reviewed by**: Claude Sonnet 5 (author on Claude Sonnet 5)
**Scope**: 79 tracked files + 9 untracked (docs/specs, web/e2e/floor.spec.ts, web/src/admin/api/floor.ts, web/src/admin/floor/*, web/src/admin/routes/admin-floor.tsx + test), branch vs `main` (merge base `67b8a92`)
**Verdict**: Approve

## Summary

This is a faithful, thorough implementation of spec 0010. The migration correctly reuses spec 0003's
RLS-protected tables (no new tables, so no new RLS/grants needed) and adds versioning, length/seat
checks, and case-insensitive live-uniqueness exactly as the data model sketch specifies. The
occupancy race (`FOR UPDATE` on archive vs `FOR SHARE` in `require_live_table`) and the
section-emptiness race (same pattern) are implemented and covered by dedicated concurrency tests
using real concurrent transactions plus the project's `until_blocked` helper rather than sleeps. The
range-add and section-restore "check first, write once, roll back and re-check on a late clash"
protocol matches the spec's rationale precisely, including the ordering nuance where a name-taken
check happens inside the same statement as a labels-taken pre-check. Every endpoint is admin-gated
through `Actor<Admin>`, documented in `openapi.rs`, and asserted present with `401`/`403` in a test.
The web side mirrors the menu screen's established patterns (dialogs, stale-version handling, no
cache optimism, drag-and-drop lifted into a shared `admin/shared/reorderable.tsx`), and i18n keys
are at full parity between `en` and `hi` across `admin.json`, `common.json`, and `waiter.json`. Test
coverage on both sides is extensive and traceable to acceptance criteria via `covers:` comments.

## Strengths

- The two load-bearing lock orderings (table-occupancy and section-emptiness) are implemented exactly
  as designed and exercised by real two-transaction concurrency tests in both orders
  (`api/tests/floor.rs:404`, `api/tests/floor.rs:622`), not just asserted at the unit level.
- The "roll back, read fresh" late-clash path (`api/src/presentation/handlers/admin_floor.rs:567-580`,
  `:773-784`) matches the documented rationale and is covered
  (`api/tests/floor.rs:772`, `a_label_taken_between_the_check_and_the_insert_is_found_afterwards`).
- `web/src/admin/shared/reorderable.tsx` generalizes the menu's drag-and-drop component cleanly to a
  second `ReorderKind`, keeping translated screen-reader announcements and the "held order, never
  cache-written" pattern intact for both dishes/categories and sections/tables.
- i18n parity (`en`/`hi`) verified programmatically across all three touched namespaces: no keys
  missing on either side.
- The old `catalog.rs` floor functions were cleanly retired in favour of `floor.rs`, with every caller
  (seed, service_flow tests, concurrency tests) updated in the same change.

## Test coverage

Extensive on both sides. `api/tests/floor.rs` covers creation, editing, moving, reordering (both
levels), archive/restore for both tables and sections, the range add and its clash list, stale-version
conflicts, cross-tenant 404s, and the two concurrency races. `web/src/admin/routes/admin-floor.test.tsx`
covers the dialogs, stale-edit recovery, clash lists, drag announcements in both languages, and an axe
pass. `web/e2e/floor.spec.ts` drives the two-browser live scenario per AC-20. One gap, consistent with
the rest of the codebase rather than specific to this change: there is no HTTP-level test (through the
real router/extractor stack) asserting `403`/`401`/`404` for the floor endpoints; role gating is
proven only by the `Actor<Admin>` type in the handler signature plus the OpenAPI-document test. This
mirrors how `staff.rs` and `menu.rs` already test role gating in this codebase, so it is not a
regression introduced here, just worth naming since AC-16 explicitly calls out that scenario as a
critical test.

## Nits

- ⚪ `api/src/bin/seed.rs:334`, the `ensure_floor` signature line is long; rustfmt will already have
  reflowed it if CI is green, so this is cosmetic only.
- ⚪ `docs/specs/0010-tables-and-floor-plan/index.md` follow-up items are still open (drawn floor map,
  bill label snapshot) — expected and already tracked in `docs/scope/scope.md`, not an omission of
  this change.
