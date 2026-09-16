# Review, feat/staff-accounts, 2026-09-15

**Reviewed by**: Claude Sonnet 5 (author on Claude Sonnet 5)
**Scope**: 82 files, branch vs main (commit f4fea40 plus uncommitted edits)
**Verdict**: Changes requested

## Summary

This feature lets an admin create, rename, re role, reset, deactivate, and reactivate staff
accounts, with a forced password change on first sign in. The backend is careful and well
reasoned: a single advisory lock plus a fixed refusal order make the two guard rails (no acting on
your own row, never remove the last admin) genuinely race free, every access changing write revokes
sessions in the same transaction, no password or hash ever reaches the audit log, and the password
owed gate is a type level marker on the `Actor` extractor so a new endpoint is refused by default
rather than by someone remembering. Repository level tests are thorough and include real
concurrency and cross tenant proofs against Postgres. Two real problems remain: a state closure bug
in the web layer that can show an admin the wrong password after creating or resetting an account,
and a complete absence of automated tests for every new web screen this feature added.

## Major

### 🟠 The hand over panel can show a password that was never written, `web/src/admin/staff/create-staff-dialog.tsx:70-77` and `web/src/admin/staff/reset-password-dialog.tsx:48-53`

**Problem**: `onSuccess` reads the `password` component state directly instead of the value that was
actually submitted:

```ts
const save = useMutation({
  mutationFn: (form: NewStaffForm) => createStaff(form),
  onSuccess: async (created) => {
    await queryClient.invalidateQueries({ queryKey: staffKey })
    setHandOver({ displayName: created.displayName, email: created.email, password })
  },
  ...
})
```

TanStack Query's `MutationObserver.setOptions` replaces an in flight mutation's callbacks
(`this.#currentMutation.setOptions(this.options)`) whenever the owning component re-renders while
the mutation is still pending (verified in
`node_modules/.pnpm/@tanstack+query-core@5.101.4/.../mutationObserver.js`). The `Input` for the
password is never disabled during `save.isPending`, so if the admin edits the password field (or any
field, since any re-render replaces the pending mutation's options) between pressing submit and the
response landing, `onSuccess` fires with the closure from the latest render, not the one active when
`mutate()` was called. The panel then displays whatever is currently in the field, which is not what
was hashed and stored. The same pattern exists in `ResetPasswordDialog`, which closes over `password`
in both `mutationFn` and `onSuccess` rather than passing it as a mutate variable.

**Why it matters**: The whole feature is built around "shown once, never recoverable": the API never
returns the password or its hash, so the form's value is the only record of it anywhere. If that
value silently drifts before the hand over panel renders, the admin reads out a password that does
not work, the new hire cannot sign in, and nothing signals that anything went wrong. The only
recovery is another password reset, which is a support incident that a network heavy or slow-typing
admin could hit for no reason connected to anything they did wrong.

**Suggested fix**: Use the `variables` argument `onSuccess(data, variables)` provides (the exact
object passed to `mutate()`) instead of the outer `password` state, e.g.
`setHandOver({ displayName: created.displayName, email: created.email, password: variables.password })`.
For `ResetPasswordDialog`, pass the password through `mutate(password)` with a `mutationFn` that
takes it as a parameter, and read it back from `variables` in `onSuccess` rather than from the
closure. Alternatively, disable both password inputs while their mutation is pending, which removes
the window entirely.

## Minor

### 🟡 A no-op role change still announces "role changed", `web/src/admin/staff/edit-staff-dialog.tsx:63-73`
Setting somebody's role to the one they already hold succeeds (per AC-8) but writes and revokes
nothing. The dialog still shows `staff.edit.roleChanged`, which tells the admin a change took effect
when none did. Not incorrect, just a small UX inconsistency worth a distinct message or a no-op
toast.

## Nits

- ⚪ `api/src/presentation/handlers/staff.rs:60-76` vs `92-97`: `CreateStaffRequest.role` is a raw
  `String` (validated by hand, for field error UX) while `ChangeRoleRequest.role` is the typed
  `RoleDto`. The asymmetry is explained in a comment and is deliberate, but it means an invalid role
  on the edit path fails as a generic `400 invalid` rather than a field error, unlike create.
- ⚪ `api/src/infrastructure/db/repository/staff.rs:619-631,641-647`: `lock_restaurant` is taken for
  every targeted write, including a plain rename that can never touch the last-admin guard rail, so
  all staff writes for one restaurant serialise unconditionally. Accepted deliberately per the
  module doc and fine at "tens of staff," but worth knowing if staff counts ever grow.

## Strengths

- The two guard rails (no self action, never remove the last admin) and the five-way fixed refusal
  order are implemented once, in `refuse_before_writing`, and proven under real concurrency in
  `api/tests/staff.rs` (`two_admins_demoting_each_other_at_once_leave_exactly_one_standing`,
  `a_deactivate_and_a_reactivate_at_once_resolve_to_exactly_one_winner`).
- The password owed gate is a second type parameter on `Actor`, defaulting to refuse, so a new
  handler is covered by omission rather than by someone remembering to add a check; a dedicated test
  (`saying_nothing_about_the_password_means_the_strict_gate`) pins the default itself.
- No password or hash ever reaches the audit log or a response body, and this is asserted directly
  in tests on both the Rust (`a_password_reset_puts_no_password_and_no_hash_in_the_log`,
  `a_staff_member_reaches_an_admin_with_no_password_and_no_hash`) and the design side.
  Tenant isolation (`an_admin_can_neither_read_nor_write_another_restaurants_staff`) runs against
  real Postgres as `app_api`, not mocked.
  The dialog focus-restoration fix (`web/src/shared/ui/dialog.tsx`) that shipped alongside this
  feature, with new tests for the "mounts already open" case these admin dialogs introduced, is a
  genuine, well-tested bug fix rather than scope creep.

## Test coverage

Backend: thorough. `api/tests/staff.rs` covers every acceptance criterion that can be checked
against a real Postgres transaction (creation, renames, role changes, resets, deactivate/reactivate,
the guard rails under real concurrency, the six-shape audit log, and cross tenant isolation), and
`actor.rs`'s and `openapi.rs`'s own unit tests pin the password gate's default and every staff
route's admin-only marker reaching the OpenAPI document. `docs/specs/0009-staff-accounts/verify.md`
records a full manual pass over the HTTP surface (403s for waiter/chef and for an owed password
change, the SSE heartbeat closing within one cycle, the fresh-database seed) that this project does
not otherwise automate in Rust, consistent with its stated test pyramid (repository integration
tests plus a runtime verify, no Axum-level HTTP test harness elsewhere in the codebase either).

Web: this is the real gap. Every file this feature added under `web/src/admin/routes/admin-staff.tsx`,
`web/src/admin/staff/*.tsx`, and `web/src/app/routes/choose-password.tsx` has zero test coverage, and
the six existing test files this diff touches only gained one mechanical line each
(`mustChangePassword: false`) to satisfy the type checker. This breaks the codebase's own established
pattern: every other admin route screen has a matching `*.test.tsx`
(`admin-menu.test.tsx`, `restaurant-settings.test.tsx`), and `choose-password.tsx`'s own doc comment
says it renders "outside the application shell, like sign in and register" — but it was never added
to `web/src/app/routes/auth-screens.test.tsx`, the file literally titled "the screens outside the
shell," so it is the one screen in that group that never got the accessibility check
(`expectAccessible`) its siblings get, despite AC-21 requiring the same accessibility bar. New
branching logic with no coverage at all includes: the version-conflict retry flow in
`EditStaffDialog`, the self-row action hiding in `AdminStaffScreen`, and the two new router loader
gates (`requireIdentity`'s redirect to the change screen, `requireOwedPassword`). `docs/scope/scope.md`
marks "Test it" as done for this feature; that should be read as the Rust suite only.
