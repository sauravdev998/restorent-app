# 0012. Kitchen display: rationale

The reasoning behind [index.md](index.md). Not read during a build.

## Context

> ⚠️ Premise note: the Ready area this spec adds is a second alarm for a plate that feature 12
> already alarms about. The waiter gets a chime, a badge, a grouped alert and a two minute reminder
> with an acknowledge, all built to be reliable. Putting a reddening age on the same plate in the
> kitchen means two screens nagging about one failure, and the kitchen cannot resolve it: a chef's
> only remedy is to shout at somebody. Alert duplication does not double attention, it halves the
> credibility of both alerts. The right framing is that the kitchen's Ready area is a window, not a
> siren: it exists so plated food is visible to the people standing next to it, and its red state
> means chase the waiter, not raise a second alarm. If the two ever compete, the kitchen's ageing is
> the one to soften, because the waiter's is the one that reaches somebody who can act.

The kitchen pass already works. Spec 0007 built it as part of the thin order thread and said in
writing that it was deliberately crude and would be visibly rebuilt by this feature. Spec 0011
touched it again, but only far enough to stop long notes being clipped and cancelled dishes
vanishing mid cook, and recorded both as stopgaps for this feature to replace properly. So the
problem is not a missing screen. It is a screen that was built to prove a pipe and is now the thing
a restaurant lives in for five hours an evening.

Four specs left debts that all land here, which is the real reason this feature exists as one piece
of work rather than several. Spec 0004 built `ElapsedTime` with a threshold prop and a placeholder
default, and wrote down that the real per restaurant value belongs to this feature. Spec 0007
repeated that, and separately noted that the kitchen read has no limit and should be given one.
Spec 0005 asked this feature to confirm the type scale still reads at three metres. Spec 0011 left
the cancelled dish presentation and the question of whether a chef may void at all. Settling them
one at a time would mean touching the same screen four more times.

The forces that actually shaped the design are about where this screen lives rather than what it
shows. It is the only surface in the product nobody holds. A waiter's phone is in a hand or an
apron and gets glanced at constantly; a kitchen tablet is bolted to a wall, read from three metres,
touched with wet or gloved hands, and left alone for an hour at a time. Three consequences follow
and all three are load bearing. A tablet left alone goes to sleep and shows nothing. A browser that
has had no interaction refuses to play a sound, so the alert the kitchen bought the tablet for never
fires. And a screen that has stopped receiving looks exactly like a screen with no orders, which is
the failure this feature most has to prevent: on a phone a dead stream is noticed in seconds, on a
kitchen wall it is noticed when a table complains.

Two constraints make the work cheaper than it would otherwise be. Nothing is deployed yet, and the
API and the web app ship from one repository in one commit, so a contract change is coordinated by
the compiler rather than by a rollout plan. And the mechanisms this feature needs already exist:
scoped transactions, the event stream with its invalidate never write rule, the kitchen density
token layer, the audio unlock module, and the server clock correction. This feature adds no new
mechanism. It adds three columns and uses what is there.

## Options considered

### Option 1: Thicken the existing pass in place

Keep the route, the component and the endpoints, and grow them. The Ready area, the second urgency
level, the undo, the whole ticket clear, the chef void and the four reliability behaviours all land
on the screen that exists, and the only schema change is three columns on the restaurant.

**Pros**:

- The existing screen is small, well documented and correct. Its hard parts, the server clock
  correction, the snapshot read, the one tap one dish rule, are the parts worth keeping, and they
  survive untouched.
- Every debt closes in one visit to one file set, which is exactly what the four earlier specs asked
  for.
- No cutover, no parallel screens, no flag, no window where a chef could be on either version.

**Cons**:

- A lot lands on one surface at once. Seven visible behaviours arrive together and there is no
  intermediate state where half of them are proven in a real kitchen before the rest are built.
- The component grows well past what it is today and will want splitting during the build, which is
  refactoring mixed into feature work.

### Option 2: A new kitchen surface beside the old one, cut over when proven

Build a second route with the real display, leave the current pass reachable, run both, and retire
the old one once the new one has survived a service. The strangler pattern.

**Pros**:

- A real kitchen could try the new screen on a quiet night with the old one one tap away, which is
  the kind of safety net a screen this operationally exposed deserves.
- Each behaviour could land on the new screen without any risk to the working one.

**Cons**:

- There is nothing to strangle. The old screen is roughly 190 lines and shares its endpoints and its
  data with the new one, so the two would be two views over one model, not an old system and a new
  one.
- Nothing is deployed and there is no production kitchen to protect. The safety net guards a risk
  that does not exist yet, and costs a duplicate screen, duplicate tests and a retirement task.
- Two kitchen routes in the router is exactly the sort of thing that survives longer than intended.

### Option 3: Settle only the recorded debts, leave the screen as it is

Do the four things the earlier specs actually asked for: the real threshold, the read cap, the
cancelled presentation, the three metre check. Leave the Ready area, the undo, the chime, the
banner and the wake lock out.

**Pros**:

- Much smaller, and every line of it is work somebody already wrote down as owed.
- Closes the specs' follow ups honestly without inventing new scope from a scope row's one line.

**Cons**:

- It does not satisfy the scope's own done when clause, which asks for a screen legible and usable
  in a working kitchen, not a screen with a correct threshold.
- It leaves the three failure modes that come from where the tablet lives, the sleeping screen, the
  silent chime and the indistinguishable dead stream, entirely unaddressed. Those are the ones that
  lose food.
- The feature would have to be reopened almost immediately, and the screen touched a sixth time.

## Rationale

Option 2 fails on its own terms. The strangler pattern earns its cost when there is a working
production system whose behaviour nobody fully understands and whose users cannot be interrupted.
Here the old screen is a few hundred lines written six weeks ago by this same project, with its
reasoning in its own doc comments, sharing every endpoint with the replacement, and there are no
users to interrupt because nothing is deployed. Running two kitchen routes to protect a risk that
does not exist is the cost without the benefit.

Option 3 is the tempting one and it is the one to argue against most carefully, because doing only
what was written down is usually right. It fails on the force that runs through the whole Context:
this screen's failure modes come from where the tablet is, not from what it displays. A correct
threshold on a screen that has gone dark, or gone silent, or quietly stopped receiving, is a correct
number nobody sees. The three reliability behaviours are not scope invented from a one line scope
row; they are the difference between a display and a display that works in the room it is bolted
to. Fixing the recorded debts and leaving those out would mean reopening this feature within a
service or two.

So Option 1, with the premise note above as a standing caution. The specific calls inside it follow
the same force. The Ready area exists because plated food is invisible the instant a ticket vanishes
and the only thing watching it is a phone in an apron, and its ageing reuses `order_rounds.ready_at`
and the same two thresholds because a third number is a third thing for an admin to get wrong. Undo
is allowed for as long as the dish is ready, rather than for a few seconds after the tap, because
the tap most worth taking back is the last one on a ticket, and a seconds long window is exactly the
window a chef carrying a plate does not have. It needs no rule about the round on top of that: a
round cannot be served while any line is still ready, so the dish's own status is the whole
condition. The chef's void reuses the existing void path rather than getting
its own kitchen endpoint, because that path already recomputes the bill, writes the audit row and
fans out the event, and a second copy of that sequence is a second place for the three of them to
drift apart. The thresholds ride on the kitchen response rather than the cached identity bundle so
that an age and the thresholds it is judged against always come from one answer built at one
moment, which is the same reasoning that put `serverTime` there in the first place.

The two choices that go slightly beyond the feature are deliberate and both are argued in
Consequences. `restaurants` gets its `version` column here because this is the first feature to give
that row optimistic concurrency, and the project's own rule says an editable row carries one. The
row has in fact been editable since spec 0003, through `PATCH /api/restaurant`, with last write
wins, so this is not a new capability arriving but an old gap closing, and the check falls on the
five settings that endpoint already writes as well as the two new ones. Leaving it for feature 14
means adding optimistic concurrency at the same time as currency and tax, which is the worse moment.
And the warning below late rule is a check constraint as well as a Rust check because amber after
red is a state the screen cannot render: the Rust check is what names the offending field to an
admin, and the constraint is the form of the rule a future handler cannot forget.
