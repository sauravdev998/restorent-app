# API (Rust, Axum)

## Overview

The JSON over HTTP API plus the live event stream every kitchen and waiter screen holds open.
Four layers with dependencies pointing inward only, one Postgres listen connection per process
fanning changes out to the streams that process is holding, and a health endpoint that reports
an instance as unhealthy the moment it stops delivering. Root `AGENTS.md` holds the project wide
rules; this file holds what is true only here.

## Key files

| File | Owns |
|---|---|
| `src/lib.rs` | The layer map, and the rule that a handler cannot reach the pool |
| `src/main.rs` | The binary edge: config, pool, listener, router, graceful shutdown. The only place `anyhow` is allowed |
| `src/domain/` | Entities and their rules. `error.rs` is the one error type the inner layers speak |
| `src/application/ports.rs` | Traits the use cases need, which `infrastructure` implements |
| `src/infrastructure/config.rs` | The only module that reads an environment variable |
| `src/infrastructure/db/mod.rs` | The only module that holds a `PgPool`. The field is private |
| `src/infrastructure/db/scoped.rs` | `ScopedTx`: a transaction with `app.restaurant_id` already set |
| `src/infrastructure/passwords.rs` | `argon2id` hashing and verification, both run off the async runtime |
| `src/infrastructure/events/listener.rs` | The listen connection, its bounded connect and its backoff ladder |
| `src/infrastructure/events/registry.rs` | Which open streams belong to which restaurant |
| `src/presentation/router.rs` | Route table and the middleware around it |
| `src/presentation/error.rs` | The one error shape every failed request returns |
| `src/presentation/extract/actor.rs` | Who is asking: the session lookup, the restaurant it scopes to, and the role requirement carried in the handler's own type |
| `src/presentation/origin.rs` | The same origin check every mutating request passes before a handler runs |
| `src/presentation/openapi.rs` | The OpenAPI document. A route missing from `paths` never reaches the client |
| `migrations/0001_bootstrap.sql` | `current_restaurant_id()` and `notify_entity_change()`. No tables on purpose |
| `migrations/0004_accounts_and_sessions.sql` | Credentials, sessions, `login_attempts`, and `resolve_session`, the second of the two `auth_lookup` owned lookups |
| `scripts/init-roles.sql` | Creates `app_api` locally. Run by hand on RDS |
| `Dockerfile` | Multi stage arm64 build. Build it from the repository root, not from here |
| `Cargo.toml` | Dependencies, and the `[lints.clippy]` block that turns pedantic on |

## Commands

Run from the repository root.

```bash
cargo run --manifest-path api/Cargo.toml            # serve
cargo test --manifest-path api/Cargo.toml           # tests
cargo clippy --manifest-path api/Cargo.toml --all-targets -- -D warnings
pnpm sqlx:prepare                                   # refresh the committed .sqlx cache
pnpm sqlx:check                                     # what continuous integration runs
pnpm openapi:generate                               # rewrite api/openapi.json
```

## Conventions

- **Imports point inward only.** No `axum` and no `sqlx` type appears in `domain/` or `application/`. A use case reaches the world through a trait it declares in `application/ports.rs`.
- **Every query goes through `Database::begin_scoped`.** It returns a `ScopedTx` with `app.restaurant_id` already set. `ScopedTx::new` is visible only inside `infrastructure::db`, so there is no other way to build one.
- **A read that assembles a document uses `Database::begin_scoped_snapshot` instead.** Same scoping, but at `REPEATABLE READ`, so every statement in the transaction sees one moment. Writes keep using `begin_scoped`.
- **Only `infrastructure::config` reads the environment.** Everything else takes a typed value.
- **`#![deny(missing_docs)]`.** Every public item carries a doc comment. No `unwrap` or `expect` outside tests and `main`.
- **One error shape.** Handlers return `DomainError`; `presentation/error.rs` maps it to a status code and a JSON body. A database message stays in the logs and never rides out on a response.
- **A role restriction lives in the handler's own signature**, carried by the `Actor` extractor's type, so it reaches the OpenAPI document and a missing check is visible rather than remembered. Refusal happens before the handler body runs.
- **Every mutating request passes the same origin check** in `presentation/origin.rs` before its handler: the `Origin` header's host must equal the request's own `Host`, with `Sec-Fetch-Site: same-origin` accepted instead, and neither header refused. It needs no configured value, so there is nothing to get wrong per environment.
- **Nothing crosses the wire as a domain entity.** `presentation` owns its own serde DTOs, so an inner layer never learns the `utoipa` schema traits exist.
- **A new route must be added to `paths(...)` in `presentation/openapi.rs`**, or it is absent from the document, and therefore absent from the generated TypeScript client, with nothing failing to tell you.
- **Change SQL, run `pnpm sqlx:prepare`.** The `.sqlx` cache is committed and the Docker build reads it with `SQLX_OFFLINE=true`.
- **Clippy pedantic is on**, configured in `[lints.clippy]` in `Cargo.toml`, and continuous integration runs it with `-D warnings`, so a warning fails the build. Two lints are allowed, each with the reason written beside it: `single_match_else` crate wide, and `duration_suboptimal_units` on the one test whose constants mirror seconds written in `infra/`. Add an exception the same way, argued, never blanket.

## Gotchas

- **The timeout ladder is load bearing and its parts live apart.** Pool acquire is 5 seconds and the router cuts a request off at 30, so a caller gets the `503` the handler chose rather than a bodiless `408`. The health probe is 2 seconds against a 5 second health check. One listen connect attempt is bounded at 5 seconds, well under the 30 second backoff ceiling, because SQLx's own default would otherwise make every attempt cost 30 seconds. Tests in `db/mod.rs` and `events/listener.rs` pin these relationships.
- **`PgListener::connect` is deliberately not used.** It builds its own pool and leaves SQLx's 30 second acquire timeout on it, unreachable from outside. `connect_bounded` builds the pool here so the bound also covers the reconnect the listener does internally.
- **`/api/events` is mounted outside the timeout and compression layers.** A timeout would close it every 30 seconds, and compression buffers, which is the one thing a live ticket feed must not do.
- **The 15 second heartbeat is sized against numbers in `infra/`** (CloudFront `readTimeout` 60 seconds, load balancer `idleTimeout` 300 seconds). This crate cannot import them, so the test in `handlers/events.rs` pins the relationship against copies. Change a number in `infra/lib/platform-stack.ts` and nothing here goes red.
- **A request's restaurant comes from its session and from nowhere else.** `Actor` resolves the session cookie against the database on every request, with no cache, and hands the handler the restaurant to scope to. The old `RestaurantScope` placeholder, the `x-restaurant-id` header, and the `?restaurant_id=` query parameter are gone; nothing may reintroduce a client supplied restaurant, in any environment.
- **Exactly two paths read across restaurants**, both `SECURITY DEFINER` functions owned by `auth_lookup` rather than by the schema owner: the credential lookup from `0002` and `resolve_session` from `0004`. Owned by the schema owner instead, `FORCE ROW LEVEL SECURITY` filters them to nothing and nobody can sign in. A migration that drops and recreates one must re assert the owner and the grant, because a `DROP` takes both with it.
- **`/api/dev/notify` exists only in development and is deliberately absent from the OpenAPI document**, so it is absent from the typed client too. That is on purpose: the document describes the real API surface.
- **The listen connection dying ends every stream on the instance.** That is intended. A screen that looks connected while receiving nothing is worse than one that reconnects and refetches.
- **A multi statement read at `READ COMMITTED` can return a state the database never held.** Each statement takes its own snapshot, so a write landing between two of them is half visible. The real one: a ticket read as `cooking` while every dish on it read `ready`, and the waiter's ready alert then never fired. `begin_scoped_snapshot` is the fix, and the window is milliseconds locally and much wider against a database across the internet, so this hides in development.
- **`0001_bootstrap.sql` creates no tables.** Feature 4 owns the schema. What it creates is the plumbing every later table uses.

## Agent skills

- [rust-best-practices](../.agents/skills/rust-best-practices/): `apollographql/skills`, idiomatic Rust, ownership, borrowing, and `Result` handling
- [rust-async-patterns](../.agents/skills/rust-async-patterns/): `wshobson/agents`, async Rust, concurrency, and async error handling
- [tokio-patterns](../.agents/skills/tokio-patterns/): `geoffjay/claude-plugins`, Tokio idioms: worker pools, timeouts, retries, graceful shutdown
- [axum-web-framework](../.agents/skills/axum-web-framework/): `manutej/luxor-claude-marketplace`, Axum routing, extractors, Tower middleware, state
- [rust-backend](../.agents/skills/rust-backend/): `windmill-labs/windmill`, Rust backend conventions from a production Axum and SQLx codebase
- [sqlx-postgres](../.agents/skills/sqlx-postgres/): `daiki48/dotfiles`, SQLx query macros, migrations, transactions, enums, JSONB
- [postgresql-table-design](../.agents/skills/postgresql-table-design/): `wshobson/agents`, Postgres schema design, data types, indexing, constraints

## Related specs

- [0001 stack and architecture](../docs/specs/0001-stack-and-architecture/index.md)

_Drafted by /audit from the repo, worth a quick human pass. Edit freely: once a line stops matching this draft, later runs treat it as curated and will flag rather than overwrite it._
