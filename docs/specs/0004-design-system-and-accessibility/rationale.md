# 0004. Design system and accessibility baseline: reasoning

The decision itself, and everything a build needs, is in [index.md](index.md). This file holds the
problem, the options weighed, and why the chosen one won.

## Context

> ⚠️ Premise note: the first pass agreed here is wider than the answer that set its size. The width
> was chosen as "only what the thin order thread needs", which matches the project's Tracer Bullet
> approach, and then a data table and a toast stack were added, neither of which slice 1 renders.
> The failure mode this invites is the one Tracer Bullet exists to prevent: components shipped
> without a real screen proving them, which then get rebuilt when the real screen finally arrives.
> The right framing is that these two are cheap insurance against three separate features each
> inventing a table, and that is a defensible call, so the plan keeps them but places them at the
> end, after the thread is proved. If time runs short, steps 10 and 12 of the build plan are the two
> to drop.

Three groups of people use this platform on three very different pieces of glass, and they share one
codebase. An owner does paperwork on a laptop at a desk and wants density: many rows on screen, a
mouse, a keyboard, daylight. A waiter works from a phone held in one hand while carrying plates in
the other, in a dim room, glancing rather than reading. A chef reads a screen on a hot wall from two
to three metres away, through steam, and taps it with a wet or gloved hand. A design that suits any
one of them fails the other two.

Nothing about the look has been decided. The scaffold from feature 1 left plain Tailwind screens,
two placeholder tokens (`--text-kitchen` and `--spacing-touch`), a reduced motion rule, and a comment
in `web/src/styles/index.css` saying that this feature owns the real answer. Spec 0001 chose Tailwind
v4 and shadcn/ui and stopped there; shadcn is not installed. There is no `docs/design.md`. Fifteen
features are queued behind this one and every one of them draws a screen.

The accessibility question is the expensive half. Set no level and every later feature's acceptance
criteria have nothing to point at, so accessibility becomes whatever the reviewer noticed that week.
Set a level and write it in a document, and it drifts the first time someone is in a hurry. The
retrofit cost is what makes this a foundation row rather than a feature: adding a label to every form
control, a focus ring to every interactive element, and a contrast guarantee to every colour, across
twenty screens, is a project of its own. Doing it once, in the components everything else is built
from, is a week.

There is also a live constraint from the schema. Spec 0003 fixed the order states as `queued`,
`ready`, `served`, and `voided`, with no cooking state and no late state. A design system that
invents states the database does not have produces screens nobody can build.

Two things are settled elsewhere and are not in play here. Feature 6 owns language and text, so this
feature must not hardcode a string but must not build the translation layer either. Feature 7 owns
accounts and roles, so the route groups stay open and nothing here may assume it knows who is
looking.

## Options considered

### Option 1: One scale for everyone, sizes chosen at each call site

Define one set of tokens and let each screen pass the size it wants, the way most component
libraries work by default: `<Button size="lg">` in the kitchen, `<Button size="sm">` in admin.

**Pros**:

- The simplest mental model in the codebase. Nothing cascades, nothing is remapped, and what you read in a component is what renders.
- No dependence on any Tailwind v4 internal behaviour, so it cannot break on a minor version.

**Cons**:

- The kitchen legibility rule lives in hundreds of call sites rather than one place, and every one of them can be forgotten. One missed prop is a dish a chef cannot read across the room, discovered during service.
- Every vendored shadcn component has to be edited to accept and thread the size, forever, which makes the CLI progressively less useful.

### Option 2: One semantic token layer remapped per surface, on shadcn's naming

Four layers of CSS variables. Components only ever use plain utility classes. Each surface redefines
Tailwind's own `--spacing` multiplier and its `--text-*` scale, so the same class produces three
different results in three places. Token names follow shadcn's Tailwind v4 convention
(`--background`, `--foreground`, `--muted-foreground`) rather than an invented scheme.

**Pros**:

- A component physically cannot forget a surface, because it never knew about surfaces. The guarantee is structural rather than remembered, which is the same shape as the tenant scoping rule in `AGENTS.md`.
- Vendored shadcn components become surface aware for free and stay unedited, so the CLI remains useful for the life of the project.
- The whole density system is about twenty lines of CSS in three blocks, which is a small thing to review and a small thing to change when a real kitchen screen proves a number wrong.

**Cons**:

- It is clever, and clever costs comprehension. `h-9` meaning three different heights is genuinely surprising to a new reader, and no amount of documentation removes the surprise entirely.
- It leans on Tailwind v4 emitting utilities that reference variables. That is how v4 works today and how shadcn's own theming depends on it working, but it is still a behaviour rather than a promise.
- Portals escape the React tree, so the surface attribute has to go on the document element. That is a subtle rule someone can get wrong.

### Option 3: Three separate token sets, one per surface

Admin, waiter, and kitchen each get their own tokens and their own component variants, selected by
import or by folder.

**Pros**:

- Maximum clarity. Open the kitchen tokens and you see exactly what the kitchen renders, with nothing cascading in from elsewhere.
- Each surface can diverge as far as it needs without arguing with the other two.

**Cons**:

- Every shared component grows three code paths, and the three drift apart the first time one is fixed and the others are not. This is the classic way a design system stops being one system.
- Triples the contrast checking, the axe runs, and the visual review, since there are three of everything rather than one thing at three sizes.

### Option 4: Scale the root font size per surface

A much smaller version of Option 2. Rather than shadowing a dozen Tailwind scale variables, set one
percentage font size on the document element per surface. Tailwind v4's spacing and type scales are
both in `rem`, so everything rescales from that single line, including the container widths that
Option 2 has to handle with a separate token.

**Pros**:

- One line per surface instead of a block, and no dependence on knowing which Tailwind internal variable name backs which utility category.
- It removes the comprehension cost Option 2's own rationale admits is its worst feature, because `h-9` keeps meaning one thing and only the base unit moves.
- A percentage multiplies the reader's own browser font size rather than replacing it, so it does not stomp on someone who has set a larger default.

**Cons**:

- Tailwind v4's breakpoints are also in `rem` and are evaluated against the root font size, so tripling it for the kitchen silently moves every responsive breakpoint on that surface. That is a hard bug to see and a worse one to explain.
- It welds the type scale to the spacing scale at one ratio. The kitchen wants roughly double the type but not double every gap, and this option cannot express that difference at all.
- It still does not fix border widths, which are in pixels either way, so it does not actually remove the exception list.

**Also weighed and dismissed early**: adopting a full component library with its own theme and
overriding what the kitchen needs. It is by far the fastest route to a working screen, with
accessibility already handled inside the components, but no mainstream library is built for a 72
pixel tap target read at three metres, so the kitchen surface becomes a pile of overrides fighting
the library's own scale. It also reverses spec 0001, which chose shadcn precisely because components
are copied into the repository rather than imported, so the cost of that choice is already paid.

## Rationale

Option 2 wins on the force that dominates this feature: three contexts, one codebase, and fifteen
features queued behind it. The Context makes the kitchen requirement non negotiable, and Option 1
puts that requirement in the hands of whoever writes the next screen. The project already made this
same call once, in the `AGENTS.md` rule that tenant scoping is structural rather than a convention
because someone will eventually forget. The same reasoning applies here: a guarantee a component
cannot break is worth more than a guarantee a component is asked not to break.

Option 4 is the one that nearly changed the answer, because it is genuinely simpler and it fixes the
one thing this design is worst at. It lost on the breakpoint problem: Tailwind v4 evaluates its
breakpoints against the root font size, so scaling that per surface moves the responsive layout on
the kitchen screen for reasons nothing in the code explains. Trading a comprehension cost that a
comment in `docs/design.md` can fix for a silent layout bug that only shows up on one device is the
wrong direction. Its second problem is decisive on its own: it cannot give the kitchen double the
type without also giving it double the gap, and the kitchen needs the first and not the second.

Option 3 was the other real contender, and it lost to the drift argument. Three token sets is three systems
in a trench coat, and this codebase has one developer. The moment a fix lands in the waiter set and
not the other two, the promise that a shared component behaves consistently is gone, and nothing in
the tooling would notice. Option 2's clever cascade is a comprehension cost paid once by a reader;
Option 3's duplication is a correctness cost paid repeatedly by everyone.

Keeping shadcn's token names, rather than the more descriptive scheme the `design-tokens` skill
suggests, is a deliberate trade. Names like `--color-bg-primary` read better than `--background`.
But every component the shadcn CLI generates hardcodes `bg-background`, `text-muted-foreground`, and
`border-border`, so a nicer scheme means a rename pass on every component, forever, in exchange for
readability in a file almost nobody opens. The skill's structural advice, separate palette from
semantic roles and design both appearances deliberately rather than inverting one, is followed in
full; only the naming is overridden, and only because a prior decision in spec 0001 makes it
expensive.

On the accessibility half, WCAG 2.2 level AA was chosen over 2.1 because 2.1 is superseded and the
two criteria that differ most, minimum target size and focus not obscured, are exactly the two this
product needs anyway: a chef with gloves and a waiter's phone with a sticky header. Enforcement by
three gates rather than by review follows from the same reasoning as the token architecture. A level
written in a document degrades silently; a level that fails `pnpm check` does not. The contrast
script is the only one of the three that is new code, and it is perhaps forty lines, because the
plugin and the test runner are both already installed.

Two answers here run against the project's own defaults, and both are conscious. Dark first with a
fully built light theme costs roughly double the appearance work, which the Consequences record;
it was chosen because two of the three surfaces live in dim rooms all evening and the third is used
by one person. Carrying a data table and a toast stack in the first pass is speculative work under
Tracer Bullet, and the premise note above says so plainly; it was chosen because five later features
need a table and the alternative is five tables.
