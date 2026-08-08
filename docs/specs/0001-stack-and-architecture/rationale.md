# 0001. Stack and architecture: reasoning and options

The decision itself is in [index.md](index.md). This file is the decision record: why, what else was weighed, and what it is grounded in. Builds skip it.

## Context

> ⚠️ Premise note: this stack has one engineer building and operating authentication, a realtime pipe, and an AWS foundation from scratch, in a language that is slow to iterate in. Building authentication yourself is a well known failure pattern: session fixation, timing safe token comparison, reset token expiry, cookie flags, and rate limiting are each an independent way to be breached, and a solo project has no reviewer to catch a mistake. The managed alternative (Option 2 below) would have delivered authentication, realtime, and storage on day one and put a working order on a kitchen screen in days rather than weeks. The engineer was shown that comparison and chose this direction knowingly. The design proceeds accordingly, with the risk recorded here and a Follow-up item requiring the authentication code to get a second review before the first login form ships.

The product is a multi restaurant operations platform. Many restaurants sign up; each has an owner acting as admin, plus waiters and chefs. Waiters open a bill on a table and send rounds of dishes to the kitchen, chefs see tickets appear live and mark each dish done, and the bill is totalled and closed at the end of the meal. The full scope runs to twenty two features across six slices (see `docs/scope/scope.md`).

Four forces dominate.

**Three genuinely different screens.** An admin on a desktop needs dense tables and forms. A waiter needs one hand phone use during a busy service. A chef needs a screen readable across a hot kitchen, operable with wet or gloved hands. These are not three breakpoints of one design, they are three interaction models sharing one data set.

**Live updates are the product, not a nice touch.** The scope's own success criterion for the first slice is that a dish sent by a waiter appears on the kitchen screen within a second or two, on a different device, without a refresh, and that marking it done alerts the waiter the same way. A stack that makes this awkward makes the product awkward.

**Multi restaurant separation is the expensive mistake.** Every table carrying operational data belongs to exactly one restaurant, and a leak between two restaurants is not a bug, it is a business ending incident. The scope calls the data model the most expensive thing in the project to get wrong, and separation is the reason.

**One engineer, no users yet, and a real product ahead.** There is no team to absorb operational complexity and no revenue to fund it. Anything chosen here has to be buildable, debuggable, and operable by one person on a Tuesday evening. At the same time the scope is not a toy: money, tax, printing, reporting, and legal pages are all in it.

Money is also load bearing in an unusual way: each restaurant sets its own currency, its own tax components, and its own service charge, a closed bill must keep the rates it was charged at, and the reports must agree with the bills. That constrains the numeric type and rules out floating point.

Not deciding is not an option. Every later feature rests on this, and retrofitting tenant separation, a realtime mechanism, or a money type after twenty screens exist is a rewrite rather than a change.

## Options considered

Full stacks, not individual technologies. Each is described as its best advocate would.

### Option 1: Rust API plus React single page app, self operated on AWS (chosen)

A Vite built React single page app with three route groups, a separate Axum API in Rust, Postgres on RDS, containers on ECS Fargate, and authentication, sessions, and the realtime pipe all written in house. Server sent events carry pushes, fanned out between instances by Postgres `LISTEN` / `NOTIFY`.

**Pros**

- Rust is genuinely excellent at the thing this product needs most: many long lived connections held cheaply by a small process, with no runtime surprises during a dinner rush.
- Compile time checked SQL turns a whole class of bug into a build failure, and the reporting and tax queries stay plain SQL where they belong.
- Nothing is rented. No vendor can change pricing, deprecate a feature, or hold the user table.
- Low and predictable running costs, roughly 50 to 70 US dollars a month, with a lot of headroom before anything needs revisiting.
- Postgres does the fan out, so the realtime design scales past one container without a message broker to run.

**Cons**

- Roughly a month of foundation work before the first order reaches a kitchen screen. Authentication, sessions, realtime, and the AWS foundation are all built rather than supplied.
- Building authentication is the highest risk item in the whole plan, and there is no second reviewer.
- Rust taxes exploratory work through compile times and the borrow checker, on a product with many ordinary create, read, update, delete screens where that tax buys nothing.
- Two languages, two toolchains, and a generated seam between them to keep in step.
- AWS is a large surface to learn while also learning the product.
- The Rust agent skill ecosystem is thin, so implementation guidance for the backend is weaker than for the frontend.

### Option 2: Next.js on Supabase, hosted on Vercel

One TypeScript codebase using Next.js server components and server actions, with Supabase supplying Postgres, authentication, realtime, and storage. Row level security enforces restaurant separation inside the database, and Supabase Realtime authorizes every pushed event against those same policies.

**Pros**

- Four of the seven foundation features arrive largely solved on day one: database, authentication, realtime, and storage.
- Restaurant separation is enforced by database policies that the realtime layer also honours, so a subscription cannot leak what a query could not.
- One language across the whole codebase, and by far the fastest path to a working order on a kitchen screen.
- Generous free tiers make the pre revenue period nearly free, and the public marketing page gets server rendering and search engine handling with no extra work.
- Much the largest body of documentation, examples, and answers of any option here.

**Cons**

- Two vendors own the platform, and both pricing and feature direction are theirs to change.
- Realtime authorization checks run per subscriber per change, which needs care at scale (broadcasting from database triggers rather than subscribing to raw table changes).
- Costs rise faster than a self operated setup once traffic is real.
- Less control over the exact behaviour of long lived connections than a container you own.
- Serverless functions are a poor host for anything needing a persistent process.

### Option 3: TypeScript everywhere, self operated

Next.js or React Router with a Node API, plain managed Postgres, Better Auth for identity, and a self written realtime layer, on a container platform such as Railway or Fly.io.

**Pros**

- One language everywhere, so no generated seam between frontend and backend types and half the toolchain to maintain.
- Users live in your own database, and Better Auth's organization support maps closely onto restaurants, staff, and roles.
- Far faster to iterate in than Rust, on a product that is mostly forms and tables.
- No platform vendor holding core product capability.

**Cons**

- Still requires building the realtime pipe and operating the infrastructure, so most of Option 1's foundation work remains.
- Node holds many concurrent open connections less comfortably than Rust, which is exactly the workload the kitchen screen creates.
- No compile time SQL checking of the kind SQLx gives, so schema drift shows up at runtime.
- Gets neither the speed of Option 2 nor the control and efficiency of Option 1.

### Option 4: Rust and React, but on a managed container platform

The same application as Option 1, deployed on Fly.io or Railway with their managed Postgres instead of AWS.

**Pros**

- Removes the largest single chunk of learning curve. No load balancer, task definition, security group, or CDK to write.
- Container based, so it hosts long lived connections properly, unlike serverless.
- Cheaper than AWS at small scale and considerably faster to a first deploy.
- Keeps every application level advantage of Option 1 unchanged.

**Cons**

- Less control over networking and infrastructure detail than AWS gives.
- A smaller platform is a larger dependency: its outage is your outage, with less recourse.
- Migrating to AWS later is real work, since infrastructure code does not transfer.
- Does not remove the authentication and realtime work, only the AWS part.

## Rationale

Option 1 was the engineer's choice, made after seeing Option 2's timeline advantage stated plainly. This record keeps that comparison rather than rewriting history.

Within that choice, every layer decision follows from the four forces in Context.

**Server sent events over websockets** is the most consequential call, and it comes straight from the traffic pattern. Every push in this product runs one way, server to client; a waiter sending a round and a chef marking a dish are ordinary POST requests. A websocket would supply a return channel nothing uses, in exchange for reconnection logic, keepalive protocol, and idle timeout tuning all written and debugged by hand. Server sent events are plain HTTP with reconnection built into the browser. The scope's own target, a second or two, is met comfortably either way.

**Postgres `LISTEN` / `NOTIFY` over Redis** follows the operational reality constraint. Redis would be a second piece of infrastructure to run, pay for, monitor, and recover, solving a problem the database already solves. Because every API instance holds its own listener connection, the fan out works across several containers with nothing added, so this is not a choice that has to be revisited at the second container.

**Both application scoping and row level security** was chosen against the simpler single lock alternatives because the risk is asymmetric. A forgotten `WHERE` clause is an ordinary mistake that any engineer makes eventually; a solo engineer with no reviewer will make it. In this product that ordinary mistake shows one restaurant another restaurant's orders. Paying a small per request cost to make that mistake non fatal is straightforwardly worth it. Application scoping stays the primary mechanism because policy only debugging is genuinely painful.

**Opaque database sessions over JSON web tokens** is driven by a specific line in the scope rather than by general preference: feature 10 requires a deactivated staff account to be refused at once. Stateless tokens make that the awkward case, and the usual fix, a deny list, reintroduces the database lookup that made tokens attractive. Buying immediate revocation for one indexed lookup is the right trade for a product where a fired waiter keeping access until token expiry is a real operational problem.

**Exact decimal money over integer minor units** was chosen because integers only appear to remove the hard part. Per restaurant currencies with different decimal places, several tax components, and a service charge all produce fractions that require a rounding decision either way; decimals keep the arithmetic looking like the rules it implements, which matters when a bill has to be defended to a restaurant owner.

**A layered monolith** needs little defence at this size. One engineer, no users, and services that could be extracted later but never easily merged back.

**AWS CDK over Terraform** inverts the usual advice, deliberately. Terraform is the more portable and better documented tool, but its ECS, load balancer, RDS, and CloudFront setup runs to several hundred lines written by hand, and this engineer is new to AWS. CDK's high level constructs produce the same thing in a few dozen lines with the fiddly parts wired correctly, in a language already in the project. Removing learning curve was worth more here than portability.

**Skipping staging** is a deliberate cost decision that carries a real cost: risky migrations get rehearsed against local data only. That is acceptable while no restaurant depends on the system and should stop being acceptable the day one does.

## Landscape check, August 2026

A bounded web check ran during the design conversation to keep the options current rather than remembered. What it established:

- **Axum is the pragmatic default for a new Rust API in 2026.** Actix Web still leads raw throughput by roughly 10 to 15 percent and has a more mature websocket story, but Axum's Tower ecosystem and Tokio team maintenance win for a codebase one person maintains. The throughput gap is irrelevant at restaurant scale.
- **SQLx is the async first choice**, with the `query!` macro validating SQL against a live database at compile time. Its known cost is build time: continuous integration needs either a database or a checked in offline query cache. Diesel catches more at compile time but adds a query language to learn and slower compiles; SeaORM adds indirection this project does not need.
- **Neither Axum nor SQLx has a first party agent skill.** Registry search found only small community entries (479 installs for the leading Axum skill, and the leading SQLx entry is an individual's dotfiles). This is recorded as a real gap in the Consequences.
- **Managed Postgres and platform comparisons** confirmed the Option 2 and Option 4 descriptions above, including that Fly.io no longer offers a free tier to new accounts and that Coolify on a cheap virtual server runs at roughly a tenth of comparable managed platform cost.
- **On authentication**, the current landscape reinforces the premise note: the mature options are all buy rather than build, and each of the three leading products treats organizations and roles as a first class primitive, which is precisely the model this product needs.

## Cross check, August 2026

An independent critique on a different model reviewed the drafted spec before it was accepted. Six gaps were found and all six were closed in `index.md` rather than deferred. Recorded here because two of them are the kind of mistake that would have looked correct in code review and in tests:

1. **Row level security was specified but would not have run.** Postgres exempts a table's owner from its own policies unless `FORCE ROW LEVEL SECURITY` is set. The original draft named no separate application role, so the natural implementation (application and migrations sharing one database user) would have bypassed every policy silently. Closed by specifying three database roles and `FORCE` on every tenant scoped table.
2. **Notification scoping was undefined.** The draft said events carry a type and an entity id but never said the payload carries a restaurant id, nor how an instance routes a notification to the right streams. The shortcut a builder would reach for, broadcasting to every open stream, leaks ticket volume and timing across restaurants even though the follow up fetch is denied. Closed by specifying the payload and the per instance routing map.
3. **No recovery from a missed notification.** `NOTIFY` queues nothing for a disconnected listener, so a blip meant a ticket that never appeared. Closed by requiring resynchronisation on every stream open, and by closing an instance's streams when its listener connection dies.
4. **No health check definition.** Closed by specifying `GET /api/health` and what it must actually check.
5. **Local development is cross origin** even though production is not. Closed with a Vite dev proxy.
6. **The transaction scoping rule was a convention rather than a guard.** Closed by requiring a request extractor that only yields a scoped transaction handle, so bypassing it fails to compile.

The critique also confirmed that the `SET LOCAL` claim is correct (transaction scoped, resets before the connection returns to the pool) and raised two production failure modes now recorded in Consequences: a rolling deploy closes every stream at once, and a single shared database instance gives no isolation between restaurants.

## References

**Project sources**

- `docs/scope/scope.md`: the twenty two feature scope, the Tracer Bullet build approach, and the GA workflow tier. Feature 1's success condition (the stack recorded in a spec and a scaffold that boots and builds) is what this spec satisfies.
- `docs/scope/scope.md` feature 10: the requirement that a deactivated account is refused at once, which drove the session mechanism.
- `docs/scope/scope.md` feature 14: per restaurant currency, tax components, and service charge, which drove the money type.
- Installed community skills: `rust-best-practices`, `rust-async-patterns`, `axum-web-framework`, `postgresql-table-design`, `react-router-data-mode`, `tanstack-query`, `shadcn`.
- No `AGENTS.md` exists yet; feature 2 creates it via `/audit`.

**Practices and standards**

- Monolith first: extract services only when a measured bottleneck or a team ownership boundary forces it.
- A relational database as the default primary store; the NoSQL case is specific and none of it applies here.
- Defence in depth for tenant isolation: enforce in the application and again in the database.
- Never build authentication from scratch without a documented reason. Raised, overridden knowingly, and recorded.
- argon2id for password hashing; opaque session tokens stored hashed, never in plain text.
- Serverless is unsuitable for long lived connections and stateful processes.
- Object storage for files, never the database.
- Structured logging from day one rather than as an afterthought.

**Links** (verified during the August 2026 landscape check; comparison articles rather than official documentation, so read them as a snapshot of current opinion)

- Axum vs Actix Web 2026: https://rustify.rs/articles/rust-axum-vs-actix-web-2026
- Axum vs Actix Web vs Rocket comparison 2026: https://reintech.io/blog/axum-vs-actix-web-vs-rocket-rust-framework-comparison-2026
- Diesel vs SQLx vs SeaORM comparison 2026: https://reintech.io/blog/diesel-vs-sqlx-vs-seaorm-rust-database-library-comparison-2026
- Rust ORMs 2026, SQLx vs Diesel vs SeaORM: https://byteiota.com/rust-orms-2026-sqlx-vs-diesel-vs-seaorm-comparison/
- Fly.io vs Railway vs Render vs Coolify 2026: https://www.devtoolreviews.com/reviews/fly-io-vs-railway-vs-render-vs-coolify-2026
- Authentication options compared 2026: https://makerkit.dev/blog/tutorials/better-auth-vs-clerk
- Supabase realtime with row level security (the Option 2 mechanism): https://supabase.com/docs/guides/realtime/postgres-changes

**Installed skill sources**

- https://skills.sh/apollographql/skills/rust-best-practices
- https://skills.sh/wshobson/agents/rust-async-patterns
- https://skills.sh/wshobson/agents/postgresql-table-design
- https://skills.sh/manutej/luxor-claude-marketplace/axum-web-framework
- https://skills.sh/shadcn/ui/shadcn
- https://skills.sh/remix-run/agent-skills/react-router-data-mode
- https://skills.sh/tanstack-skills/tanstack-skills/tanstack-query
