# Verify: coding standards and tooling · updated 2026-08-08

_Steps derived from the scope feature 2 **Done when** line, since this feature has no decision spec: the choices it installs were recorded in root `AGENTS.md` `## Tooling` by `/audit`. `/check verify` runs these; `/test` locks the durable ones._

_Run these from the repository root with the local database up (`pnpm db:up && pnpm migrate`)._

## Commands

- [ ] `pnpm format:check` → passes. Covers `cargo fmt --check` and Prettier over the whole repo from the single root `.prettierrc.json` → Done when, format runs clean
- [ ] `pnpm lint` → passes. Covers clippy with `-D warnings` and pedantic on, ESLint in `web/`, ESLint in `infra/` → Done when, lint runs clean
- [ ] `pnpm typecheck` → passes for both `web` and `infra`
- [ ] `pnpm test` → passes. 16 Rust tests, 12 Vitest tests
- [ ] `pnpm check` → passes end to end. This is the same command CI runs, so a green laptop should mean a green pipeline
- [ ] `pnpm sqlx:check` → passes. The committed `.sqlx` cache matches the SQL in the source
- [ ] `pnpm client:check` → passes. The committed `api/openapi.json` and `web/src/shared/api/schema.d.ts` match what the Rust handlers generate

## The pre commit hook

- [ ] `pnpm install` → `.git/hooks/pre-commit` exists and is executable. The `prepare` script runs `lefthook install`, so a fresh clone gets the hook without a separate instruction → Done when, the pre commit hook runs clean
- [ ] Stage a badly formatted `.ts` file, then commit → the commit succeeds and the committed content is formatted, not just the file on disk. This is `stage_fixed` working; without it the fix would sit outside the commit
- [ ] Stage a badly formatted `.rs` file, then commit → same, and only that file is reformatted. The hook runs `rustfmt` on the staged files rather than `cargo fmt` on the crate, so an unstaged file nearby is left alone
- [ ] Stage a `.ts` file holding a real lint error (a non null assertion, `value!.length`) → the commit is refused and ESLint names the rule
- [ ] Time a commit touching one or two files → under about two seconds. The hook deliberately skips typecheck, `cargo check`, and the test suites: a hook that costs a minute is a hook people escape with `--no-verify`

## Continuous integration

- [ ] Push a branch → the `CI / Checks` workflow runs and passes
- [ ] Push a commit that renames a serde field on an API response without regenerating → CI fails at "The generated API client is current"
- [ ] Push a commit that changes SQL without running `pnpm sqlx:prepare` → CI fails at "The SQLx offline cache is current"

_Not yet exercised against a real GitHub runner: everything above the CI section was run locally and passed. The CI section is verified by the first push._

## Done when coverage

- Root `AGENTS.md` reflects the real stack … covered by the `/audit` run that preceded this build, plus the Node 24 correction
- Lint runs clean … `pnpm lint`
- Format runs clean … `pnpm format:check`
- The pre commit hook runs clean … the five hook steps above
