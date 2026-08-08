# 0003. Core data model for the restaurant operations platform

**Date**: 2026-08-08
**Status**: In Progress

## Summary

This decision fixes the database schema every other feature reads and writes: restaurants and their settings, staff and sessions, the menu, tables, and the whole path from a party sitting down to a paid bill. The shape that matters most is how one restaurant's data is kept away from another's, and it is guarded three times over: a scoped transaction, a database policy, and foreign keys that carry the restaurant id so a cross restaurant link is physically impossible. Money figures and dish names are copied onto a bill when it closes, so a bill printed a year later still says what the customer actually paid. Feature 4 ships the migration, the matching Rust types, and the tests that prove the isolation works. It ships no endpoints; the features above it add those.

Reasoning and options: see [rationale.md](rationale.md).

## Requirements

**User stories**:

- As a restaurant owner, I want my data to be unreachable by any other restaurant on the platform, so that signing up costs me nothing in confidentiality.
- As a restaurant owner, I want a bill I closed last year to still show exactly what I charged, so that my accounts and my tax filings agree with my records.
- As a waiter, I want one thing open per table for the whole meal, so that a second waiter cannot start a duplicate order on a table I am already serving.
- As a chef, I want to mark one dish done without touching the rest of the ticket, so that food leaves the pass as it is ready.
- As an engineer building feature 7 and beyond, I want the entities, their states, and their rules already fixed, so that I am not inventing schema partway through a slice.

**Acceptance criteria** (the contract, each independently checkable against a real Postgres):

- **AC-1**: A table holds at most one open visit. A second insert of an open visit for the same table is refused by the database, not by application code.
- **AC-2**: A cross restaurant reference is impossible. Inserting an order line whose dish belongs to another restaurant, or a bill whose visit belongs to another restaurant, is refused by the database even when the inserting transaction is correctly scoped.
- **AC-3**: A transaction scoped to restaurant A reads zero rows belonging to restaurant B, on every tenant scoped table, for select, update, and delete alike.
- **AC-4**: A transaction with no restaurant set reads zero rows from every tenant scoped table, and the API database role cannot bypass this, because every such table has both `ENABLE` and `FORCE ROW LEVEL SECURITY` and the role owns nothing.
- **AC-5**: A closed bill is unchanged by later edits. After a bill closes, changing a dish price, renaming a dish, archiving a dish, or editing a tax component or the service charge leaves every figure and every name on that bill exactly as it was.
- **AC-6**: Bill numbers are per restaurant, allocated at close, unique, and gapless. Two bills closing at the same moment in the same restaurant receive two different consecutive numbers, and an abandoned bill consumes none.
- **AC-7**: Status is per dish. Marking one order line ready leaves every other line in the round untouched, and the round becomes ready exactly when its last line that is neither served nor voided becomes ready.
- **AC-8**: A state change from an unexpected state is refused. When two people act at once, the losing write changes zero rows and the caller is told the state moved, rather than overwriting it.
- **AC-9**: Money is exact. Every money column is `numeric(14,4)` and never null, stored bill figures are already rounded to the restaurant's own currency decimals half away from zero, and a bill's total equals its subtotal plus its service charge plus the sum of its tax components, with no residue.
- **AC-10**: Archiving hides without breaking. An archived dish, table, section, category, or staff member disappears from working queries, and every bill, round, and line that referenced it still resolves.
- **AC-11**: Deletion is complete or absent. Deactivating a restaurant leaves all of its data intact and reachable; deleting a restaurant row removes every row belonging to it across every tenant table, leaving no orphan.
- **AC-12**: There are exactly two paths that read across restaurants: the login lookup by email and the session token resolution, both through named security definer functions with fixed return shapes, owned by a role that owns no table and reached through one named policy per table. Both return the right row with no restaurant set, and no other unscoped read path exists in the schema.
- **AC-13**: A bill's figures match exactly the lines assigned to it. Reassigning a line between two bills of the same restaurant is permitted and recomputes both; reassigning a line to a bill of another restaurant is refused.
- **AC-14**: Consequential changes are recorded. Every void, bill close, dish price edit, tax component edit, service charge edit, staff role change, and staff deactivation writes one audit row carrying who, when, which entity, and the before and after values.
- **AC-15**: Time is unambiguous. Every timestamp column is `timestamptz`, and the local day a bill belongs to is derived from the restaurant's own timezone, not the server's.

## Decision

**Chosen option**: Option 1: visit centred schema with composite tenant keys.

Sixteen tables and six enum types, in one migration, where a visit owns table occupancy, bills are payment documents drawn from a visit's lines, and every tenant table carries `restaurant_id` inside both its unique constraints and its foreign keys so that a cross restaurant link cannot be written at all.

**Implementation skills**: `postgresql-table-design` (`wshobson/agents`, `.agents/skills/postgresql-table-design/`) · `sqlx-postgres` (`daiki48/dotfiles`, `.agents/skills/sqlx-postgres/`) · `rust-best-practices` (`apollographql/skills`, `.agents/skills/rust-best-practices/`) · `rust-backend` (`windmill-labs/windmill`, `.agents/skills/rust-backend/`)

## Rationale

Reasoning, the options weighed, and the two standard rules knowingly broken: see [rationale.md](rationale.md).

## Feature design

**Data model sketch**

Enum types (Postgres enums, mapped to Rust enums by SQLx):

| Type | Values |
|---|---|
| `staff_role` | `admin`, `waiter`, `chef` |
| `visit_status` | `open`, `closed` |
| `round_status` | `queued`, `ready`, `served`, `voided` |
| `line_status` | `queued`, `ready`, `served`, `voided` |
| `bill_status` | `open`, `closed`, `voided` |
| `payment_method` | `cash`, `card`, `other` |

Conventions applied to every table below: primary key `id uuid` (version 7, generated in Rust), `created_at timestamptz not null default now()`, `updated_at timestamptz not null` written by the application (no triggers), money `numeric(14,4)`, percentages `numeric(6,3)`. Every tenant table additionally carries `restaurant_id uuid not null`, a `unique (id, restaurant_id)` constraint, composite foreign keys, and row level security as described under Security model.

| Table | Columns | Foreign keys | Constraints |
|---|---|---|---|
| `restaurants` | `id`, `name`, `currency_code char(3)`, `currency_decimals smallint`, `timezone text`, `service_charge_percent numeric(6,3) null`, `address text null`, `tax_registration_number text null`, `deactivated_at timestamptz null` | none, this is the tenant root | `currency_decimals between 0 and 4`. The timezone is validated in Rust on write, not by a check constraint, because a check constraint cannot query `pg_timezone_names` |
| `tax_components` | `id`, `restaurant_id`, `name`, `rate_percent numeric(6,3)`, `position int`, `archived_at null` | restaurants | `rate_percent >= 0`, unique `(restaurant_id, name)` where not archived |
| `staff` | `id`, `restaurant_id`, `email text`, `password_hash text`, `display_name text`, `role staff_role`, `deactivated_at null` | restaurants | unique index on `lower(email)`, platform wide, not per restaurant. A plain unique index on the lowered value, not the `citext` extension, so no extension is added for one column |
| `sessions` | `id`, `restaurant_id`, `staff_id`, `token_hash bytea`, `expires_at`, `last_seen_at`, `revoked_at null` | staff (composite) | unique `token_hash`, index on `(staff_id)` |
| `table_sections` | `id`, `restaurant_id`, `name`, `position int`, `archived_at null` | restaurants | unique `(restaurant_id, name)` where not archived |
| `dining_tables` | `id`, `restaurant_id`, `section_id null`, `label text`, `seats smallint null`, `position int`, `archived_at null` | table_sections (composite) | unique `(restaurant_id, label)` where not archived |
| `menu_categories` | `id`, `restaurant_id`, `name`, `position int`, `archived_at null` | restaurants | unique `(restaurant_id, name)` where not archived |
| `dishes` | `id`, `restaurant_id`, `category_id`, `name`, `description text null`, `price numeric(14,4)`, `is_available bool`, `position int`, `archived_at null` | menu_categories (composite) | `price >= 0` |
| `visits` | `id`, `restaurant_id`, `table_id`, `status visit_status`, `guest_count smallint null`, `opened_by_staff_id`, `opened_at`, `closed_at null` | dining_tables, staff (composite) | unique `(restaurant_id, table_id)` where `status = 'open'` |
| `order_rounds` | `id`, `restaurant_id`, `visit_id`, `sequence_no int`, `status round_status`, `sent_by_staff_id`, `sent_at`, `ready_at null`, `served_at null` | visits, staff (composite) | unique `(restaurant_id, visit_id, sequence_no)` |
| `order_lines` | `id`, `restaurant_id`, `round_id`, `dish_id`, `bill_id null`, `quantity int`, `unit_price numeric(14,4)`, `dish_name text`, `line_total numeric(14,4)`, `note text null`, `status line_status`, `ready_by_staff_id null`, `ready_at null`, `served_at null`, `voided_by_staff_id null`, `voided_at null`, `void_reason text null` | order_rounds, dishes, bills, staff (composite) | `quantity > 0`, `unit_price >= 0`, `line_total = quantity * unit_price`, `status = 'voided'` requires `void_reason`. `round_id` is not null: a line exists only once it has been sent, so the waiter's unsent basket is client side state and is never persisted |
| `bills` | `id`, `restaurant_id`, `visit_id`, `number bigint null`, `status bill_status`, `currency_code char(3)`, `currency_decimals smallint`, `subtotal not null default 0`, `service_charge_percent numeric(6,3) null`, `service_charge_amount not null default 0`, `tax_total not null default 0`, `total not null default 0`, `opened_by_staff_id`, `closed_by_staff_id null`, `closed_at null` | visits, staff (composite) | unique `(restaurant_id, number)`, `status = 'closed'` requires `number` and `closed_at`, all amounts `not null` and `>= 0`. A null `service_charge_percent` means no service charge and an amount of zero, never a null amount |
| `bill_taxes` | `id`, `restaurant_id`, `bill_id`, `name text`, `rate_percent numeric(6,3)`, `amount numeric(14,4)` | bills (composite) | unique `(bill_id, name)` |
| `payments` | `id`, `restaurant_id`, `bill_id`, `method payment_method`, `amount numeric(14,4)`, `taken_by_staff_id`, `taken_at`, `note text null` | bills, staff (composite) | `amount > 0` |
| `bill_number_counters` | `restaurant_id` primary key, `next_number bigint not null default 1` | restaurants | one row per restaurant. Nothing relies on it having been created: the allocation is an upsert (`insert ... on conflict (restaurant_id) do update`), so a missing row creates itself rather than matching zero rows silently |
| `audit_log` | `id`, `restaurant_id`, `actor_staff_id null`, `action text`, `entity_type text`, `entity_id uuid`, `before jsonb null`, `after jsonb null`, `occurred_at` | restaurants | index on `(restaurant_id, occurred_at desc)`. `actor_staff_id` is nullable for the one case that has no staff member, a change made by the system itself (a migration, or a future automated job). Every change AC-14 names is staff initiated and carries an actor |

Indexes beyond the constraints above:

- `order_rounds (restaurant_id, sent_at)` where `status = 'queued'`: the kitchen queue, oldest first.
- `order_lines (restaurant_id, round_id)` and `order_lines (restaurant_id, bill_id)`: the two ways lines are read.
- `order_lines (restaurant_id, dish_id)`: dish ranking in feature 18.
- `bills (restaurant_id, closed_at)` where `status = 'closed'`: revenue by day and by hour.
- `visits (restaurant_id, status)`: the floor view of what is occupied.

**State transitions**

- **Visit**: `open` → `closed`, by the explicit `close_visit` operation, never as a side effect. Closing is refused while any of its bills is still `open`, or while any of its non voided lines is not assigned to a bill. A visit with no bills at all closes freely, which is how a party that sits down and leaves without ordering frees the table. Moving a party is an update to `table_id` while `open`, and it is refused if the destination already has an open visit.
- **Order line**: `queued` → `ready` → `served`, and `queued | ready` → `voided`. Voiding requires a reason and a staff member. There is no state before `queued`: a line is created by `send_round` and therefore already sent, so what the waiter deletes while building an order is client side basket state that never reached the database.
- **Order round**: its status is a total function of its lines, evaluated in the same transaction as every line write, in this order. Every line `voided` → `voided`. Otherwise every non voided line `served` → `served`. Otherwise no non voided line `queued` → `ready`. Otherwise → `queued`. Evaluating it in that order is what stops an all voided round from counting as ready.
- **Bill**: `open` → `closed`, and `open` → `voided`. Closing allocates the number, writes the snapshot figures, and is refused while any assigned line is neither `served` nor `voided`, and refused when the bill has no assigned non voided line at all, so an empty bill never consumes a number. A closed bill is never edited again.

**Interface surface**

This feature ships no HTTP endpoints; features 7 and above add those. The surface it does ship is the repository layer, and every operation runs inside a `ScopedTx` obtained from `Database::begin_scoped`, so the restaurant is already set before any statement runs.

| Operation | Key inputs | Key outputs | Auth | Notifies | Key errors |
|---|---|---|---|---|---|
| `open_visit` | table_id, opened_by, guest_count (opt) | visit | scoped tx | `visit` | table already occupied (unique violation), table archived |
| `move_visit` | visit_id, new_table_id | visit | scoped tx | `visit` | destination occupied, visit not open |
| `close_visit` | visit_id | visit | scoped tx | `visit` | a bill is still open, a non voided line is unassigned |
| `send_round` | visit_id, lines (dish_id, quantity, note) | round with lines | scoped tx | `order_round` | dish unavailable or archived, visit not open |
| `mark_line_ready` | line_id, staff_id | line, round status after | scoped tx | `order_line`, then `order_round` if its status changed | line not in `queued` (conflict), returns rows affected zero |
| `mark_line_served` | line_id | line, round status after | scoped tx | `order_line`, then `order_round` if its status changed | line not in `ready` (conflict) |
| `void_line` | line_id, staff_id, reason | line, round status after | scoped tx | `order_line`, then `order_round` if its status changed | line already served, missing reason |
| `open_bill` | visit_id, opened_by | bill, all figures zero | scoped tx | `bill` | visit not open |
| `assign_lines_to_bill` | bill_id, line_ids | bill with recomputed subtotal | scoped tx | `bill` | line belongs to another restaurant (composite key violation), bill not open |
| `close_bill` | bill_id, staff_id | bill with number and figures | scoped tx | `bill` | unserved line present, bill has no lines, bill already closed |
| `record_payment` | bill_id, method, amount, staff_id | payment | scoped tx | `bill` | bill not closed, amount not positive |
| `find_staff_for_login` | email | staff id, restaurant id, role, password hash, deactivated_at | none, security definer function | none | no match returns no row |
| `resolve_session` | token_hash | session id, staff id, restaurant id, role, expires_at | none, security definer function | none | expired or revoked returns no row |

Every operation that notifies calls `notify_entity_change(restaurant_id, entity, entity_id)` from `0001_bootstrap.sql`, inside the same scoped transaction that made the change, using the entity strings in the Notifies column. Those strings are the closed vocabulary the live pipeline runs on and they match the `EntityKind` variants in `domain/event.rs` one for one: `visit`, `order_round`, `order_line`, `bill`, `dish`, `dining_table`, `staff`. Nothing invents a string; adding one is a change to the Rust enum and to this table together.

Two operations take a row lock, and they take it on the bill: `assign_lines_to_bill` and `close_bill` both lock the bill row before reading its lines, so a close can never snapshot figures while lines are being reassigned under it. Two other places serialise per restaurant: the visit row lock that allocates a round's sequence number, and the counter row that allocates a bill number. No operation takes both of those, and if one ever must, it takes the visit lock before the counter lock.

**Value sourcing**

| Action | Value produced or displayed | Source |
|---|---|---|
| every request | which restaurant this transaction is scoped to | `resolve_session(token_hash)`, whose `restaurant_id` is passed to `Database::begin_scoped`, which issues `SET LOCAL app.restaurant_id` (spec 0001) |
| `send_round` | the line's unit price | `dishes.price` copied at send time, never read again afterwards |
| `send_round` | the line's dish name on the bill | `dishes.name` copied at send time |
| `send_round` | the round's sequence number | `max(sequence_no) + 1` for that visit, allocated under the visit's row lock |
| `mark_line_ready`, `mark_line_served`, `void_line` | the round's status after the write | the total function over that round's lines in State transitions, evaluated in the same transaction as the line update |
| `close_bill` | the bill number | `bill_number_counters.next_number` for that restaurant, allocated by one `insert ... on conflict (restaurant_id) do update set next_number = next_number + 1 returning`, which both creates a missing counter and increments an existing one. A rolled back transaction releases the number, which is what keeps the sequence gapless |
| `open_bill` | the bill's starting figures | zero, not null, for subtotal, service charge, tax total, and total |
| `assign_lines_to_bill` | which figures move before close | `subtotal` only. Service charge, tax total, and total stay zero until `close_bill` computes them once, so no half computed money figure is ever readable |
| `close_bill` | the subtotal | sum of `line_total` over the lines assigned to that bill and not voided |
| `close_bill` | each tax component name and rate on the bill | `tax_components` rows for that restaurant, copied into `bill_taxes` at close |
| `close_bill` | each tax amount | subtotal times the copied rate, rounded once to `restaurants.currency_decimals` |
| `close_bill` | the service charge percent and amount | `restaurants.service_charge_percent` copied onto the bill, applied to the subtotal, rounded once. A null percent yields an amount of zero, never a null |
| `close_bill` | how every rounding is performed | half away from zero (`rust_decimal`'s `MidpointAwayFromZero`), stated explicitly because `rust_decimal` rounds half to even by default, and half away from zero is what a customer reading a receipt expects |
| `close_bill` | the currency the bill is in | `restaurants.currency_code` and `restaurants.currency_decimals`, both copied onto the bill |
| `close_bill` | the total | subtotal plus rounded service charge plus the sum of rounded tax amounts |
| reporting reads (feature 18) | the local day a bill belongs to | `bills.closed_at` converted using `restaurants.timezone`, never the server timezone |
| any change worth telling screens about | the live event payload | `notify_entity_change(restaurant_id, entity, entity_id)` from `0001_bootstrap.sql`, called inside the same scoped transaction |
| any consequential change | the audit row's actor | the staff id resolved from the session for this request |

**Key invariants**

1. At most one `open` visit per dining table. Enforced by a partial unique index.
2. Every foreign key between two tenant tables carries `restaurant_id` and points at a `(id, restaurant_id)` unique constraint. There is no plain single column foreign key between tenant tables anywhere in the schema.
3. `line_total = quantity * unit_price`, enforced by a check constraint.
4. A closed bill's `total` equals `subtotal + service_charge_amount + sum(bill_taxes.amount)`, and its `subtotal` equals the sum of `line_total` over its assigned non voided lines. Enforced by the close path and by an integration test, not by a constraint, because it spans tables.
5. A bill in `closed` has a `number`, a `closed_at`, and a `closed_by_staff_id`, and none of its columns nor its `bill_taxes` rows ever change again.
6. A bill number is unique per restaurant and consecutive. Nothing allocates one except the close path.
7. A line in `voided` has a `void_reason` and a `voided_by_staff_id`.
8. A round's status is exactly the total function of its lines given in State transitions, with the all voided case tested first. A round whose lines are all voided is `voided`, never `ready`.
9. Every money column is `numeric(14,4)` and every timestamp column is `timestamptz`. No float, no bare `timestamp`, anywhere.
10. Every stored bill figure is already rounded to that bill's own `currency_decimals`, half away from zero, and no money column is ever null.
11. A closed bill has at least one assigned non voided line. An empty bill cannot close and cannot consume a number.
12. A visit is `closed` only when none of its bills is `open` and every non voided line under it is assigned to a bill. A visit with no bills satisfies this trivially.
13. Email identifies exactly one staff account platform wide.
14. Deleting a restaurant deletes every row referencing it, through `on delete cascade` on every tenant table's restaurant foreign key.

**Security model**

- **Every tenant table** gets `ALTER TABLE ... ENABLE ROW LEVEL SECURITY` and `FORCE ROW LEVEL SECURITY`, plus one policy `FOR ALL USING (restaurant_id = current_restaurant_id()) WITH CHECK (restaurant_id = current_restaurant_id())`. `restaurants` itself uses `id = current_restaurant_id()`. `FORCE` is what makes the policy apply to the schema owner too; without it the backstop is decorative, as spec 0001 records.
- **The API connects as `app_api`**, which owns nothing and holds only table grants. Migrations run as the schema owner over `OWNER_DATABASE_URL`. Both roles already exist per spec 0001 and `api/scripts/init-roles.sql`.
- **Exactly two functions read across restaurants**, both `SECURITY DEFINER`, both with fixed narrow return shapes, both granted `EXECUTE` to `app_api`: `find_staff_for_login(email)` returns only the identity and hash fields login needs, and `resolve_session(token_hash)` returns only the session, staff, restaurant, and role. Neither returns anything else about the staff row, and neither takes a restaurant argument. Every other read in the system goes through row level security.
- **Those two functions are owned by a third role, `auth_lookup`, not by the schema owner.** This is load bearing and easy to get subtly wrong. A `SECURITY DEFINER` function runs as its owner, and `FORCE ROW LEVEL SECURITY` is exactly what removes the owner's exemption, so a function owned by the schema owner would still be filtered by the policy, would find `current_restaurant_id()` NULL, and would return no rows at all. Sign in would never work. Instead `auth_lookup` owns the two functions, owns no table, and holds only `SELECT` on `staff` and `sessions`, and each of those two tables carries one extra policy `FOR SELECT TO auth_lookup USING (true)`. The bypass is then a named policy anyone can list, rather than an invisible consequence of who owns what, and it needs no superuser, which matters because RDS does not hand out `BYPASSRLS`.
- **Roles carried, not enforced here.** `staff.role` is stored; which role may perform which action is feature 7's decision and lives above this layer. This schema makes the role available and nothing more.
- **Compliance scope**: staff names, emails, and password hashes are personal data, so GDPR style erasure applies to them. The schema supports it two ways: blanking the personal columns on a staff row while keeping the row so historical bills stay attributable, and a full cascade delete of a restaurant. Payment records carry a method and an amount only; no card data, no payment provider, nothing in PCI scope, because spec 0001 put no payment provider in the product.
- **Audit log** is required and not negotiable here, because this schema holds money and access control. It records the actor, the entity, and the before and after values for voids, bill closes, price and tax edits, and staff role and deactivation changes.

**Configuration required**

No new environment variables. This feature uses `DATABASE_URL` and `OWNER_DATABASE_URL`, both already read by `infrastructure/config.rs` and both already in `.env.example`. What it does require is two database roles that are not the schema owner: `app_api`, which already exists, and the new `auth_lookup`, which owns the two login functions. Both are created by `api/scripts/init-roles.sql` locally and are a manual step on RDS. Nothing connects as `auth_lookup`; it exists only to own two functions, so it needs no password and no login attribute.

**Critical test scenarios**

- Happy path: open a visit on a table, send a round of two lines, mark each line ready, watch the round flip to `ready`, mark them served, open a bill, assign the lines, close it with a number and correct figures, record a payment, close the visit and see the table free again. Verifies **AC-1**, **AC-7**, **AC-9**, **AC-13**, **AC-15**.
- Round status is a total function: a round whose lines are every one voided ends `voided` and never `ready`; a round with one voided line and one served line ends `served`; a round with one queued line and one ready line stays `queued`. Verifies **AC-7**.
- Empty cases: a visit with no bills closes and frees its table; a bill with no assigned lines is refused at close and consumes no number. Verifies **AC-1**, **AC-6**.
- Isolation, read: with the transaction scoped to restaurant A, every tenant table returns zero of restaurant B's rows, and with nothing scoped, every tenant table returns zero rows at all. Verifies **AC-3**, **AC-4**.
- Isolation, write: scoped to restaurant A, inserting an order line whose `dish_id` belongs to restaurant B is refused by the composite foreign key, and assigning one of A's lines to a bill of B is refused the same way. Verifies **AC-2**, **AC-13**.
- Failure case, occupancy: two concurrent attempts to open a visit on the same table leave exactly one open visit and one unique violation. Verifies **AC-1**.
- Failure case, concurrency: a chef marking a line ready and a waiter voiding the same line concurrently produces one winner, and the loser's conditional update changes zero rows. Verifies **AC-8**.
- Failure case, numbering: two bills closed concurrently in one restaurant receive two different consecutive numbers, and a bill that is voided rather than closed consumes none. Verifies **AC-6**.
- Immutability: close a bill, then change the dish price, rename the dish, archive it, and edit both the tax component and the service charge; re read the bill and every figure and name is unchanged. Verifies **AC-5**, **AC-10**.
- Auth path: connected as `app_api` with no restaurant set, `find_staff_for_login` and `resolve_session` each return the right row, while a direct select on `staff` and on `sessions` returns nothing. This test is the one that would have caught the owner exemption mistake, so it is not optional. Verifies **AC-12**.
- Concurrency, figures: a line reassignment running against a concurrent close of the same bill leaves the closed bill's figures matching exactly the lines it ended up with. Verifies **AC-8**, **AC-13**.
- Deletion: deleting a restaurant row leaves zero rows in every tenant table for that restaurant; deactivating one leaves every row intact. Verifies **AC-11**.
- Audit: a void, a bill close, a price edit, and a role change each write exactly one audit row with the actor and the before and after values. Verifies **AC-14**.

## Build plan

The project builds by Tracer Bullet, and this feature is the deliberate exception to it: the schema is written whole in one migration rather than grown per slice, because a foundation's value is that it does not move, and the reshapes a grown schema needs cost most once real data exists. The thread stays thin where it should: feature 8 uses only the small part of this schema its one dish, one table, one round path needs, and the ordering below still builds inward to outward, so the isolation guarantees are proven before anything is built on them.

1. Add the missing dependencies: `rust_decimal` with its `serde` support, the `rust_decimal` feature on `sqlx`, and the `v7` feature on `uuid`. Postgres 17 has no built in `uuidv7()`, so version 7 identifiers are generated in Rust. Satisfies **AC-9**.
2. Write the migration's first half: the six enum types, then all sixteen tables with their columns, check constraints, `unique (id, restaurant_id)` constraints, and composite foreign keys with `on delete cascade` from the restaurant. Satisfies **AC-1**, **AC-2**, **AC-9**, **AC-11**, **AC-13**, **AC-15**.
3. Write the migration's second half: the indexes listed in the design, the row level security enable, force, and policy per tenant table, and the table grants to `app_api`. Satisfies **AC-3**, **AC-4**, **AC-7**.
4. Add the `auth_lookup` role to `api/scripts/init-roles.sql` beside `app_api`, owning no table and holding only `SELECT` on `staff` and `sessions`. Then add the two security definer functions, `find_staff_for_login` and `resolve_session`, owned by that role and granted `EXECUTE` to `app_api`, plus the one `FOR SELECT TO auth_lookup USING (true)` policy on each of those two tables. Guard the grants the way `0001_bootstrap.sql` guards its own, for a database where the role does not exist yet. Satisfies **AC-12**.
5. Add the bill number allocation as a single `insert ... on conflict (restaurant_id) do update set next_number = next_number + 1 returning`, so a missing counter row creates itself rather than matching nothing. Satisfies **AC-6**.
6. Write the Rust domain types: identifier newtypes beside the existing `RestaurantId`, the six enums mirroring the Postgres enum types through `sqlx::Type`, and the entity structs with their invariants. Satisfies **AC-7**, **AC-8**, **AC-9**.
7. Extend `domain::event::EntityKind` with the seven real entities in the Notifies column, replacing the `Probe` placeholder's role as the only variant, and keep that enum and the entity strings in this spec in step with each other. Satisfies **AC-7**.
8. Write the repository plumbing on `ScopedTx`: the read helpers that apply the archived filter so no caller has to remember it, and the write helpers for each of the eleven scoped operations in the interface surface, every state change expressed as a conditional update naming the state it expects, the round status recomputed by its total function on every line write, and the bill row locked by both operations that touch its figures. Each notifies with the entity string its row gives. Satisfies **AC-7**, **AC-8**, **AC-10**, **AC-13**.
9. Write the bill close path: the snapshot of currency, dish names, prices, tax components, and service charge, the rounding to the restaurant's currency decimals half away from zero, the number allocation, and the two refusals, an assigned line that is neither served nor voided, and a bill with no assigned non voided line at all. Satisfies **AC-5**, **AC-6**, **AC-9**.
10. Write the audit log helper and call it from the void, close, price edit, tax edit, role change, and deactivation paths. Satisfies **AC-14**.
11. Write the integration tests for every critical test scenario, against a real Postgres inside a rolled back transaction, connecting as `app_api` rather than the owner so the row level security tests mean something. Satisfies **AC-1** through **AC-15**.
12. Run `pnpm sqlx:prepare` and commit the refreshed `.sqlx` cache, then confirm `pnpm check` passes. Satisfies **AC-9**.

## Consequences

**Positive**

- Restaurant separation is guarded three times, and the third guard catches the failure the other two structurally cannot: a correctly scoped write that references another restaurant's row. After this migration, that class of bug is a database error rather than a leak.
- A closed bill is a self contained document. Reprints, reports, and tax filings all read the same stored figures, so two surfaces cannot disagree about what a customer paid.
- Eighteen features above this one inherit a settled vocabulary. Nothing from feature 7 onward has to invent an entity, a status value, or a tenancy convention.
- The scope's occupancy rule is a database guarantee rather than a code path, so a second waiter cannot open a duplicate order on an occupied table even through a race.
- Every state change is a conditional update, which makes each transition individually testable and keeps no lock held across a request during service.

**Negative and tradeoffs**

- **Sixteen tables land before a single order has been taken.** This is the largest single piece of design in the project committed furthest ahead of evidence, and some of it will be wrong. The parts most likely to be wrong are the ones the fewest features depend on (audit log shape, section grouping), and the parts most expensive to be wrong about (tenancy, money) are the ones with the most reasoning behind them.
- **Split and merge are permitted but unbuilt.** The schema carries a nullable `bill_id` on order lines and a visit layer that exists mostly to serve them. If no feature ever implements splitting, that is real complexity carried for nothing, and the visit remains justified on its own merits (occupancy and payment genuinely have different lifetimes).
- **The round's stored status can drift** from its lines if any code path ever writes one without the other. It is stored deliberately, for the kitchen queue's sake, and the mitigation is that both writes live in one transaction in one repository function and an invariant test pins the relationship.
- **Two functions bypass row level security.** They are narrow, named, and auditable, and they are still the weakest point in the isolation story. Any future change to either one deserves the same scrutiny as the policies themselves.
- **A third database role, `auth_lookup`, joins `app_api` and the schema owner.** It is the price of making that bypass work at all under `FORCE ROW LEVEL SECURITY`, and it is one more thing to create by hand on RDS and one more thing to get wrong when setting up a new environment. The failure is loud (nobody can sign in) rather than silent, which is the right direction.
- **Every tenant table carries an extra unique index** on `(id, restaurant_id)` purely so composite foreign keys can point at it. That is storage and write cost on every table for a guarantee that only matters when there is a bug.
- **Email is unique platform wide**, so one person working at two restaurants on the platform needs two email addresses, and an owner with two restaurants needs two accounts. This is a real product limitation accepted to keep sign in to an email and a password with nothing extra to type.
- **Gapless bill numbering serialises closes** within a restaurant, because the counter row is locked for the duration. Two tills closing bills at the same second will briefly queue. Irrelevant at one restaurant's volume, and it is a genuine serialisation point to remember if it ever is not.
- **Feature 7 has no spec yet**, and this schema fixes the shape of `staff` and `sessions` that feature 7 must build on. If feature 7's design wants something materially different, this migration changes before it has data in it, which is cheap now and will not be later.

**Neutral**

- Postgres enum types make adding a value a one line non blocking `ALTER TYPE`, and make removing or reordering one genuinely awkward. That trade was chosen on the grounds that none of these six lists is expected to shrink.
- No reporting rollup tables exist. Feature 18 queries the real tables through the indexes above, and adds rollups only if a real restaurant is measurably slow.
- The schema does not copy the restaurant's own name, address, or tax registration number onto a bill, so a bill reprinted after the restaurant moves or re registers prints the current details, not the ones in force at the time. Acceptable now, and a follow up if a tax authority disagrees.
- `sqlx` needs a live database or the committed `.sqlx` cache at build time, so this migration must be applied locally before the query macros in step 8 will compile.

## Follow-up

- [ ] Connect a Postgres MCP server once this migration has run locally, so agents read the live schema rather than trusting the migration file. Carried over from spec 0001; it is a user configuration step no skill can perform. It becomes actionable the moment step 3 of the build plan lands.
- [ ] Feature 7 (accounts, restaurants, and roles) owns registration, sign in, password reset, and which role may do what. This spec fixes only the shape of `staff`, `sessions`, and the two bootstrap lookups underneath it. Design feature 7 before building on those tables, so any change to them happens while they are still empty.
- [ ] Decide whether a bill should snapshot the restaurant's name, address, and tax registration number. Deliberately excluded here; revisit when a real restaurant's tax rules are known.
- [ ] Split and merge are permitted by the schema and implemented by nothing. If they are wanted, they need their own scope row rather than being smuggled into feature 14 or 15.
- [ ] The audit log has no retention policy, by choice, because it only records consequential changes. Revisit if it ever grows faster than the bills do.
- [ ] `AGENTS.md` and `api/AGENTS.md` both say `0001_bootstrap.sql` creates no tables because feature 4 owns the schema. Both lines stop being true when this migration lands, and `/sync` owns updating them.
- [ ] Spec `0002-coding-standards-and-tooling` has a `verify.md` but no `index.md`. Not this feature's business, and worth a look before someone tries to link it.
