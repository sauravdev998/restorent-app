# 0007 rationale: the thin order thread

The reasoning behind [index.md](index.md). A build does not need this file.

## Context

Every foundation this platform stands on is now built, and none of it has ever carried a real
order. Spec 0003 shipped the schema and eleven scoped repository operations covering the whole
path from a party sitting down to a paid bill, and its integration tests drive that path directly
against Postgres. Spec 0001 shipped the live pipeline end to end: a change calls
`notify_entity_change`, Postgres publishes it, one listen connection per process picks it up, and
`GET /api/events` pushes it to whichever screens belong to that restaurant. Spec 0006 shipped sign
in, sessions resolved on every request, and a role requirement carried in each handler's own type.
Spec 0004 shipped the components this thread draws with, including the status pills, the ticking
elapsed time, the alert, the live region, and a synthesised ready chime.

What has never happened is any of it working together in a browser. No HTTP endpoint touches a
visit, a round, a line, or a bill. The waiter screen and the kitchen screen are both a heading and
a placeholder sentence. The live stream has carried exactly one kind of message in its life, a
`probe` from a development only endpoint that exists to prove the pipe with no data behind it. So
the project's central claim, that a dish sent on a phone appears on a kitchen screen across the
room within a second or two with nobody refreshing anything, rests entirely on the parts having
been built correctly and never on having been seen to work.

That is the risk this slice exists to retire, and it is the reason the scope calls it the thin
order thread rather than the waiter feature or the kitchen feature. The build approach on record
is Tracer Bullet: one narrow path pierces every layer and works for real, then later slices
thicken one segment at a time. Feature 12 owns the waiter's real working screen, feature 13 the
kitchen display, features 9 through 11 the menu, staff, and tables an actual restaurant would set
up, and features 14 and 15 the money. Every one of them thickens a segment of this thread. If the
thread is wrong, all of them are built on it.

Two forces shape what this slice can and cannot do. First, none of the administration exists yet:
there is no way to create a dish, a table, or a staff member other than registering a restaurant
as its admin, so the thread has no rows to run on unless the slice provides them. Second, the
schema is already fixed and already right, which means the slice's honest job is to reach the
operations that exist rather than to add anything to them. A slice that finds itself wanting a new
column has almost certainly wandered outside its own scope.

## Options considered

### Option 1: A real thread over the operations that exist, no schema change

Add the presentation layer and the two screens on top of spec 0003's repository, seed the rows the
thread needs into development only, and prove the two device claim with one browser test. Nothing
new in the database, nothing new in the stack.

**Pros**:

- Exercises the whole pipe exactly as production will: real sign in, real row level security, real
  `NOTIFY`, real stream, real browser.
- The scariest claim in the project gets a repeatable automatic check in the same slice that makes
  the claim.
- Every part it adds is a part features 9 through 15 keep and thicken, so almost nothing is
  thrown away.

**Cons**:

- It is the widest of the three: two screens, roughly ten endpoints, a seed rewrite, and a browser
  test harness that does not exist yet.
- It ships screens that are deliberately crude and will be visibly rebuilt in slices 2 and 3,
  which can read as wasted work to somebody watching the app rather than the risk register.

### Option 2: Screens first on placeholder data, wire the backend later

Build the waiter and kitchen screens against fixtures, agree the shapes, then connect them to real
endpoints in a later slice.

**Pros**:

- The screens land quickly and can be looked at and argued about early.
- The interface work is not blocked on any endpoint being finished.

**Cons**:

- It proves nothing about the one thing in doubt. A kitchen screen that updates from a fixture is
  a slideshow, and every genuine risk here (does the event reach the browser, does row level
  security hold with a real session, does a concurrent tap behave) stays untouched.
- It is the Facade approach, and the project's recorded approach is Tracer Bullet for exactly this
  reason.

### Option 3: Endpoints and an integration test, screens deferred to slice 3

Ship the HTTP surface over the repository, cover it with API level tests, and let features 12 and
13 build the first real screens.

**Pros**:

- Cheapest slice by a distance, and it leaves the interface work to the features that own it.
- The endpoints are still a real, checkable increment.

**Cons**:

- The claim that survives untested is precisely the one nobody can reason their way to: that a
  browser holding a stream open sees a change appear with no refresh, on a second device, through
  a proxy, with a session cookie. An API test cannot show that.
- It leaves the waiter and kitchen screens as placeholders until slice 3, so the first time
  anybody sees the product work is very late.

## Rationale

Option 1, because the whole point of a tracer bullet is that it lands somewhere you can see. The
context above says every layer is built and none of it has been observed working together, so the
value of this slice is almost entirely in the observation, not in the code. Options 2 and 3 each
delete one end of the thread, and in both cases the end they delete is the end where the doubt
lives. Option 3 in particular is tempting because it is cheap and its tests look rigorous, but an
integration test against Postgres proves the repository, which spec 0003 already proved, and says
nothing about the browser.

The narrowness is bought elsewhere, deliberately. Voiding a dish, per line notes, splitting a bill,
recording a payment, and every administration screen stay out, and each of them belongs to a
feature that already exists in the scope. What stays in is only what the sentence "a waiter sends a
dish, the kitchen sees it, the chef marks it done, the waiter is alerted, the bill closes" needs to
be literally true. That gives a slice which is wide across layers and very thin within each one,
which is the shape the approach asks for.

Two smaller calls deserve their reasoning recorded, because both look like scope creep and neither
is.

The first is that the seed grows to create the tables, the menu, and the waiter and chef accounts.
It reads like feature work borrowed from slices 2 and 3, and it is not: those features build the
screens that let an admin manage those things, which is entirely different work. What the seed
needs is rows, and it already creates rows through the real registration path rather than by
inserting them, which is the property worth keeping. The alternative, a development only endpoint
that creates staff, is a second way into the account system that has to be maintained and then
found and deleted, and this project has just finished deleting the last two development
placeholders in feature 7.

The second is the conflict code refactor. Today `DomainError::Conflict` carries a free English
string and the whole family collapses to one wire code, `conflict`, which the web maps to one
generic translated sentence. That was fine while the only conflicts were a taken email address and
a session token collision, neither of which a person acts on. This thread has at least five that a
person must act on differently: the table was just taken, the dish was already marked, the food is
still on the pass, the bill is already closed, the party has already left. Telling a waiter
"conflict" while the real answer is "two dishes are still cooking" is the kind of thing that gets
worked around later with an English message rendered on a Hindi screen, which spec 0005 exists to
prevent. Doing it now costs about fifteen call sites; doing it after features 12 and 15 have added
their own refusals costs far more, and by then the generic sentence will have been on screen long
enough to feel normal.

One risk is accepted rather than solved. Sending a round is not idempotent: if the network eats the
response, the waiter's second tap creates a second ticket. The proper fix is an idempotency key,
which is a column, and this slice takes no migration. The mitigation for slice 1 is that the send
is single flight in the browser and the basket clears only on success, and the real fix is enrolled
against feature 12, which owns the waiter's working screen and will need the column anyway once the
same phone is used all evening on patchy restaurant wifi.
