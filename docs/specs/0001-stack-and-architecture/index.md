# 0001. Stack and architecture for the restaurant operations platform

**Date**: 2026-08-07
**Status**: Proposed

## Summary

This decision fixes the technology the whole platform is built on. A React single page app (one codebase serving three very different screens: admin, waiter, kitchen) talks to a Rust API, backed by Postgres, all running on AWS. Live updates reach the kitchen and the waiter over server sent events, a plain HTTP stream the server pushes down, fanned out between servers by Postgres itself. Authentication, realtime, and infrastructure are all built and operated in house rather than bought from a platform, which buys full control and low running costs at the price of roughly a month of foundation work before the first order reaches a kitchen screen.

Reasoning and options: see [rationale.md](rationale.md).

## Decision

**Chosen option**: Option 1: Rust API plus React single page app, self operated on AWS.

The platform is a layered monolith: one Rust binary serving a JSON API and a server sent event stream, one React single page app built by Vite, one Postgres database, deployed as containers on AWS ECS Fargate with static assets on S3 behind CloudFront.

**Implementation skills**: `rust-best-practices` (`apollographql/skills`, `.claude/skills/rust-best-practices/`) · `rust-async-patterns` (`wshobson/agents`, `.claude/skills/rust-async-patterns/`) · `axum-web-framework` (`manutej/luxor-claude-marketplace`, `.claude/skills/axum-web-framework/`) · `postgresql-table-design` (`wshobson/agents`, `.claude/skills/postgresql-table-design/`) · `react-router-data-mode` (`remix-run/agent-skills`, `.claude/skills/react-router-data-mode/`) · `tanstack-query` (`tanstack-skills/tanstack-skills`, `.claude/skills/tanstack-query/`) · `shadcn` (`shadcn/ui`, `.claude/skills/shadcn/`)

## Proposed stack

| Layer | Choice | Reason |
|---|---|---|
| Architecture pattern | Layered monolith (handlers, services, repositories) | One engineer, no users yet. A single deployable unit is the fastest thing to build, debug, and operate; services can be extracted later, they cannot easily be merged back. |
| Backend language | Rust, stable channel, edition 2024, pinned in `rust-toolchain.toml` | Engineer's choice. Genuinely well suited to the long lived connections the kitchen screen needs, and cheap to run because one small container handles a lot of concurrent streams. |
| Backend framework | Axum 0.8 | Maintained by the Tokio team, so the least friction in async Rust. Its Tower middleware layer supplies auth, tracing, timeouts, and rate limiting instead of hand written code. |
| Async runtime | Tokio (multi threaded scheduler) | Implied by Axum; the runtime every crate in this stack targets. |
| Primary database | PostgreSQL on AWS RDS, single instance, start at db.t4g.micro | Relational data with real relationships and money that must add up. Managed backups and point in time recovery, which a restaurant's billing history requires. |
| Database access | SQLx with the `query!` macros (compile time checked SQL) | A wrong column name fails the build, not the dinner rush. Reports and tax maths stay plain SQL. Needs a live database or a checked in `.sqlx` offline cache at build time. |
| Migrations | `sqlx-cli`, plain SQL files in `api/migrations/`, run as a one off ECS task before the new task set takes traffic | Plain SQL is inspectable and reversible by hand. Running as a separate deploy step avoids several booting containers racing to migrate the same database. |
| Realtime transport | Server sent events on `GET /api/events`, one stream per authenticated session | Every push in this product is one way, server to client; actions travel as ordinary POSTs. Plain HTTP with browser native reconnection, so no upgrade handshake and no keepalive protocol to get wrong. |
| Realtime fan out | Postgres `LISTEN` / `NOTIFY` on one global channel, one listener connection per API instance. The payload is JSON carrying `restaurant_id`, entity type, and entity id | Any instance notifies, every instance pushes to its own matching clients. Scales past one container with no extra infrastructure to run, pay for, or monitor. The restaurant id in the payload is what keeps one restaurant's events off another restaurant's stream. |
| Tenant isolation | Scoped repository types in Rust, plus Postgres row level security as a backstop, set per request with `SET LOCAL app.restaurant_id` inside a transaction | Two independent locks on the scariest risk in the scope. A forgotten `WHERE` clause is caught by the database rather than leaking another restaurant's orders. Only holds if the database roles below are set up correctly. |
| Database roles | Three roles: an owner role that owns the schema and runs migrations, a least privilege `app_api` role the running API connects as (owning nothing, holding only the table grants it needs), and the RDS master used for neither | Postgres skips row level security for a table's owner. An API connecting as the owner would bypass every policy silently, so the backstop would look present in code and do nothing in reality. |
| Health checking | `GET /api/health`, returning 200 only when the connection pool answers and the listener connection is alive. The load balancer target group checks this path | A container whose listener connection has died is still serving requests but no longer delivering tickets. A liveness only check would keep that container in service indefinitely. |
| Sessions | Opaque random tokens (32 bytes from a cryptographic random source), stored hashed in a `sessions` table, delivered in an httpOnly, Secure, SameSite=Lax cookie | Revocation is instant, which the scope demands: feature 10 requires a deactivated account to be refused at once. Costs one indexed lookup per request. |
| Password hashing | `argon2` crate, argon2id, default parameters reviewed against current guidance at build time | The current standard for password hashing, memory hard against GPU cracking. |
| Frontend build | Vite with React 19 and TypeScript | Fast builds and instant reload; the standard single page app toolchain. |
| Frontend routing | React Router 7 in data mode, three route groups (`/admin`, `/waiter`, `/kitchen`) behind a role gate | One codebase, one deploy, shared components and types across three surfaces. Data mode (not framework mode) is the correct match for a single page app with a separate API. |
| Server state | TanStack Query | Caching, refetching, and loading and error states handled once instead of on every screen. Also the landing point for incoming events: push the change into the query cache and every screen showing that ticket re renders. |
| Styling and components | Tailwind CSS v4 with shadcn/ui | shadcn components are copied into the repo, so a kitchen screen can get enormous touch targets while admin screens stay dense. Built on Radix, so keyboard and focus behaviour is already correct. |
| Translations | react-i18next, one JSON file per language, lazy loaded | Adding a language is dropping in one file, exactly what feature 6 asks for. Plurals and per locale dates and numbers included. |
| API contract | `utoipa` generates an OpenAPI document from the Rust handlers; `openapi-typescript` plus `openapi-fetch` generate the typed client | Two languages means the seam needs keeping honest. Rename a field in Rust and the frontend fails to compile instead of failing on a Saturday night. |
| Validation | `serde` plus the `validator` crate on the server (the authority); `zod` on the frontend for form input only | Never trust the client; the server rejects independently of whatever the browser checked. |
| Money | Postgres `NUMERIC` with the `rust_decimal` crate, no floating point anywhere | Exact decimal arithmetic, so percentage tax on a subtotal behaves the way an accountant expects. Handles two decimal and zero decimal currencies alike. |
| Timestamps | `timestamptz` everywhere, never bare `timestamp`; each restaurant carries its own timezone column | Restaurants may be in different countries. Elapsed ticket time and daily revenue both need an unambiguous instant plus a local day. |
| Compute | AWS ECS Fargate behind an Application Load Balancer, arm64 (Graviton), multi stage Docker build on a Debian slim runtime | Long lived containers, which server sent event streams and the Postgres listen loop both require. No servers to patch. arm64 is roughly 20 percent cheaper for identical work. |
| Static hosting | S3 behind CloudFront, with `/api/*` routed from the same distribution to the load balancer | One origin, so there is no CORS to configure and the session cookie simply works. Static assets served from a CDN for pennies. |
| Infrastructure as code | AWS CDK in TypeScript, in `infra/` | High level constructs wire a load balanced Fargate service, networking, security groups, and health checks in a few dozen lines, and it is a language already in the project. |
| CI/CD | GitHub Actions: test, build and push the image to ECR, run migrations as a one off task, roll the ECS service, build and sync the web app to S3, invalidate CloudFront | One workflow, free at this volume, and migrations land before new containers take traffic. |
| Secrets and config | AWS SSM Parameter Store, injected into the task definition by ECS | No secret in the repository or in a build log. Standard parameters are free. |
| Environments | Production plus each engineer's local machine. No staging yet | Pre launch and solo, so a staging environment mostly doubles the bill and the infrastructure to maintain. Add one when a real restaurant depends on the system. |
| Local development | Docker Compose running the same Postgres major version as RDS, with the Vite dev server proxying `/api` to the local Rust process | One command to a working database, and the live database SQLx needs to check queries against at compile time. The proxy makes local development same origin too, so the session cookie behaves exactly as it does behind CloudFront. |
| Testing | Rust integration tests against a real Postgres inside a rolled back transaction; Vitest for frontend units; Playwright for the two device browser scenarios | Testing the real SQL is the whole point of compile time checked queries. Playwright is what proves a dish sent on one device appears on another. |
| Observability | `tracing` plus `tracing-subscriber` emitting structured JSON to stdout, collected by CloudWatch Logs | Structured logging from day one. Error and crash monitoring proper is feature 3 and is decided in its own spec. |
| Background jobs | None in the first version. When one is genuinely needed, a Tokio interval task inside the API process | No job exists yet. Adding a queue before there is work for it is infrastructure with no payer. |
| File storage | None in the first version | Nothing in slices 1 to 6 uploads a file. When menu photos arrive, S3 with presigned uploads, never the database. |

**Repository layout**: one repository. `api/` (Rust), `web/` (React), `infra/` (CDK), `docs/`. Node 22 LTS with pnpm on the JavaScript side.

**Load bearing implementation notes** (these are the details that make or break the choices above, `/develop` must honour them):

- **Every tenant scoped table needs `FORCE ROW LEVEL SECURITY`, not just `ENABLE`.** `ENABLE` alone still exempts the table owner. With `FORCE` set, and the API connecting as the non owner `app_api` role, the policy applies to every query the application can make. Without both, the backstop is decorative.
- **Row level security only works if the session variable is set.** Every request handler opens a transaction and issues `SET LOCAL app.restaurant_id = $1` before any query. `SET LOCAL` is scoped to the transaction, so a pooled connection cannot leak the previous request's restaurant.
- **Make that structural, not a convention.** Handlers receive a request extractor that yields only an already scoped transaction handle. Raw pool access lives in a module handlers do not import. Bypassing the scoping should be a compile error, not something a tired engineer remembers on feature 19. A written rule will be forgotten across twenty two features; a type will not.
- **The notification payload carries the restaurant id, and the instance routes on it.** Each instance keeps a map of open streams keyed by restaurant id, and pushes an incoming notification only to the streams belonging to that restaurant. Broadcasting every event to every open stream and letting the client filter is the obvious shortcut and it leaks ticket volume, timing, and identifiers across restaurants over the wire, even though the follow up fetch would be denied.
- **Events carry a type and an entity id, never row content.** The client refetches or patches the TanStack Query cache. This keeps row level security authoritative: a client that should not see a row gets nothing back when it asks.
- **Notifications are fire and forget, so the client must resynchronise on connect.** Postgres queues nothing for a listener that is not connected. On every stream open, including every reconnect, the client refetches its active queries rather than assuming it missed nothing. Without this, one network blip is a ticket that never appears on the kitchen screen.
- **If an instance's listener connection dies, close that instance's streams.** A process whose `LISTEN` connection has dropped is still serving HTTP but is no longer delivering anything. Closing its streams makes clients reconnect and resynchronise, which is far better than a screen that looks connected and is not. The health endpoint reports the same condition so the load balancer can replace the task.
- **Keep the event stream alive through the load balancer.** The Application Load Balancer's default idle timeout is 60 seconds and will close a quiet stream. Raise it to 300 seconds and send a comment heartbeat every 15 seconds from the server.
- **One listener connection per instance, not per client.** The `LISTEN` connection is taken from outside the main pool and held for the process lifetime. A connection per connected screen would exhaust Postgres by the second restaurant.
- **Avoid the NAT gateway.** Fargate tasks in private subnets need a NAT gateway at roughly 32 US dollars a month plus data charges, which would cost more than everything else combined. Run the tasks in public subnets with no inbound access except from the load balancer's security group, or use VPC endpoints.
- **Rate limit the authentication endpoints** with `tower-governor` from the first slice that has a login form.
- **Error types**: `thiserror` for domain errors mapped to HTTP status codes, `anyhow` only at the binary edge.

## Consequences

**Positive**

- Running costs are low and predictable, roughly 50 to 70 US dollars a month at launch scale, and the architecture has a lot of headroom before any of it needs revisiting.
- Restaurant separation is enforced twice, once in application code and once in the database, which is the correct treatment for the risk the scope itself names as the most expensive to get wrong. This holds only if the database roles and `FORCE ROW LEVEL SECURITY` are set up as specified above; get that wrong and the second lock is decorative while looking present in the code.
- The compile time checked SQL plus the generated API client mean two whole categories of bug (a wrong column, a drifted API field) fail the build rather than a dinner service.
- Everything is owned: no vendor can change pricing, deprecate a feature, or hold the user table.
- Rust's concurrency story fits the actual workload. One small container will hold a lot of open kitchen screens without effort.

**Negative and tradeoffs**

- **The first order reaches a kitchen screen roughly a month later than it would on a managed platform.** Authentication, session handling, the realtime pipe, and the AWS foundation are all work that a platform would have supplied on day one. This is the central cost of this decision and it is accepted knowingly.
- **Authentication is being built rather than bought**, which is the single highest risk item in this spec. Session fixation, timing safe token comparison, cookie flags, reset token expiry, and rate limiting are each a way to get breached, and there is no reviewer.
- **Rust is slow to iterate in.** Compile times and the borrow checker both tax exploratory work, and the product ahead has a lot of ordinary create, read, update, delete screens where that tax buys nothing.
- **AWS is a large surface to learn while also building a product.** Load balancers, task definitions, security groups, and IAM policies are each a place to lose an afternoon.
- **The Rust agent skill ecosystem is thin.** No first party Axum or SQLx skill exists, so implementation guidance for the backend is weaker than for the frontend.
- **Two languages means two toolchains**, two dependency systems, two test runners, and a generated seam between them that has to be regenerated whenever the API changes.
- No staging environment means risky migrations are rehearsed only against local data.
- **A rolling deploy closes every open event stream at once**, so every kitchen and waiter screen reconnects simultaneously. Survivable because clients resynchronise on reconnect, but it means deploying during a dinner service is visibly disruptive. Deploy between services.
- **One shared database instance means no isolation between restaurants.** A single heavy or badly indexed query starves connections for every other restaurant mid service. This is the strongest argument for the reporting queries in feature 18 getting real attention rather than being written casually.

**Neutral**

- Single instance RDS means a maintenance window is downtime. Acceptable before real restaurants; revisit with Multi AZ when one depends on it. Note there is no globally quiet hour to schedule it in once restaurants span countries, so the window is somebody's dinner service by definition.
- The single page app renders in the browser, so the public marketing page in feature 19 needs its own answer for search engines. That is that feature's spec to solve, most likely by pre rendering that one page.
- Extracting a service later is possible but nothing about this design pushes toward it.

## Follow-up

- [ ] Feature 3 (error and crash monitoring) decides the error tracking tool. This spec only fixes structured logging to CloudWatch, which is not the same thing as knowing a waiter's screen broke mid shift.
- [ ] Feature 4 (core data model) owns the schema. This spec fixes only the conventions the schema must follow: `timestamptz`, `NUMERIC` for money, a restaurant id on every tenant scoped table, and a row level security policy per such table.
- [ ] Feature 7 (accounts, restaurants, and roles) owns the role model, registration, and password reset. This spec fixes only the session machinery underneath it.
- [ ] The seven installed community skills are project wide conventions and belong in the root `AGENTS.md` `## Agent skills` section. Feature 2 runs `/audit`, which writes that file.
- [ ] Record as declined so they are not offered again: the AWS CDK agent skill, the Playwright agent skill, the AWS MCP servers, and Playwright MCP.
- [ ] Connect a Postgres MCP server once feature 4's schema exists. It is a user configuration step, not something a skill can do, and it lets the agent read the real schema instead of trusting a migration file.
- [ ] Before the first login form ships, have the authentication code reviewed by a second pair of eyes or a dedicated security review. This is the one area of the stack where building in house carries breach risk rather than merely cost.
- [ ] Revisit hosting cost after the first paying restaurant. If the AWS bill outgrows the value it delivers, the same containers run on a cheaper platform with modest changes, since nothing here depends on an AWS specific service beyond ECS, S3, and Parameter Store.
