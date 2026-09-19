# 0011. Waiter service flow: rationale

The decision record behind [index.md](index.md). `/develop` builds from the index and skips this
file.

## Context

Feature 8 built the waiter's screens thin on purpose. They prove the thread works: a floor of table
cards, one table screen holding the basket, the rounds and the bill, and a ready alert that only
fires on the table screen the waiter already has open. On a real evening a waiter has six tables
and is standing at none of them when the kitchen finishes a dish. The alert they need goes off on
exactly the screen they are not looking at.

Reading the code as it stands shows the gaps. The note on each order line already goes through the
API and appears on the kitchen ticket, but the waiter screen has no way to type one, and the kitchen
cuts it off with an ellipsis. The ready alert lives in one component's memory, so a reload or
another screen loses it. Serving works only for a whole round, so a starter that is ready early
waits for the main course. A dish sent by mistake cannot be cancelled, even though `void_line`
exists in the repository, so it blocks the close. Sending a round is not idempotent (safe to
retry), so a timed out send on restaurant Wi Fi that the waiter taps again makes a second ticket.
Spec 0007 listed that as a follow up for this feature. And because opening a table always opens a
bill, and closing refuses a bill with no dishes, a table opened by mistake can never be freed.

The forces pulling on this are the phone and the room. The device is a phone, often locked, often
muted, on patchy Wi Fi, in a loud room, held by someone walking. Several waiters share one floor
and hand tables over at shift changes, and the schema records who opened a table but not who is
looking after it now. Anything added has to run on the stack already chosen: server sent events
fanned out through Postgres `NOTIFY`, TanStack Query invalidation through the written fan out map,
and no new infrastructure that the team would have to run.

Leaving it undecided means the waiter's screen, the one used most in the whole product, stays a
demo. Features 13 and 15 would each have to invent part of it: the kitchen would need to know what a
cancelled dish looks like, and bill closing would need voids that nobody can create.

## Options considered

### Option 1: Thicken the existing screens in place, with ready state derived from the server

Keep the floor and table routes and the send, serve and close endpoints. Add an Orders view beside
the floor, driven by one new read of every open visit that is mounted once in the waiter shell.
The ready alert, the badges and the reminders are all worked out from that read (a line whose
status is `ready`), so nothing about an alert is stored and nothing is lost on reload. Add the
responsible waiter, per dish serving, voiding, moving, notes, and an idempotency key, using one
migration and the repository operations that already exist wherever they fit.

**Pros**:
- No new infrastructure. The alert rides the live stream and the query cache that already work.
- "Is anything ready?" has one answer, the database's, so a badge cannot disagree with the kitchen.
- Reuses `void_line`, `mark_line_served`, `move_visit` and `send_round`; the migration only adds
  columns and rewrites one check.

**Cons**:
- A locked phone hears nothing. The alert only works while the app is open in a browser tab.
- The Orders read returns every open visit's lines on every relevant event, which is more data per
  refetch than the floor alone.

### Option 2: Web Push notifications for ready food

Everything in Option 1, plus a service worker, VAPID (the keys a server uses to sign push messages)
key pairs in config, a push subscriptions table, and a sender that pushes to the responsible
waiter's devices when a line turns ready.

**Pros**:
- Reaches a locked phone, which is the case Option 1 cannot cover.

**Cons**:
- New moving parts: a service worker lifecycle, subscription expiry and cleanup, a new secret, and a
  sending path outside the request.
- On iOS it works only for a site installed to the home screen, so part of the floor gets it and
  part does not, which is worse than nobody getting it, because nobody can rely on it.
- A second source of "ready" beside the live stream, which can disagree with it.

### Option 3: Rebuild the waiter surface as a new route tree beside the old one

Build a new waiter app under new routes, run it beside the thin one, and switch over when it is
done (the strangler pattern, growing the new beside the old and retiring the old).

**Pros**:
- The thin screens stay untouched as a fallback while the new ones are built.

**Cons**:
- The thin screens are two components with tests, not a production system with users. Running two
  copies costs more than it protects.
- The two device Playwright scenario and the seed would have to cover both surfaces for a while.

## Rationale

Option 1, because the need is "tell the right person while they are using the app", and the app
already has a live stream that delivers every line change within a second or two. Working out the
alert from server state rather than storing it is what makes it survive a reload, a second screen
and a handover with no new table: the ready badge is simply "lines in `ready` on this visit", and
the only thing kept on the phone is which of those this phone has already chimed for. The engineer
chose this over Web Push knowing that a locked phone stays silent. The two minute reminder is there
so that the moment the waiter unlocks and glances at the phone, the unserved food is at the top of
the Orders list and chimes again soon.

Option 3 is the right instinct for a live system and the wrong one here. The waiter screens were
written to be replaced, have no users, and are small. Changing them in place, one step at a time
under Tracer Bullet, gives the same safety with half the surface.

Within Option 1 the calls that were mine to make:

- **Responsible waiter as a column on `visits`**, filled from the opener, rather than a handover
  table. The audit log already records every take over with who and when, so a second history
  table would duplicate it. Runner up: `visit_assignments`, if handover history ever needs to be
  queried rather than just read.
- **Take over names who it expects to take over from**, and the update is conditional on it. This is
  the same "name the state you expect" rule every other write in the project follows (spec 0003,
  invariant 6). Last write wins would silently move a colleague's alerts to someone else.
- **The idempotency key is required on every send, not optional.** The web app is the only client
  and is generated from the same OpenAPI document, so there is no older client to protect. An
  optional key would be a defence nobody switches on. The key is checked under the visit row lock
  that `send_round` already takes, so two identical sends in flight at the same moment line up
  behind each other and the second finds the first.
- **A replay returns `200` with the first ticket** rather than `409`. The phone's only question
  after a timeout is "did it go?", and the answer that needs no second request is the ticket.
- **Voiding gets its own conflict code, `line_not_voidable`**, instead of reusing `line_not_queued`.
  A ready line can be voided, so "not queued" would be wrong for exactly the case where voiding
  fails (the line was already served).
- **Voiding recomputes the open bill's subtotal in the same transaction.** `void_line` does not
  touch the bill today, and `bills.subtotal` must stay true at every moment of the meal (spec 0007,
  invariant 4).
- **An empty bill is voided on close, not left open.** A bill with nothing chargeable takes no
  number (spec 0003 keeps numbers gapless), so voiding it and then closing the visit is the one path
  that frees the table without inventing a zero value bill.
- **The chime rings at once and later lines join the visible alert silently for three seconds.**
  Waiting three seconds before the first chime would make every alert late in order to group a few.
- **Notes are counted in characters the way Postgres counts them** (Unicode code points), on both
  sides, so a Hindi note that the phone accepts is never refused by the database.
- **The floor's `foodReady` flag becomes `readyDishCount`.** The generated client changes in the same
  commit and the web is the only consumer, so a count replaces a boolean rather than sitting beside it.
