# Review, feat/accounts-restaurants-and-roles, 2026-09-06

**Reviewed by**: Claude Opus 5 (author on Claude Sonnet 5, per commit trailers)
**Scope**: 123 files (+21 generated `.sqlx` cache files), branch vs `main` (merge base `12369d8f`)
**Verdict**: Approve with nits

## Summary

This lands the whole authentication, session, and role model spec 0006 calls for: opaque
session tokens hashed with SHA-256, `argon2id` passwords hashed off the runtime with a
constant-time refusal path, a `RoleRequirement`-typed `Actor<R>` extractor that replaces the
`RestaurantScope` placeholder, a two-bucket database-backed throttle serialised by a
transaction-level advisory lock, an `Origin`/`Sec-Fetch-Site` same-origin check on mutating
methods, and a matching web side (sign in, register, account, restaurant settings, the `401`
redirect path). The security-critical primitives — cookie attributes, token hashing, password
hashing and its constant-time dummy path, the origin check, row level security scoping, the
open-redirect guard on `next` — are all correct, well tested at the unit level, and match the
spec's stated invariants closely. The one real gap is test coverage: the two acceptance criteria
that are explicitly about *concurrent* correctness (the registration race and the throttle's
advisory lock) are proven only by sequential unit tests and a manual `curl -parallel` step in
`verify.md`, not by an automated test exercising genuine concurrency, even though the exact
`tokio::spawn` pattern needed already exists in `api/tests/concurrency.rs`. Everything else is
nits: a trait doc comment that overclaims what actually reaches the OpenAPI document, and a
couple of fragile string-matched error mappings.

## Major

### 🟠 The throttle's advisory lock and the registration race are not proven under real concurrency, `api/tests/accounts.rs`
**Problem**: Spec 0006's own "Critical test scenarios" section names two scenarios explicitly:
"two concurrent registrations with the same email address" (AC-2) and "several sign in attempts
for one address issued in parallel do not all pass the count, because the advisory lock
serialises the count and the insert" (AC-10). Milestone 13's whole reason for existing is that
serialisation. What exists today:
- `a_duplicate_address_leaves_no_restaurant_behind` (accounts.rs:111) runs two registrations
  *sequentially*, one after the other in the same test. It proves the transaction rolls back
  cleanly on a unique-index conflict, which is real and good, but it says nothing about whether
  two requests racing genuinely at the same time behave the same way.
- `one_address_running_out_of_attempts_does_not_stop_another` (accounts.rs:538) calls
  `record_login_attempt` five times in a straight-line loop, `.await`ed one after another. It
  proves the *count* is right; it cannot prove the `pg_advisory_xact_lock` actually blocks a
  second transaction that arrives while the first is still between its count and its insert,
  which is the entire bug the lock exists to prevent.
- `docs/specs/0006-accounts-restaurants-and-roles/verify.md` lists "six sign ins in parallel"
  as a manual `curl` step, checked off once by a human, not as a regression test.

The codebase already has the right pattern for this, unused here: `api/tests/concurrency.rs`
opens two `Database` handles, starts one transaction, `tokio::spawn`s the second against a
`database.clone()`, and asserts on the interleaving (see
`two_waiters_racing_for_a_table_leave_exactly_one_party_at_it`, concurrency.rs:59-95).

**Why it matters**: This is precisely the kind of security-relevant, branching, concurrency-
sensitive code the review guide asks to weigh heavily, and it is exactly the kind of bug that is
invisible until it is exploited: a wrong lock key, a lock taken on the wrong connection, an
`await` accidentally introduced between the count and the insert, or a future refactor that
moves the two into separate transactions would all pass every test in this suite today while
silently reopening the race the design section spends a full paragraph justifying. A regression
here would not fail CI; it would fail in production, under load, which is exactly when the
throttle is supposed to hold.

**Suggested fix**: Add one test modelled on `concurrency.rs`'s pattern: open N `tokio::spawn`
tasks against `database.clone()`, all calling `record_login_attempt` for the same lowered email
at (as close to) the same instant, and assert that at most `MAX_ATTEMPTS_PER_EMAIL` succeed and
the remainder are `DomainError::Throttled`. A second test doing the same for two concurrent
`accounts::register` calls with the same email, asserting exactly one `Ok` and one
`DomainError::Conflict`, would additionally close the AC-2 gap and could reuse the existing
`register_and_sign_in`-adjacent fixtures.

## Minor

### 🟡 `RoleRequirement::DESCRIPTION`'s doc comment overclaims what it actually does, `api/src/presentation/extract/actor.rs:52-58`
**Problem**: The doc comment on `RoleRequirement::DESCRIPTION` says "It travels into the
generated `OpenAPI` document... so the requirement is visible to whoever is building the screen
as well as to whoever is reading the Rust." In fact `DESCRIPTION` is read in exactly one runtime
place (the `tracing::info!` refusal log in `actor.rs:171`) and in its own unit tests
(`actor.rs:312-315`). Every `#[utoipa::path]` macro documents its role restriction as separate,
hand-typed prose in the `summary`/`description`/response text (confirmed in `openapi.json`'s
`/api/restaurant` entry: `"summary": "Changes the restaurant's settings. Admins only."`), with no
compile-time or generation-time link back to `Admin::DESCRIPTION`.
**Why it matters**: AC-8 asks for the role restriction to be "visible... in the `OpenAPI`
document," which it is, but the mechanism that makes it visible is a human remembering to type
matching prose beside each handler, not the type-level guarantee the comment describes. A future
role marker (or a change to what `Admin::DESCRIPTION` says) can drift from the OpenAPI prose with
nothing pointing it out, which is the exact failure mode the rest of this feature goes out of its
way to make impossible by construction.
**Suggested fix**: Either wire `R::DESCRIPTION` into the utoipa path definition (e.g. as part of
the generated `description`) so the two cannot drift, or narrow the doc comment to say plainly
that the OpenAPI text is written by hand and must be kept in sync manually.

### 🟡 Field-error mapping matches on message substrings, `api/src/presentation/handlers/me.rs:308-317`
**Problem**: `settings_field_error` decides which form field a repository-level
`DomainError::Invalid` belongs to by testing `reason.contains("timezone")` and
`reason.contains("restaurant name")` against the free-text message built in
`accounts::update_settings` (`api/src/infrastructure/db/repository/accounts.rs:344,362`).
**Why it matters**: Nothing ties these two message fragments to each other at compile time. A
future edit to either error message's wording (a very plausible refactor, since both messages
read as prose meant for a log) silently stops matching, and the specific field error quietly
degrades into the generic `error.into()` fallback — a real regression that no compiler warning
and no existing test would catch, since the current tests exercise the field-error path via the
handler's own validation (catalogue checks), not via this string-matched branch.
**Suggested fix**: Have `update_settings` return a typed reason (e.g. a small enum or the
existing `FieldError` itself) instead of a formatted `String` for these two cases, so the handler
matches on a variant rather than a substring.

## Nits

- ⚪ `api/src/infrastructure/db/mod.rs:293` (`record_login_attempt`): the doc comment is excellent
  but the function does two logically separate things (check-and-throttle, then record) under one
  name; a reader scanning call sites in `auth.rs` has to open this function to learn that it also
  performs the insert. Not worth splitting given how tightly the two must stay atomic, just worth
  the doc comment's existing "records this one, and refuses if either bucket is already full"
  phrasing carrying into the function name if it is ever touched again.
- ⚪ `api/src/presentation/handlers/auth.rs:215-216`: `sign_in` trims but does not lower the email
  before using it as the throttle/log/sweep key in this function's own scope, relying on
  `record_login_attempt`/`clear_login_attempts`/`correlation_of` to each lower internally. Correct
  today (verified against every call site), but it means "already lowered" is an invariant three
  separate functions each re-establish rather than one the type system carries, unlike
  `EmailAddress::lowered()` a few lines above in `credentials.rs` which exists for exactly this.
- ⚪ `web/src/shared/session/identity.ts:63`: `snapshot` is module-level mutable state written from
  three different call sites (`identityQuery.queryFn`, `rememberIdentity`, `forgetIdentity`). The
  file's own doc comment is careful to say this is a cache of what the server already returned and
  not a second source of truth, which is the right framing, but nothing enforces that new call
  sites keep respecting it — worth a lint-level "only these three functions may assign `snapshot`"
  note if this file grows more writers later.

## Strengths

- The timing-safety work is genuinely thorough, not just present: `Argon2Passwords::verify_nothing`
  computes its dummy hash lazily against the crate's live parameters (so it can't quietly become
  cheaper than a real verify if the parameters ever change), and
  `verifying_nothing_still_costs_a_hash` asserts on wall-clock time to catch a future refactor that
  short-circuits it. Combined with `sign_in` recording the throttle attempt and calling
  `verify_nothing()` on every non-match path (unknown address, unparseable password, wrong
  password), a wrong password and an unknown address are provably indistinguishable in both body
  and cost.
- `presentation/origin.rs` is a model of a security control that documents its own threat model
  inline and tests every edge deliberately: opaque `Origin: null`, a suffix-matching subdomain
  attack (`app.example.com.elsewhere.example`), a same-host-different-port attack, and the
  fail-closed default for a request carrying neither header.
- The session-lifetime invariants (`SESSION_ABSOLUTE_LIFETIME > SESSION_LIFETIME`,
  `SLIDE_AFTER * 100 < SESSION_LIFETIME`, etc.) are pinned by both a domain-level unit test and,
  independently, a `CHECK` constraint in Postgres, and migration `0005` is an honest, well-narrated
  fix of a real bug the pair of tests didn't originally catch (the ceiling-clamp interaction),
  including a correct follow-up fix to `resolve_session`'s derived-vs-stored `last_seen_at` problem.
- Tenant isolation is enforced three independent ways exactly as the design section promises (role
  extractor, RLS via `begin_scoped`, composite foreign keys), and `isolation.rs` still proves the
  two `SECURITY DEFINER` unscoped reads are the *only* two paths that cross restaurants, including
  the specific historical failure mode (a function owned by the schema owner silently returning
  nothing under `FORCE ROW LEVEL SECURITY`) called out by name in its own test.
- The web side's open-redirect guard on `next` (`returnPathFrom` in `signed-out.ts`) is tested
  against a real attack list (`https://elsewhere.example`, `//elsewhere.example`,
  `javascript:alert(1)`), not just the happy path, and `useIdentity`'s loader-vs-query fallback is
  exercised for both the personal-language and restaurant-rename cache-freshness cases.
- The `.sqlx` cache churn (3 deleted, 3 added) matches migration `0005`'s `resolve_session` and
  `slide` changes exactly — verified by diffing the deleted files' stored query text against the
  new ones — so it is not stale.

## Test coverage

Strong at the unit level across every security-sensitive primitive: cookie attributes, token
hash round-tripping, password rules (including the byte/character-count distinction with real
multi-byte examples), the origin check, the client-address parser (including the IPv6-colon
pitfall it exists to avoid), the field-error and `JsonBody` extractor shapes, and the audit-action
label/position pinning. Integration coverage against a real Postgres (`accounts.rs`,
`isolation.rs`) covers registration atomicity, session sliding and its ceiling, sign-out scoping,
password change's session revocation, and the sign-in sweeps' scoping — all sequential and all
solid.

The one substantive gap is concurrency, detailed above: the two acceptance criteria that are
specifically about parallel/racing requests (AC-2's duplicate registration race, AC-10's advisory
lock) are exercised only sequentially in the automated suite, with the true concurrent case
checked off as a manual `curl` step in `verify.md` rather than a regression test, despite the
exact `tokio::spawn`-based pattern already existing in this repository for other race conditions.
Frontend coverage (`use-identity.test.tsx`, `signed-out.test.ts`, `auth-screens.test.tsx`,
`restaurant-settings.test.tsx`, `account.test.tsx`) is thorough and reads as if it drove the
implementation rather than following it.
