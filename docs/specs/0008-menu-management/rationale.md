# 0008. Menu management: rationale

The decision record behind [index.md](index.md). `/develop` does not read this file.

## Context

Slice 1 proved the order thread on a seeded menu: two categories and six dishes written by
`pnpm db:seed`, in development only. A real restaurant has no way to put its own menu into the
platform, so the thread works for nobody but a developer. Feature 9 is the first half of slice 2's
promise that a restaurant's own setup drives the thread.

Most of the ground is laid. Spec 0003 built `menu_categories` and `dishes` with positions, an
`is_available` flag, soft removal through `archived_at`, and composite tenant keys, and `catalog.rs`
already has create, edit (audited), and archive operations for dishes plus create and archive for
categories. What is missing is everything a person touches: no admin endpoint writes the menu, and
the admin surface is an empty landing screen. Three gaps in the existing operations also matter once
people use them for real: `update_dish` writes availability as part of a whole dish edit, so an admin
saving a form overwrites a chef's switch; archiving a category neither checks for live dishes nor
notifies; and nothing stops two live dishes sharing a name, which the kitchen ticket and the bill
cannot tell apart.

The forces are the busy night and the people in it. The one who first knows a dish ran out is the
chef at the pass, not the admin at a desk. The waiter has a basket half built on a phone when that
happens. Several people can touch one dish in the same minute, and every screen is live over one
event stream per browser, invalidated by entity kind (spec 0007). Spec 0004 sets an accessibility
baseline every new interaction must meet, including drag and drop, and holds colour back for order
status. Spec 0005 requires every word to come from a translation file, in English and Hindi.

The scope row and the built code disagree on one visible behaviour. The row says an unavailable dish
is removed from the waiter's screen; spec 0007's AC-3 and its code keep it visible, greyed, and
impossible to add. The engineer confirmed the built behaviour.

## Options considered

### Option 1: a live, directly edited menu over the existing schema (chosen)

Admin endpoints and one admin screen edit the live menu directly; the chef switches availability from
a kitchen tab; a small migration adds the diet marker, a version per row, and case insensitive live
name uniqueness; dnd-kit drives drag reordering.

**Pros**:
- Reuses spec 0003's tables and most of `catalog.rs`; one small migration.
- Every change reaches waiters through the event path that already exists.
- The version column and the separate switch close the overwrite bug without locks held across a
  request.

**Cons**:
- No draft: a big rework happens live, one save at a time, during service if the admin chooses.
- Drag and drop adds a library and the heaviest accessibility work in the feature.

### Option 2: the simplest admin CRUD

Admin only create, edit, and archive on today's operations, up and down buttons for order, last save
wins, and no chef access.

**Pros**:
- The least code and no new dependency; buttons meet the accessibility baseline for free.

**Cons**:
- The chef has to find an admin to switch off a dish mid service, the exact case the feature exists
  for.
- An admin saving an edit form silently turns a dish back on that was just switched off.
- Two live dishes can share a name on a kitchen ticket.

### Option 3: a draft menu with publish

The admin edits a draft copy and publishes it as a whole; waiters always read the published copy.
Availability stays live on the published copy, outside the draft.

**Pros**:
- A large rework lands at once, can be previewed, and never shows half done mid service.
- Publishing is a natural point to validate the whole menu.

**Cons**:
- Two copies of every category and dish, and a merge rule for availability, which must stay live and
  so cannot live only in the draft.
- Order lines reference dish ids; publishing must keep ids stable across copies, which is the hard
  part of every draft and publish design.
- A few hundred rows edited a few times a week do not need a release process.

## Rationale

The feature exists for the minute the kitchen runs out, which rules out Option 2: without the chef's
switch and without protecting that switch from a stale edit form, the menu is wrong on exactly the
night it matters. Option 3 solves a problem this restaurant does not have yet (large coordinated menu
releases) and pays for it with a second copy of the menu and an availability merge rule; the menu is
small and changes incrementally, so live edits with stale edit refusal are enough. Option 1 keeps
spec 0003's single source of truth and spec 0007's live event path, and adds only what the busy night
demands: availability as its own action, a version to refuse stale writes, name uniqueness the kitchen
can rely on, and an archive rule that cannot race.

Two engineer choices shaped the cost. Drag and drop was chosen over buttons, the recommended pick,
for how it feels on a desk; it is achievable within the baseline because dnd-kit ships keyboard
sensors and announcement hooks, and keeping drags within one category (moving is done in the form)
keeps the keyboard path to a single list. The standard coloured diet marks were chosen over a neutral
badge because diners and waiters look for the symbol; the colour rule in `docs/design.md` gets one
written exception, and each mark still carries its own shape and name so meaning never rests on
colour alone.

Case insensitive uniqueness across the whole live menu, rather than per category, follows from the
ticket and the bill showing a name and nothing else: two dishes the kitchen cannot tell apart are a
wrong plate waiting to happen.
