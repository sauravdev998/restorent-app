# Review, feat/core-data-model (feature 0004: design system and accessibility), 2026-09-02

**Reviewed by**: Claude Sonnet 5 (author on unspecified model)
**Scope**: 57 files, branch vs `main` (scoped to commits after `9ae4468`, i.e. feature 0004 only — feature 0003 already reviewed separately), plus uncommitted working tree changes and two untracked test files
**Verdict**: Approve with nits

## Summary

This lands the four-layer CSS token system, the base component library in `web/src/shared/ui/`,
and the three automated accessibility gates (contrast script, `jsx-a11y` strict + a physical-direction
lint rule, and an axe helper wired into nine test files) that spec 0004 calls for. I ran the actual
gates rather than trusting the prose: `pnpm --filter web test` (62/62 passing), `tsc -b --noEmit`
(clean), `eslint .` (clean), and `node scripts/check-contrast.ts` (144 pairs across 3 appearances,
all passing) all succeed against the working tree as it stands, including the uncommitted fixes. The
work is unusually careful — the skip link, the disabled-state forced-colors handling, `DataTable`'s
focusable region, and the `Toast` landmark wrapping were all fixed in the uncommitted changes and are
now backed by real tests, not just comments. The one place the accessibility promise has a genuine
gap is `Toast`'s auto-dismiss, which has no way to pause or extend its 6-second timer — a real WCAG
2.2.1 concern that none of the three gates catch. Everything else is minor or nit level.

## Major

### 🟠 Toast auto-dismiss has no pause, extend, or turn-off mechanism, `web/src/shared/ui/toast.tsx:29-37`

**Problem**: `Toast` starts a 6-second (`DEFAULT_DURATION_MS`) timer on mount and calls
`dismissToast` unconditionally when it fires. There is no way to pause it on hover or focus, extend
it, or turn it off short of manually clicking Dismiss before it expires. `ToastRequest.durationMs` is
configurable per-call but that is a caller choice, not a mechanism the person reading the toast
controls.

**Why it matters**: WCAG 2.2 SC 2.2.1 (Timing Adjustable, Level A) requires that any content with a
time limit let the user turn it off, adjust it, or extend it, unless the situation meets one of a
short list of exceptions (real-time event, essential, or a limit over 20 hours). None of those
exceptions apply here. A waiter who is mid-interruption — another table calling, a dropped plate —
loses the toast's content before they can read it, with no recourse. This is exactly the kind of gap
the review brief flags as serious: none of the three automated gates (axe, the contrast script, or
the lint rules) can catch a timing issue, so this component can carry a real Level A violation while
every gate stays green. Spec 0004's own promise is "the accessibility level stops being a document
and becomes three build failures" — this is a case where it silently isn't, because timing sits
outside all three gates by construction.

**Suggested fix**: Pause the dismiss timer on pointer hover and on keyboard focus within the toast,
and restart it on blur/pointer leave (the common accepted pattern, matching what `role="status"` live
regions imply about not yanking content away from someone still reading it). At minimum, note the gap
in `docs/design.md`'s "what the gates do not cover" section so it is a tracked decision rather than an
unnoticed one.

## Minor

### 🟡 `toast-store`'s module-level state is never reset between tests, `web/src/shared/ui/toast-store.ts:19-20` (exercised in `web/src/shared/ui/feedback.test.tsx:58-111`)

**Problem**: `toasts` and `listeners` are plain module-scoped `let`/`const` bindings, not reset by any
`beforeEach`/`afterEach`. In `feedback.test.tsx`, the `'is a named landmark...'` test calls
`showToast` twice and never dismisses either, leaving two toasts in the module singleton after the
test finishes. The suite currently passes only because no later test in the same file (same module
instance) touches the toast store again — it is order-dependent by accident, not by design.

**Why it matters**: This is real but not urgent latent fragility: the next person who adds a toast
test after that one, or reorders the `describe` blocks, gets a confusing failure with no indication
the cause is leftover state from an unrelated test rather than their own code.

**Suggested fix**: A `resetToasts()` test-only export (or a `beforeEach` that drains the store) called
from `feedback.test.tsx`, so each test starts from an empty store regardless of execution order.

## Nits

- ⚪ `web/src/shared/ui/toast-store.ts:47,52`, `dismissToast` and `subscribeToToasts` have no doc
  comment, unlike `showToast` right above them and every other exported function in `shared/ui/`.
- ⚪ `web/src/shared/ui/icon.tsx`, `web/src/shared/ui/connection-status.tsx`, no standalone
  `expectAccessible` test for `Icon` or `ConnectionStatus` — both are exercised only indirectly
  (inside `Button`/`StatusPill`'s and `SurfaceShell`'s axe runs respectively). That satisfies AC-4 in
  substance, but a reader looking for "where is `Icon` proven accessible" has to know to look inside
  another component's test file.

## Strengths

- The contrast gate (`web/scripts/check-contrast.ts` + `contrast-pairs.ts`) genuinely holds no
  duplicated colour: it parses `index.css` with `postcss`, walks `:root`, the light media query, both
  `[data-theme]` blocks, and `@media print`, and resolves `var()` chains recursively. I ran it standalone
  and it reports 144 pairs across 3 appearances, all passing, which matches `docs/design.md`'s claim
  exactly. The `SURFACES` list (`background`, `card`, `muted`, `secondary`) looks narrow at first
  glance, but `--popover` and `--accent` are defined to always equal `--card`/`--background` and
  `--secondary` respectively in every appearance, so the coverage is transitively complete rather than
  selectively narrow — a deliberate palette property, not an oversight I needed to flag.
- The axe helper (`web/src/test/axe.tsx`) is actually wired in, not just present: nine test files call
  `expectAccessible`, covering every base component in the spec's table (directly or via a parent
  component/page), each across both appearances and all three densities (six renders per call). Disabling
  axe's own `color-contrast` rule under jsdom and giving that responsibility entirely to the contrast
  script is the right call and is explained in both the code and `docs/design.md`.
- The uncommitted fixes are the strongest part of this change: `DataTable`'s scroll container became a
  named, focusable `role="region"` so a keyboard user can reach the far side of a table that doesn't fit
  a phone; `SurfaceShell`'s `<main>` got `tabIndex={-1}` so the skip link actually moves focus rather than
  just scrolling; `Input`/`Select` got matching forced-colors disabled-state handling to match `Button`;
  and `ToastViewport` became a named `<section>` landmark wrapping the `<ul>` rather than mislabelling the
  list itself. Every one of these is backed by a real assertion, not just a comment.
- The physical-direction ESLint rule is precise, not just present — I verified with real greps across
  `web/src` that no physical utility slipped through, and that the rule's own exemption for `border-line`
  and `rounded-lg` is correctly disambiguated from `border-l`/`rounded-l` by the trailing-boundary group in
  its regex.
- Every string introduced by this feature goes through `t()`: I diffed every `t('design.*')` call in
  `design-gallery.tsx` against `common.json` and every key resolves, including the dynamically built ones
  (`design.icon.${size}`, `design.${appearance}`).
- `docs/design.md` matches the shipped code point for point where I checked it against `index.css`
  (density table, colour roles, the `--border`/`--input` values called out as deliberately lighter than a
  designer's instinct).

## Test coverage

Test signal is `configured` (Vitest). Coverage is strong and specific rather than superficial: tests
assert on `aria-describedby` resolving to real element text, on focus actually landing on `<main>` and
returning to the dialog's opener, on the tab order via `user.tab()`, on `aria-sort` and row-header
semantics, and on the density attribute clearing when navigating back out of the kitchen surface — not
just on classes being present. The two real gaps are the ones named above: no test would catch a
regression to `Toast`'s timing (there is no mechanism to test), and the toast-store's cross-test state
is untested rather than reset. `verify.md` is honest about what still needs a human with a screen
reader (dialog announcement, `DataTable` row-header reading) rather than claiming those are covered.
