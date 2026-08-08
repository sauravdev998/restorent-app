# Restaurant Operations Platform

A platform many restaurants sign up for: waiters take orders at the table, tickets appear live
in the kitchen, food is marked ready per dish, and the meal ends with a bill.

## Stack

- **Language / Runtime**: Rust (stable, edition 2024, pinned in `rust-toolchain.toml`) and TypeScript on Node 24 (root `package.json` pins `engines.node >= 24`)
- **Framework**: Axum 0.8 on Tokio for the API; React 19 with Vite and React Router 7 in data mode for the web app
- **Key dependencies**: SQLx with the `query!` macros against PostgreSQL, TanStack Query, Tailwind CSS v4 with shadcn/ui, `utoipa` plus `openapi-typescript` for the typed API client
- **Package manager**: `cargo` for `api/`, `pnpm` for `web/` and `infra/`
- **Layout**: one repository. `api/` (Rust), `web/` (React), `infra/` (AWS CDK in TypeScript), `docs/`
- **Realtime**: server sent events on `GET /api/events`, fanned out between instances by Postgres `LISTEN` / `NOTIFY`
- **Hosting**: AWS ECS Fargate behind an Application Load Balancer, static assets on S3 behind CloudFront, Postgres on RDS

Full reasoning and the load bearing implementation notes: [docs/specs/0001-stack-and-architecture/index.md](docs/specs/0001-stack-and-architecture/index.md).

## Build approach

**Tracer Bullet**: a thin thread pierces every layer and works, then you thicken it.

## Commands

Root `package.json` wraps these. Run them from the repository root.

```bash
# Install
cargo fetch --manifest-path api/Cargo.toml && pnpm install

# Dev server (database first, then migrations, then API, then web)
pnpm db:up && pnpm migrate && pnpm dev:api && pnpm dev:web

# Build
pnpm build

# Test
pnpm test

# Everything continuous integration runs
pnpm check

# Regenerate the committed artifacts, after changing SQL or a handler's types
pnpm sqlx:prepare && pnpm client:generate
```

Local Postgres listens on 5434, not 5432. `.env.example` carries both connection strings and says which is which.

## Specs

Stored in `docs/specs/`. Format: `docs/specs/NNNN-title/index.md`.

## Rules

- **Four layers in `api/`**, dependencies pointing inward only: `domain` (entities and their invariants), `application` (use cases and services), `infrastructure` (SQLx repositories, the Postgres listener), `presentation` (Axum handlers). No `axum` or `sqlx` type appears anywhere in `domain/` or `application/`.
- **Tenant scoping is structural, not a convention.** Handlers receive an extractor yielding an already scoped transaction handle. Raw pool access lives in a module handlers do not import, so bypassing the scoping fails to compile rather than relying on someone remembering.
- **Folder shape differs per side**: `api/` groups by layer (the list above), `web/` groups by feature (`admin/`, `waiter/`, `kitchen/`, `shared/`). The three web surfaces barely share screens.
- **Strict types everywhere.** TypeScript `strict` plus `noUncheckedIndexedAccess`, no `any`, no unchecked casts. Rust denies warnings, clippy pedantic where it is sane, and no `unwrap` or `expect` outside tests and `main`.
- **One error handling pattern.** `thiserror` for domain errors mapped to HTTP status codes, `anyhow` only at the binary edge, one error shape on every API response. On the web side, TanStack Query error states and a single error boundary, never ad hoc try catch.
- **Config is validated at startup.** Every environment variable and secret is read once at boot into a typed struct, and the API refuses to start if one is missing or malformed. A missing password fails the deploy, not the first request mid service.
- **Public APIs are documented.** Public Rust items carry doc comments (`missing_docs` denied); every exported hook or component says what it is for.
- **Nothing crosses a boundary as a domain entity.** Data travels as DTOs: serde structs on the API, generated TypeScript types on the web.
- **Money is `NUMERIC` and `rust_decimal`, never a float. Timestamps are `timestamptz`, never bare `timestamp`.**
- **Two database roles, never one.** Migrations run as the schema owner (`OWNER_DATABASE_URL`); the API connects as `app_api` (`DATABASE_URL`), which owns nothing, so row level security applies to it. Postgres skips row level security for a table's owner, so connecting as the owner would silently disable the tenant backstop.
- **Tests follow the layers.** Domain and application are unit tested with no infrastructure mocked; repositories are integration tested against a real Postgres inside a rolled back transaction; Vitest covers web units; Playwright covers the two device scenarios.

## Tooling

Chosen here, installed by `/develop tooling`.

- **Lint and format**: `rustfmt` and `clippy`; ESLint and Prettier (ESLint carries the `react-hooks`, `jsx-a11y`, and TanStack Query rules this stack needs)
- **Before commit**: format and lint changed files only. Typecheck, `cargo check`, and tests are left to CI, because a cold Rust build in a commit hook trains people to use `--no-verify`
- **Continuous integration**: full checks on every push. `cargo fmt --check`, clippy denying warnings, `cargo test` against a Postgres service container, `tsc`, ESLint, Vitest. Playwright joins once slice 1 exists
- **Generated artifacts are committed**: the `.sqlx` offline cache and the generated TypeScript API client. CI regenerates both and fails if the result differs, so a renamed Rust field breaks the pull request instead of a Saturday night

## Git

- integration: on
- branch prefix: `feat/`
- commit: per-milestone
- messages: conventional commits (`feat:`, `fix:`, `chore:`, `docs:`, optional scope such as `feat(kitchen):`)
- default branch: `main`

Push and pull request always ask first.

## Agent skills

Backend:

- [rust-best-practices](.agents/skills/rust-best-practices/): `apollographql/skills`, idiomatic Rust, ownership, borrowing, and `Result` handling
- [rust-async-patterns](.agents/skills/rust-async-patterns/): `wshobson/agents`, async Rust, concurrency, and async error handling
- [tokio-patterns](.agents/skills/tokio-patterns/): `geoffjay/claude-plugins`, Tokio idioms: worker pools, timeouts, retries, graceful shutdown
- [axum-web-framework](.agents/skills/axum-web-framework/): `manutej/luxor-claude-marketplace`, Axum routing, extractors, Tower middleware, state
- [rust-backend](.agents/skills/rust-backend/): `windmill-labs/windmill`, Rust backend conventions from a production Axum and SQLx codebase
- [sqlx-postgres](.agents/skills/sqlx-postgres/): `daiki48/dotfiles`, SQLx query macros, migrations, transactions, enums, JSONB
- [postgresql-table-design](.agents/skills/postgresql-table-design/): `wshobson/agents`, Postgres schema design, data types, indexing, constraints

Frontend:

- [react-router-data-mode](.agents/skills/react-router-data-mode/): `remix-run/agent-skills`, route objects, loaders, actions, pending and optimistic UI
- [tanstack-query](.agents/skills/tanstack-query/): `tanstack-skills/tanstack-skills`, server state, caching, refetching, cache updates from incoming events
- [shadcn](.agents/skills/shadcn/): `shadcn/ui`, component installation, composition, styling, and forms
- [tailwind-4-docs](.agents/skills/tailwind-4-docs/): `lombiq/tailwind-agent-skills`, Tailwind v4 utilities, variants, and its CSS based configuration
- [react-i18next](.agents/skills/react-i18next/): `yildizberkay/skills`, translation setup, namespaces, plurals, interpolation
- [vitest](.agents/skills/vitest/): `antfu/skills`, web unit tests, mocking, coverage, fixtures

Skills live in `.agents/skills/`, which every agent reads. `.claude/skills/` holds symlinks to it.

Declined: AWS CDK skill, Playwright skill, `softaworks/agent-toolkit@openapi-to-typescript`, AWS MCP servers, Playwright MCP

MCP servers: Postgres (recommended, worth connecting once feature 4 creates a real schema, so the agent reads the live schema instead of trusting a migration file)

## Context files

- [api/AGENTS.md](api/AGENTS.md): the four layers, the scoped transaction, the listen connection, and the timeout ladder around them
- [web/AGENTS.md](web/AGENTS.md): feature folders, the generated API client, and the two rules that keep the query cache honest
- [infra/AGENTS.md](infra/AGENTS.md): the one CDK stack, not yet deployed, and the numbers that keep a stream alive

_Drafted by /audit from the repo, worth a quick human pass. Edit freely: once a line stops matching this draft, later runs treat it as curated and will flag rather than overwrite it._
