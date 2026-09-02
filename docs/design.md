# Design system

The visual language every screen in this platform is built from, and the promises it makes to the
people using it. Decided in [spec 0004](specs/0004-design-system-and-accessibility/index.md).

**The token values live in [`web/src/styles/index.css`](../web/src/styles/index.css), not here.**
This file says what the system is and why. The stylesheet is the only place a value is written down,
so the two can never drift apart into two different answers.

See it all at once: run `pnpm dev:web` and open `/design`. That route exists only in development.

## Character

A dark console. Deep slate surfaces, one luminous cyan for the action you are meant to take, and
colour otherwise held back and spent only on order status. It should feel like equipment rather than
a website: quiet, high contrast, nothing decorative competing with the one number a chef needs to
read from across a hot kitchen.

Dark is what a device with no stated preference gets, because a restaurant runs its screens in a dim
dining room and a bright kitchen all evening and neither wants a white rectangle at eye level. A
device asking for light gets a light theme that was designed rather than inverted.

## Build mandate

- **A base component never branches on which surface it is in.** If one needs to, the density layer
  is missing a token and that token gets added instead.
- **Colour is never the only carrier of meaning.** Every status is a colour, a word, and an icon.
- **Every focus ring is an `outline`, never a `box-shadow`**, because forced colours mode erases box
  shadows and the ring would silently vanish for the people who most need it.
- **Every boundary is a real border**, for the same reason, and because a background shade is
  invisible from three metres away.
- **Every spacing, alignment, and border property is logical**, so `dir="rtl"` mirrors the layout
  with no component edited. A lint rule refuses the physical utilities.
- **Every user facing string comes through `t()`.** None is written into a component.
- **Sound is always an addition to a visual signal, never a replacement for one.**

## The four token layers

All four live in `web/src/styles/index.css`, in this order.

| Layer | Selector | Holds | Overridden by |
|---|---|---|---|
| 1. Palette | `:root` | raw values only, e.g. `--console-950`, `--cyan-400` | nobody |
| 2. Semantic roles | `:root`, the light media query, `[data-theme]`, print, forced colours | `--background`, `--foreground`, `--card`, `--border`, `--primary`, `--ring`, `--status-*` | the appearance |
| 3. Density | `[data-surface='admin' \| 'waiter' \| 'kitchen']` | `--spacing`, the `--text-*` scale, `--radius`, `--border-width`, `--ring-width`, `--target-min`, `--icon-stroke`, `--shell-width` | the surface |
| 4. Tailwind bridge | `@theme inline` | maps roles onto utility names, plus the radius `calc()` chain | nobody, it is plumbing |

### The two mechanics that make it work

Tailwind emits spacing utilities as `calc(var(--spacing) * n)`, so redefining the single `--spacing`
value under a surface selector rescales every `p-*`, `gap-*`, `h-*`, and `w-*` at once. It emits text
utilities as `font-size: var(--text-sm)`, so redefining Tailwind's own scale names rescales every
vendored component's type too.

**This is why the scale keeps Tailwind's names** (`--text-sm`) rather than prettier invented ones:
invented names would mean editing every component the shadcn CLI ever generates.

The cost, and it is real: `h-9` in a component means three different heights. Anyone reading a
component has to know that. It is the single most likely thing to confuse someone new here.

### Three things the cascade does not reach

Each has its own token rather than being left to surprise someone.

| What | Why it escapes | How it is handled |
|---|---|---|
| border widths | fixed pixels, not on the spacing scale | `--border-width`, applied by the `border-line` utility |
| container widths | a separate Tailwind namespace | `--shell-width`, applied by the `shell-width` utility, owned by `SurfaceShell` |
| Lucide's `size` prop | writes literal pixels onto the SVG, which no variable can reach | `Icon` sizes itself with a `size-*` class and never passes that prop |

There is also a fourth trap, in JavaScript rather than CSS: `tailwind-merge` classifies `border-line`
as a border *colour* and would throw the width away. `web/src/shared/ui/cn.ts` teaches it otherwise,
and `cn.test.ts` holds the regression.

## Density, per surface

| | `admin` | `waiter` | `kitchen` |
|---|---|---|---|
| the room | a laptop on a desk, mouse and keyboard | a phone at arm's length, one thumb | a wall screen at two to three metres, wet or gloved hands |
| `--spacing` | `0.25rem` | `0.3125rem` | `0.5rem` |
| body (`--text-sm`) | 14px | 16px | 28px |
| `--text-xs` → `--text-3xl` | 12 → 30px | 14 → 36px | 24 → 64px |
| `--target-min` | 32px | 44px | 72px |
| `--border-width` | 1px | 1px | 2px |
| `--ring-width` | 2px | 2px | 3px |
| `--icon-stroke` | 1.75 | 2 | 2.5 |
| `--radius` | 0.5rem | 0.625rem | 0.875rem |
| `--shell-width` | 80rem | 36rem | full width |

Measured in a real browser, the same `Button` with no per surface code renders **36px / 45px / 72px**
tall, its icon **16px / 20px / 32px**.

16px on the waiter surface is not an aesthetic choice: below it, iOS zooms the whole page when an
`<input>` or `<select>` takes focus, and a waiter pinching back out mid order stops using the app.

**Who sets `data-surface`:** `RootLayout`, in one effect keyed on the pathname, on the **document
element**. Not on a wrapper, because dialogs, toasts, and tooltips render into a portal attached to
`document.body`, outside the React tree, and a wrapper would leave every overlay at admin density in
the kitchen. Not per shell either, because the attribute outlives the component that set it: walking
from the kitchen back to `/` would leave the document stuck at kitchen density forever. The first
path segment decides, and anything unmatched is `admin`.

## Appearance

- `:root` carries the dark roles. Dark is the default.
- `@media (prefers-color-scheme: light)` redefines them to the light set and flips `color-scheme`, so
  native controls, scrollbars, and form widgets follow.
- `[data-theme='dark']` and `[data-theme='light']` repeat both sets. **This is a forcing hook for the
  `/design` gallery and the axe tests only. Nothing in the product ever sets it.** It exists because
  a media query is document wide: without it the gallery could not show both appearances at once and
  jsdom could not put a component in light for its axe run, so the light theme would ship unproven.
- `@media print` is ink on white with visible borders and no dark fills. Two halves, and both are
  needed. The token half turns every surface white and every ink black. The rule half drops every
  background and forces every label to ink, because a filled control asks for the opposite by name:
  `bg-primary` with `text-primary-foreground` is white on black by design and would print as a solid
  rectangle. The tokens cannot simply be swapped instead, because the contrast gate needs a dark
  `--primary` sitting on a white page. Borders are left alone, so a button still prints as a button,
  drawn rather than blocked in. Screen only chrome, the skip link so far, carries `print:hidden`.
- `@media (forced-colors: active)` hands every role to the operating system. Boundaries survive
  because they are real borders, the focus ring survives because it is an outline, and status
  survives because it carries an icon and a word.
- `index.html` carries `<meta name="color-scheme" content="dark light">`, dark first, so the browser
  paints the right canvas before the stylesheet arrives and there is no white flash on a kitchen wall.

There is no stored appearance preference, by decision.

## Colour roles

| Role | Dark | Light |
|---|---|---|
| `--background` | `#0B0F14` | `#FFFFFF` |
| `--card` / `--popover` | `#151B23` | `#F6F8FA` / `#FFFFFF` |
| `--muted` | `#1A222C` | `#EEF2F6` |
| `--secondary` / `--accent` | `#1E2733` | `#E8EDF3` |
| `--foreground` | `#E6EDF5` | `#0B0F14` |
| `--muted-foreground` | `#94A3B8` | `#4B5866` |
| `--primary` | `#38BDF8` | `#0369A1` |
| `--primary-foreground` | `#04121B` | `#FFFFFF` |
| `--destructive` | `#F87171` | `#B91C1C` |
| `--border` | `#616C7A` | `#868E98` |
| `--input` | `#6A7583` | `#79828E` |
| `--ring` | `#7DD3FC` | `#0369A1` |

`--border` and `--input` are noticeably lighter than a designer's instinct would put them. That is
deliberate: they carry the 3:1 boundary promise, and a tasteful hairline that measures 1.4:1 is
invisible to someone with low vision and invisible to everyone from across a kitchen.

## Status

Four real states from the `line_status` and `round_status` enums in
[spec 0003](specs/0003-core-data-model/index.md), plus one derived emphasis. Nothing here invents a
state the database does not have.

| Status | Source | Dark | Light | Icon | Word |
|---|---|---|---|---|---|
| `queued` | the enum | `#FBBF24` | `#9E5A07` | `Flame` | `t('status.queued')` |
| `ready` | the enum | `#4ADE80` | `#147B3B` | `CircleCheck` | `t('status.ready')` |
| `served` | the enum | `#94A3B8` | `#4B5866` | `Check` | `t('status.served')` |
| `voided` | the enum | `#9AA7B8` | `#5D6C81` | `Ban` | `t('status.voided')`, dashed border |
| `late` | derived, never stored | `#FB7185` | `#B91C1C` | `TriangleAlert` | `t('status.late')` |

The map lives in `web/src/shared/ui/status.ts`.

## Components

All in `web/src/shared/ui/`, one file each.

| Component | States it carries | The accessibility promise it holds |
|---|---|---|
| `SurfaceShell` | one per surface, header + main | skip link onto a `tabindex="-1"` `<main>` so following it really moves focus, landmarks, one `<h1>` per screen, mounts the live regions and the toast viewport, primes the audio unlock |
| `Button` | primary, secondary, ghost, destructive × sm, md, lg, icon; hover, focus, disabled | `--target-min` floor, real border, outline focus ring, `aria-disabled` so it stays in the tab order and stays announced |
| `Card` | default | a real border, so it survives forced colours and reads at kitchen distance |
| `Icon` | sm, md, lg | sized by class, stroke from `--icon-stroke`, `aria-hidden` unless labelled |
| `StatusPill` | five statuses × full, compact | colour plus word plus icon, always all three |
| `ElapsedTime` | ticking, late | a `<time>` element, the whole duration in hidden text beside the clock face, mono tabular digits |
| `Field` | default, required, hint, error | binds the label, sets `aria-invalid`, links messages through `aria-describedby`, announces the error |
| `Input`, `Select` | default, invalid, disabled | never used bare; `Field` throws if you try. Disabled dims like a `Button` does and hands forced colours `GrayText` |
| `Dialog` | open, closed | focus trapped, Escape closes, focus handed back to the opener, titled and described |
| `LiveRegion` | polite, assertive | the one announcement mechanism, mounted once, always in the document |
| `Toast` | one tone per status, auto dismiss, manual dismiss | announces politely, never takes focus, dismissible by keyboard; the viewport is a named landmark, because a toast is pinned to the viewport rather than sitting inside `<main>` and would otherwise be content inside no landmark at all |
| `Alert` | open, closed, per status | fires visually, then through the live region, then optionally a chime |
| `Skeleton` | pulsing, still under reduced motion, labelled or decorative | one announcement per group, a border under forced colours |
| `EmptyState` | with and without an action | the one empty pattern every list uses |
| `DataTable` | sorted, unsorted, empty | real table semantics, a caption, one row header per row, `aria-sort`; the scroll box is a focusable region named by the caption, which is what lets a keyboard user reach the far side of a table that does not fit a phone |
| `ConnectionStatus` | connecting, open, closed | announced politely |

### The late threshold

`ElapsedTime` defaults to **900 seconds (15 minutes)**. That is a placeholder and it is written down
as one. The real threshold belongs to each restaurant and is feature 13's decision; the component
takes it as a prop so feature 13 supplies the real value with nothing here rewritten.

## Accessibility level

**WCAG 2.2 level AA, everywhere.** Not a document, three build failures:

| Gate | What it catches | Where |
|---|---|---|
| the contrast script | a colour edit that drops any declared pair below 4.5:1 (text) or 3:1 (boundaries and rings), in dark, light, and print | `web/scripts/check-contrast.ts`, reading the real stylesheet, 144 pairs |
| lint | bad markup (`jsx-a11y` strict) and any physical direction utility in a class string | `web/eslint.config.js` |
| axe | any violation in any base component, in both appearances and all three densities, six renders each | `web/src/test/axe.tsx` |

All three run in `pnpm check`.

**axe's own `color-contrast` rule is switched off in the shared helper, deliberately.** jsdom has no
layout and paints nothing, so the rule cannot see what is actually behind an element. Colour is owned
end to end by the contrast script. Two gates both claiming colour, one of them unreliable, is worse
than one gate that is trusted.

### What the gates do not cover

Checked by hand for now, and by Playwright once it joins continuous integration at slice 1:

- a full keyboard walk of a real screen
- forced colours mode on real hardware (the rules compile and are structurally sound, but only
  Windows high contrast proves it)
- the print layout against a real kitchen ticket and a real bill
- the audio unlock across real browsers and real devices, which is feature 12's problem
