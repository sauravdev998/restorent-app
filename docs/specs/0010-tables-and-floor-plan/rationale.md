# 0010. Tables and floor plan: rationale

The decision record behind [index.md](index.md). `/develop` builds from the index and skips this file.

## Context

Every service on the platform starts with a table. Spec 0003 designed the tables for it
(`table_sections`, `dining_tables`, and a `visits` table whose partial unique index makes "one open
visit per table" a database guarantee), and spec 0007 built the waiter's floor on them: live tables
grouped by section, each marked free or taken, updated for every waiter without a refresh. What
nothing builds yet is the restaurant's own floor. The only tables in any database are the four the
development seed writes, so a real restaurant that signs up today cannot take a single order.

The forces are mostly about the gap between setup and service. The admin changes the floor rarely
(opening day, a new terrace, a table broken or added), but those changes land while waiters may be
working, possibly at the very table being changed. Removing a table that has a party at it would leave
a waiter holding a meal on a table no screen can show. Renaming one changes what the kitchen reads
on a live ticket. Setup must be quick for a large room (thirty tables typed one at a time is a real
reason to abandon signup) and forgiving (a seasonal room removed in autumn should come back in spring
without retyping).

The admin surface for the menu (spec 0008) already solved the same shape of problem: ordered groups
of items, edited live, with stale edit protection, drag reordering, an archived list, and rules that
hold under concurrent writes. The staff and the restaurant have learned that screen. A second admin
screen that behaves differently would be a cost paid on every future admin screen too.

The word "floor plan" in the scope suggests a drawn map of the room. Whether that is wanted now, and
at what cost, is the central choice here. Whatever is chosen must work on a phone, with a keyboard,
and with a screen reader (spec 0004), and must not force a rebuild of the waiter's floor, which
feature 12 rebuilds anyway.

## Options considered

### Option 1: An ordered floor list, shaped like the admin menu

Sections as stacked groups, tables as rows inside them, order set by dragging within a group, table
details and section choice in a dialog. The waiter's floor keeps its current grouped list. No layout
columns are added; the order the admin sets is the order waiters walk.

**Pros**:
- Reuses a pattern already built, tested, and accessible (drag with keyboard, touch, and announcements).
- Works the same on a phone, a tablet, and a desktop, with no zooming or panning.
- Smallest schema change: a version column and a few constraints.
- Leaves room for a drawn map later as extra columns on top, without redoing any of it.

**Cons**:
- The screen does not look like the room. A new waiter cannot find "the table by the window" from it.
- Large rooms become long lists; order is the only spatial cue.

### Option 2: A grid layout per section

Each section is a grid of cells, and each table occupies a cell (a row and a column). The admin drags
tables between cells; waiters see the same grid, which roughly mirrors the room.

**Pros**:
- Rough spatial meaning with little freedom to make a mess (everything snaps to cells).
- Keyboard movement maps naturally to arrow keys between cells.

**Cons**:
- New columns (`grid_row`, `grid_col`), a uniqueness rule per cell, and a grid editor to build and make
  accessible.
- Grids that fit a desktop do not fit a phone; the waiter's floor needs a separate small screen layout.
- Feature 12 rebuilds the waiter's screen and would have to design around a grid decided before it.

### Option 3: A free room map

Tables placed anywhere on a canvas, with shapes and sizes, like a drawing of the room.

**Pros**:
- The most faithful picture of the restaurant; the best for new staff learning the room.
- What large competitors show in their marketing.

**Cons**:
- The largest build by far: a canvas editor, collision handling, zoom and pan, shapes.
- Very hard to make usable with a keyboard or a screen reader, which spec 0004 requires.
- Poor on a phone, which is what most waiters carry.
- Coordinates in the schema tie every future screen to one drawing.

## Rationale

Option 1 wins because the forces that matter most here are speed of setup, safety during service, and
one consistent admin pattern, and none of those need a picture of the room. The failure that actually
hurts a restaurant is a table vanishing under a seated party or a waiter opening a table the admin just
removed, and that is solved by row locks and refusal rules, identically for any layout. A drawn map is
real value for large venues, but it costs most of this feature's budget on the part least likely to be
right before feature 12 decides what the waiter's working screen really looks like. The ordered list is
also the base a map would sit on: a grid or canvas adds coordinates to these rows, so choosing Option 1
now forecloses nothing.

The removal rules lean strict on purpose. Refusing to remove a busy table, and refusing to remove a
section that still has tables, avoid every "where did my table go" case during service. The asymmetry
this creates (removing a room table by table, restoring it in one step) is the cheaper mistake: an
admin removing a room is rare and deliberate, while a side effect that removes live tables is the kind
of surprise that loses a restaurant's trust. The race safety comes from row locks that mirror spec
0008's category rule, so the code has one idea to understand, not two.

The range add exists because the largest onboarding cost in this feature is typing. Refusing a whole
range on any clash keeps the result predictable: either the admin gets exactly the tables they asked
for, in order, or nothing, with the reason listed. Carrying the clashing labels in the error body (a
small addition to the one error shape) keeps the server the single authority on what clashed, instead
of the web guessing from a cache that may be a second old. Labels are unique across the whole
restaurant, not per section, because kitchen tickets and bills print only the label, and two tables
called `1` would send food to the wrong room.

The one real risk left is the new lock in `open_visit`. It is short, it conflicts only with the admin
removing that exact table, and spec 0003's lock order is kept by taking it first. The alternative, a
trigger on `visits`, would hide the rule where nobody reading the repository looks.
