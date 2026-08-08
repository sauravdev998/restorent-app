# 0003. Core data model: reasoning and options

The decision itself, and everything a build needs, is in [index.md](index.md). This file is the record of why.

## Context

> ⚠️ Premise note: splitting and merging bills was added during this design conversation and is not asked for by any feature in the scope. Feature 11 says a table cannot hold two open bills at once and feature 12 says one bill stays open per table across the meal, which is the opposite shape. Supporting split and merge is what forces the visit entity and the nullable `bill_id` on order lines, roughly a third of the difficulty in this schema. The right framing: keep the visit, because it is genuinely the thing that occupies a table and it makes moving a party trivial, but treat split and merge as capability the schema permits rather than work to build. No feature should implement them until one asks for them, and the acceptance criteria below only require that the schema does not prevent them.

Feature 4 defines the tables every other feature reads and writes. Eighteen features are queued behind it and the scope names it the most expensive thing in the project to get wrong. Nothing in the database exists yet: `0001_bootstrap.sql` deliberately created two functions and no tables, so this is a blank sheet with the conventions already fixed.

Four forces shape it.

**Restaurant separation is the named central risk.** Many restaurants share one Postgres instance. Spec 0001 already fixed two locks on this: a scoped transaction that sets `app.restaurant_id`, and row level security policies reading it back through `current_restaurant_id()`. Both of those guard reads and writes against a forgotten filter. Neither guards a write that carries a legitimately scoped restaurant id but references a row belonging to somebody else, which is what a bug in a handler actually produces.

**Money must stay true after the fact.** A restaurant edits its menu and its tax settings while old bills exist. Feature 14 requires a closed bill to keep the rates it was charged at, feature 16 requires it to reprint correctly, and feature 18 requires reports that agree with the bills. Restaurants may be in different countries, so currency, decimal places, tax structure, and the local day all vary per restaurant.

**The service loop is concurrent by nature.** A chef and a waiter act on the same order at the same time on different devices, and both screens are driven by live events. Every state change has to be safe when two people race it, and it has to be cheap to read, because the kitchen screen queries the queue constantly.

**Identity has a bootstrap problem.** Row level security hides every row until the restaurant is known, but signing in and resolving a session token are both lookups that happen before anyone knows which restaurant is involved. Something has to be allowed to read across restaurants, and whatever that is becomes the weakest point in the whole isolation story.

The consequence of not deciding is that every later feature invents its own answer and the first two of them disagree.

## Options considered

### Option 1: Visit centred schema with composite tenant keys

A visit is what occupies a table. Rounds hang off the visit, lines hang off rounds, and bills are payment documents that lines are assigned to. Every tenant table carries `restaurant_id`, a unique constraint on `(id, restaurant_id)`, and composite foreign keys pointing at both columns, so a cross restaurant reference is refused by the database. Row level security with `FORCE` on every tenant table, and two narrow security definer functions for the login and session lookups.

**Pros**:
- Three independent locks on the central risk, and the third one (composite keys) catches the failure the other two miss, a scoped write referencing another restaurant's row.
- One open visit per table is a partial unique index, so the scope's occupancy rule is a database guarantee rather than a code path someone remembers.
- Move, split, and merge all become data operations on existing columns rather than schema changes.
- Bills are self contained snapshots, so reprints and reports stay correct forever regardless of what the restaurant edits.

**Cons**:
- Sixteen tables and six enum types before a single order has been taken, which is a lot of surface to get right in one migration.
- Every tenant table pays for an extra unique index it would not otherwise need, and every foreign key declaration is wordier.
- The visit adds a layer between the table and the bill that every waiter query has to traverse, and it is one more concept for a new reader to hold.
- The security definer functions are a real bypass. They are narrow and auditable, and they are still a bypass.

### Option 2: Bill centred schema, no visit entity

A bill is what occupies a table, exactly as the scope describes. Rounds hang off the bill. A partial unique index keeps one bill open per table.

**Pros**:
- Fewer tables, fewer joins, and a model that reads exactly like the scope text, so nobody has to learn a concept the product language does not use.
- The waiter's main query, what is open on this table, is one row.

**Cons**:
- Splitting means re pointing rounds at new bills, which is a data migration in miniature performed live during service.
- Merging two tables has no clean expression at all.
- Moving a party to another table changes the bill's table, which is fine, but the bill then carries occupancy and payment meaning at once, and those two things stop and start at different moments.

### Option 3: Thin thread schema, grown per slice

Model only what feature 8's one dish, one table, one round path needs, and add tables as each slice arrives.

**Pros**:
- Truest to the project's Tracer Bullet approach, and the fastest route to a working thread.
- Nothing is designed for a feature that might change before it is built.

**Cons**:
- Every later slice reshapes tables that already hold data, and the reshapes that hurt most (adding `restaurant_id` to something, splitting an entity in two) are exactly the ones a growing schema tends to need.
- The tenant isolation pattern would be established on three tables and then copied by hand onto thirteen more, which is how one table ends up without a policy.
- Feature 4's own acceptance in the scope asks for a schema that survives later slices without a breaking change, which this option explicitly does not attempt.

### Option 4: One Postgres schema per restaurant

Each restaurant gets its own set of tables in its own namespace, so isolation is physical rather than policy based.

**Pros**:
- The strongest isolation available short of separate databases. A missing filter cannot leak anything, because the other restaurant's rows are not reachable from the connection.
- Per restaurant backup and restore becomes trivial.

**Cons**:
- Every migration has to run once per restaurant, which turns a schema change into a job with partial failure states, and spec 0001 has no background job machinery.
- Cross restaurant queries, which the platform will eventually want for its own operations, become unions over N schemas.
- Connection pooling gets much harder, and Postgres slows down noticeably at a few thousand schemas.
- Enormous operational cost for a pre launch product with no customers yet.

## Rationale

Option 1 is chosen because the central force in Context is a risk that the two existing locks do not fully cover. A scoped transaction and a row level security policy both check that a row belongs to the current restaurant. Neither checks that a row this restaurant is allowed to write points at another row this restaurant is allowed to reference. A handler bug that takes a dish id from the wrong place produces exactly that, and with plain foreign keys the database accepts it. The composite key costs one extra unique index per table and closes the gap permanently, which is the right trade for the risk the scope itself names as the most expensive to get wrong.

The visit exists because the engineer asked for split and merge, and because occupancy and payment genuinely are two different lifetimes. A party sits down and leaves once; it may be billed once, three times, or jointly with the next table. Option 2 collapses those into one row and pays for it at exactly the wrong moment, live during service, with a data reshuffle. Given the visit exists, one open visit per table preserves the scope's invariant intact rather than dropping it, which Option 2 could not have done alongside split.

Option 3 was rejected against the project's own build approach, deliberately. Tracer Bullet is the right default for the features above this one, and it is the wrong default for the layer everything else stands on. The whole value of a foundation is that it does not move, and the reshapes Option 3 invites are the ones that cost most once real data exists. The compromise is that the migration is written whole and the thin thread in feature 8 uses only the part of it that it needs, so feature 8 is still a thin end to end thread even though the schema under it is complete.

Option 4 solves a problem this product does not have yet at a price it cannot pay yet. It stays available: nothing in this design prevents moving a large customer to their own schema later.

Two of the standard rules for this kind of design are knowingly broken, and both are worth naming. First, derived values are normally not stored. The round's status is a derived value and it is stored, because the kitchen queue reads it on every poll and every event, and because feature 13's requirement that a ticket flips to ready by itself is much easier to test as an explicit write than as an aggregate that happens to change. The drift risk is real and is mitigated by writing both in one transaction and by an invariant test. The bill's totals are a different case and not really an exception at all: they are a historical snapshot of what a customer paid, not a cache of a live computation, and recomputing them at read time is precisely the bug feature 14 asks to avoid.

Second, the login and session lookups bypass row level security. There is no way around this, since the restaurant is unknown until the lookup completes. Confining the bypass to two named functions with fixed return shapes, callable by the API role, is better than the alternative of leaving the staff and session tables unprotected, which would put the most sensitive table in the system outside the backstop entirely.

The mechanism of that bypass took two attempts and is worth recording, because the first version was wrong in a way that looks right. Owning the functions with the schema owner does nothing, because `FORCE ROW LEVEL SECURITY` is precisely what strips the owner's exemption, and a `SECURITY DEFINER` function runs as its owner. Such a function would be filtered by the very policy it was meant to sidestep, would see a NULL restaurant, and would return no rows, so nobody could ever sign in. The working shape is a third role that owns no table, owns the two functions, and is named in one extra policy on each of the two tables it reads. That also turns out to be the better design on its own merits: the bypass appears in `pg_policies` where anyone auditing the schema will see it, instead of being an invisible consequence of who owns what. It needs no superuser either, which matters because RDS does not grant `BYPASSRLS`.
