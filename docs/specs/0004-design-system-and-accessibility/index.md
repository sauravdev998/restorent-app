# 0004. Design system and accessibility baseline

**Date**: 2026-09-01
**Status**: In Progress

## Summary

This decision fixes what every screen in the platform looks like and what it guarantees for the
people using it. The look is a dark console: deep slate surfaces, one luminous cyan for the action
you are meant to take, and colour otherwise reserved for order status. One set of tokens (named
values such as "body text size" and "spacing unit") is defined once and then remapped per surface,
so the same button is 36 pixels tall for an admin at a desk, 45 on a waiter's phone, and 72 on a
kitchen screen read from three metres away, with no per surface code in the component. The
accessibility target is WCAG 2.2 level AA everywhere, and it is enforced by three automatic gates
rather than by good intentions: lint errors on bad markup, an axe check over every base component,
and a contrast script that fails the build when a colour edit drops a pair below the required
ratio. Nothing in the database changes and no endpoint is added.

Reasoning and options: see [rationale.md](rationale.md).

## Requirements

**User stories**:

- As a chef, I want to read a ticket from across a hot kitchen and tap a dish done with a gloved wet hand, so that food leaves the pass without me walking to the screen twice.
- As a waiter, I want to know food is ready even with my phone face down in an apron in a loud room, so that hot food does not sit under the lamp.
- As an owner, I want the admin screens to be dense enough to do paperwork on a laptop, so that I am not scrolling through a screen built for a kitchen wall.
- As a member of staff who is colour blind, deaf, uses a keyboard only, or runs the operating system's high contrast mode, I want every screen to work for me, so that a job on a restaurant floor is not closed to me by the software.
- As an engineer building features 7 through 20, I want the tokens, the base components, and the accessibility rules already fixed and enforced, so that I am not inventing a look or an aria attribute partway through a slice.

**Acceptance criteria** (the contract, each independently checkable):

- **AC-1**: Every colour pair the system declares meets its threshold in both appearances: 4.5:1 for text on its background, 3:1 for interface boundaries and focus rings against theirs. A script reads the real values out of `web/src/styles/index.css` (never a hand copied duplicate) and fails `pnpm check` below the threshold, so a colour edit cannot quietly break the promise.
- **AC-2**: One base component, with no per surface code inside it, renders at three densities driven only by the surface it sits in. Interactive height is at least 32px in admin, 44px in waiter, and 72px in kitchen; body text is at least 14px, 16px, and 28px respectively.
- **AC-3**: Dark is the appearance a device with no stated preference receives. A device asking for light receives a fully built light theme, not a washed out dark one, and every base component is proved in both.
- **AC-4**: Every base component passes an automated axe check with zero violations, in both appearances and all three densities.
- **AC-5**: Every order status is carried by three signals together: its colour, its translated word, and its own icon. With colour removed, every status is still distinguishable. The status token map is keyed on the real `line_status` and `round_status` enum values from spec 0003 (`queued`, `ready`, `served`, `voided`) plus one derived `late` emphasis, and it invents no state the database does not have.
- **AC-6**: Every interactive element is reachable and operable by keyboard alone, in a sensible order, with a visible focus ring meeting 3:1 against what is behind it, and nothing that receives focus is hidden behind a sticky header or a bar.
- **AC-7**: The dialog holds focus while open, closes on Escape, returns focus to whatever opened it, and is announced with its own title.
- **AC-8**: Every form control has a label bound to it programmatically. An invalid control sets `aria-invalid` and links its message through `aria-describedby`, and the message is announced rather than only shown.
- **AC-9**: Once invoked, the ready alert always fires visually and through a live region. Sound is an addition layered on top: a muted phone, a phone whose audio was never unlocked, and a deaf waiter all still get the alert. Toasts and alerts share one live region mechanism rather than each having their own. What invokes the alert (which stream event, watched where) is feature 12's decision, not this one; this criterion covers the component's behaviour once called.
- **AC-10**: With `prefers-reduced-motion: reduce` set, no animation or transition in any base component exceeds 0.01ms, skeletons do not pulse, and nothing moves on its own.
- **AC-11**: Under `forced-colors: active` every base component keeps a visible boundary, a visible focus ring, and a distinguishable status, because the boundary is a real border and the status carries an icon rather than a background colour alone.
- **AC-12**: Money, elapsed times, and bill numbers render in the mono face with fixed width digits, so a value whose digits change does not change width and a column of prices aligns on the decimal point.
- **AC-13**: No base component writes a physical direction. Spacing, alignment, and borders use logical properties throughout, so setting `dir="rtl"` on the document mirrors the layout without editing a component. A lint rule refuses the physical utilities (`ml-*`, `pr-*`, `text-left`, `border-l`, and the rest) in a JSX class string, so this cannot decay.
- **AC-14**: No user facing string is written into a base component. Every one comes through `t()` and lives in `web/src/locales/<lang>/common.json`.
- **AC-15**: A development only `/design` route renders every token and every base component in every state, in both appearances and all three densities, and it is absent from a production build.
- **AC-16**: A printed page is ink on white with visible borders and no dark fills, so a kitchen ticket and a customer bill do not print as a black rectangle.
- **AC-17**: `docs/design.md` records the tokens, the type and spacing scale per surface, the component list with each component's states, and the accessibility level, and every base component matches what it says.

## Decision

**Chosen option**: Option 2: One semantic token layer remapped per surface, on shadcn's Tailwind v4 naming.

Adopt a dark first console look built as four stacked layers of CSS variables (raw palette, semantic roles, per surface density, Tailwind bridge), keep shadcn's own token names so vendored components work unedited, and enforce WCAG 2.2 level AA with three automatic gates in `pnpm check`.

**Implementation skills**: `accessibility-compliance` (`wshobson/agents`, `.agents/skills/accessibility-compliance/`) · `wcag-audit-patterns` (`wshobson/agents`, `.agents/skills/wcag-audit-patterns/`) · `accessibility` (`addyosmani/web-quality-skills`, `.agents/skills/accessibility/`) · `design-tokens` (`julianoczkowski/designer-skills`, `.agents/skills/design-tokens/`) · `lucide-icons` (`aksuharun/skills`, `.agents/skills/lucide-icons/`) · `shadcn` (`shadcn/ui`, `.agents/skills/shadcn/`) · `tailwind-4-docs` (`lombiq/tailwind-agent-skills`, `.agents/skills/tailwind-4-docs/`) · `react-i18next` (`yildizberkay/skills`, `.agents/skills/react-i18next/`) · `vitest` (`antfu/skills`, `.agents/skills/vitest/`)

## Rationale

Reasoning and options: see [rationale.md](rationale.md).

## Feature design

**Data model sketch**: none. This feature adds no table, no column, and no migration. Branding is
fixed for every restaurant and there is no stored appearance preference, so nothing needs a home in
the database. The one place the design system reads the schema is the status token map, which is
keyed on the existing `line_status` and `round_status` enums, which spec 0003 decided and
`api/migrations/0002_core_data_model.sql` implements (the spec number and the migration number
differ because migration 0001 is the bootstrap).

**Token architecture** (four layers, all in `web/src/styles/index.css`):

| Layer | Where it lives | What it holds | Who overrides it |
|---|---|---|---|
| 1. Palette | `:root` | raw values only, e.g. `--console-950: #0B0F14`, `--cyan-400: #38BDF8` | nobody, these are constants |
| 2. Semantic roles | `:root` and `[data-theme='dark']`, plus a light block, a print block, and a forced colors block | `--background`, `--foreground`, `--card`, `--border`, `--primary`, `--muted-foreground`, `--ring`, `--status-*` | appearance (dark default, light, print, forced colors) |
| 3. Density | `[data-surface='admin' \| 'waiter' \| 'kitchen']` | `--spacing`, `--text-xs` through `--text-3xl` and their line heights, `--radius`, `--border-width`, `--target-min`, `--icon-stroke` | the surface the component sits in |
| 4. Tailwind bridge | `@theme inline` | maps role names onto utility names: `--color-background: var(--background)` for colour, and the `calc()` chain shadcn uses for radius, `--radius-sm: calc(var(--radius) - 4px)` through `--radius-xl: calc(var(--radius) + 4px)` | nobody, it is plumbing |

Two mechanics carry the whole design and are the reason a component needs no per surface code:

- Tailwind v4 emits spacing utilities as `calc(var(--spacing) * n)`, so redefining the single
  `--spacing` value under a surface selector rescales every `p-*`, `gap-*`, `h-*`, and `w-*` at
  once. A vendored shadcn button at `h-9` is 36px in admin, 45px in waiter, and 72px in kitchen
  without touching the button.
- Tailwind v4 emits text utilities as `font-size: var(--text-sm)`, so redefining Tailwind's own
  scale names under a surface selector rescales every vendored component's type too. This is why
  the scale keeps Tailwind's names (`--text-sm`) rather than invented ones (`--text-body`):
  invented names would mean editing every component the shadcn CLI ever generates.

**What the cascade does not reach** (the caveat that keeps the promise honest). Two categories of
utility do not ride `--spacing` and therefore do not rescale on their own. Both are handled by their
own density token rather than being left to surprise someone:

| Category | Why it is exempt | How this spec handles it |
|---|---|---|
| border widths (`border`, `border-2`) | fixed pixel values, not on the spacing scale | `--border-width` in the density layer; `Card`, `Button`, and `StatusPill` route their border through it, so a hairline in admin is a readable line on a kitchen wall |
| container widths (`max-w-*`) | a separate `--container-*` namespace | `SurfaceShell` owns the content width per surface and takes it from its own token, so `root-layout.tsx`'s current `max-w-4xl` moves into the shell rather than staying a fixed literal |

Lucide's `size` prop is a third trap of the same kind: it writes literal pixel `width` and `height`
onto the SVG, which no CSS variable can reach. `Icon` therefore sizes itself with a `size-*` utility
class, which does ride `--spacing`, and never passes Lucide's `size` prop. Without this rule every
icon, including one of `StatusPill`'s three required signals, stays fixed while the type around it
triples.

Density values:

| Surface | `--spacing` | body (`--text-sm`) | `--target-min` | `--border-width` | context |
|---|---|---|---|---|---|
| `admin` | `0.25rem` | `0.875rem` (14px) | `2rem` (32px) | `1px` | laptop at a desk, mouse and keyboard, dense |
| `waiter` | `0.3125rem` | `1rem` (16px) | `2.75rem` (44px) | `1px` | phone at arm's length, one thumb. 16px is also the size below which iOS zooms a focused `<input>` or `<select>`, so both stay at or above it |
| `kitchen` | `0.5rem` | `1.75rem` (28px) | `4.5rem` (72px) | `2px` | wall screen at 2 to 3 metres, gloved wet hands |

**Who sets `data-surface`, and when.** It goes on the **document element**, not on a wrapping div,
because dialogs, toasts, and tooltips render into a portal attached to `document.body`, outside the
React tree. A wrapper div would leave every overlay at admin density in the kitchen, which is exactly
the screen where that failure hurts most.

It is owned in one place, `RootLayout`, in an effect keyed on the current pathname, and never by the
individual shells. The mapping is the first path segment: `admin`, `waiter`, `kitchen`, and
**`admin` for `/` and for anything unmatched**. Ownership has to be central because the attribute
outlives the component that set it: if each `SurfaceShell` set it on mount, walking from the kitchen
back to `/` would leave the document stuck at kitchen density with nothing to clear it.

**Appearance selection**:

- `:root` carries the dark roles. Dark is what a device with no stated preference gets.
- `@media (prefers-color-scheme: light)` redefines the roles to the light set, and flips the CSS
  `color-scheme` so native controls, scrollbars, and form widgets follow.
- **Both role sets are also written under a plain `[data-theme='dark']` and `[data-theme='light']`
  selector**, mirroring the media query. This is a forcing hook for the `/design` gallery and for the
  axe tests, not a user facing control, and nothing in the product ever sets it. It is needed because
  a media query is document wide: without it the gallery cannot show the two appearances side by
  side (AC-15) and jsdom cannot put a component in light for its axe run (AC-4), so the light theme
  would ship unproven. Because the role tokens are inherited custom properties, putting the attribute
  on any container switches that subtree, which is what side by side needs. Overlays escape into a
  portal and so follow the document, which the gallery accounts for by not rendering an overlay in a
  forced panel.
- `@media print` always uses ink on white, drops every background fill, and keeps borders.
- `@media (forced-colors: active)` stops fighting the operating system: boundaries stay because they
  are real borders, focus stays because the ring is an `outline` rather than a `box-shadow`, and
  status stays legible because it carries an icon and a word.
- `index.html` gets `<meta name="color-scheme" content="dark light">`, dark first, so the browser
  paints the right canvas before the stylesheet arrives and there is no white flash on a kitchen
  screen.

**Palette**. These values were computed against their thresholds while writing this spec, not guessed:
the seed `--status-voided` of `#64748B` measured 4.06:1 on the dark canvas, under the 4.5:1 text
threshold, and was raised to `#9AA7B8` (7.9:1). The script in AC-1 remains the authority, and any
pair it rejects is adjusted rather than excused.

| Role | Dark (default) | Light |
|---|---|---|
| `--background` | `#0B0F14` | `#FFFFFF` |
| `--card` | `#151B23` | `#F6F8FA` |
| `--border` | `#232C38` | `#D6DDE5` |
| `--foreground` | `#E6EDF5` | `#0B0F14` |
| `--muted-foreground` | `#94A3B8` | `#4B5866` |
| `--primary` | `#38BDF8` | `#0369A1` |
| `--primary-foreground` | `#04121B` | `#FFFFFF` |
| `--ring` | `#7DD3FC` | `#0369A1` |
| `--status-queued` | `#FBBF24` | `#A15C07` |
| `--status-ready` | `#4ADE80` | `#15803D` |
| `--status-served` | `#94A3B8` | `#4B5866` |
| `--status-voided` | `#9AA7B8` | `#64748B` |
| `--status-late` | `#FB7185` | `#B91C1C` |

**Status presentation** (AC-5). The database has four states and no notion of late, so the design
system carries four statuses plus one derived emphasis, and invents nothing:

| Status | Source | Colour | Lucide icon | Word |
|---|---|---|---|---|
| `queued` | `line_status` / `round_status` | `--status-queued` | `Flame` | `t('status.queued')` |
| `ready` | same | `--status-ready` | `CircleCheck` | `t('status.ready')` |
| `served` | same | `--status-served` | `Check` | `t('status.served')` |
| `voided` | same | `--status-voided`, dashed border | `Ban` | `t('status.voided')` |
| `late` | derived, not a database state | `--status-late` | `TriangleAlert` | `t('status.late')` |

**Base components** (`web/src/shared/ui/`, one file per component):

| Component | What it owns | Notable accessibility behaviour |
|---|---|---|
| `SurfaceShell` | the per surface frame: header, nav, `<main>`, and the content width | skip link to main, one `<h1>` per screen, landmarks |
| `Button` | primary, secondary, ghost, destructive | minimum `--target-min`, `--border-width` border, focus ring, disabled is `aria-disabled` not removed from the tab order |
| `Card` | the surface a ticket or a panel sits on | a real `--border-width` border, so it survives forced colors and reads at kitchen distance |
| `Icon` | the Lucide wrapper | sized by a `size-*` class and never Lucide's `size` prop, stroke width from `--icon-stroke`, `aria-hidden` unless given a label |
| `StatusPill` | colour plus word plus icon | the AC-5 guarantee in one place |
| `ElapsedTime` | a ticking duration in the mono face | a `<time>` element, full text in `aria-label`, `late` emphasis past a threshold |
| `Field` | label, control, hint, error, required marker | binds the label, sets `aria-invalid`, links the message with `aria-describedby` |
| `Input`, `Select` | the bare controls `Field` wraps | never used bare, always through `Field` |
| `Dialog` | the overlay for choosing a dish | Radix underneath: focus trap, Escape, focus restored, titled |
| `LiveRegion` | one polite region and one assertive region, mounted once | the single mechanism `Toast` and `Alert` both announce through |
| `Toast` | transient confirmations | announces politely, never steals focus, dismissible by keyboard |
| `Alert` | the ready alert: badge, sound, announcement | sound is an addition, never the only channel |
| `Skeleton` | shape matched loading | no pulse under reduced motion |
| `EmptyState` | icon, one line, optional action | the one empty pattern every list uses |
| `DataTable` | admin lists | real `<table>` semantics, `aria-sort` on sortable headers, keyboard reachable rows |
| `ConnectionStatus` | the live stream indicator, moved out of `root-layout.tsx` | keeps the existing `aria-live="polite"` behaviour |

**API surface**: none. This feature adds no endpoint and changes no existing one.

**Value sourcing**:

| Action | Value produced / displayed | Source |
|---|---|---|
| render any screen | the active density | `data-surface` on the document element, set in one effect in `web/src/app/root-layout.tsx` keyed on the pathname, defaulting to `admin` |
| render any screen | the active appearance | `prefers-color-scheme`, defaulting to dark. There is no stored preference, by decision. The `data-theme` attribute forces it for the gallery and the tests only |
| render any screen | the content width | `SurfaceShell`'s own per surface token, because `max-w-*` sits in Tailwind's container namespace and does not ride `--spacing` |
| render a status | the status word | `t('status.<state>')` in `web/src/locales/<lang>/common.json`, key names fixed by the table above |
| render a status | the colour and the icon | the fixed status token map above, keyed on the `line_status` / `round_status` enums in spec 0003 |
| render elapsed time | the elapsed duration | browser `now()` minus the round's `created_at` (`timestamptz`, spec 0003) |
| render elapsed time | whether it is late | a design token threshold in `docs/design.md` for now. The real per restaurant threshold is feature 13's decision, and `ElapsedTime` takes it as a prop so feature 13 supplies it without a rewrite |
| render money | decimal places and the symbol | `restaurants.currency_code` and `restaurants.currency_decimals` (spec 0003). The design system fixes only the mono face and the tabular alignment, never the rounding |
| fire the ready alert | whether audio may play | a module scoped flag in `web/src/shared/ui/audio-unlock.ts`, primed on the first pointer or key event of the session. Not persisted, and never a gate on the visual alert |
| fire the ready alert | what invokes it | feature 12, from the live stream in `web/src/shared/events/use-live-events.ts`. Out of scope here by AC-9 |
| run the contrast gate | which pairs to check and at what threshold | `web/scripts/contrast-pairs.ts`, naming **pairs of variable names** (e.g. `foreground` on `background`) each tagged `text` or `interface`, which fixes the threshold |
| run the contrast gate | the actual colour values | parsed out of `web/src/styles/index.css` with `postcss` at check time, walking each `@media` and `[data-theme]` block as its own appearance scope. Never a hex value copied into the script, because a copy drifts the first time someone edits the palette and the gate then passes on stale numbers |

**Key invariants**:

- A base component never reads `data-surface` and never branches on the surface. If one needs to, the density layer is missing a token and that token is added instead.
- The contrast gate never holds its own copy of a colour. It reads the stylesheet, or it is not a gate.
- axe's own `color-contrast` rule is switched off in the shared helper, because it is unreliable under jsdom, which has no layout or paint. Colour is owned entirely by the contrast script, and the two never both claim it.
- Colour is never the only carrier of meaning, anywhere.
- Every focus ring is an `outline`, never a `box-shadow`, so forced colors mode keeps it.
- Every user facing string in a base component comes through `t()`.
- Every spacing, alignment, and border property is logical (`margin-inline-start`, not `margin-left`).
- Sound is always an addition to a visual signal, never a replacement for one.

**Security model**: this feature reads and writes no data, so it has no authorisation rules of its
own. One rule matters: the `/design` route is mounted only when `import.meta.env.DEV` is true, so
the gallery is tree shaken out of the production bundle and is not a surface anyone can reach in
production. No compliance scope beyond the accessibility level itself.

**Configuration required**: none. No new environment variable and no third party credential. The
fonts are self hosted from an npm package, so there is no external host to configure and nothing for
feature 20's cookie consent to account for.

**Critical test scenarios**:

- Happy path: the same `Button` and `StatusPill` rendered inside each of the three surface shells produce three different measured heights and type sizes with no per surface code, verifies **AC-2**.
- Happy path: every declared token pair is computed by the contrast script and every one clears its threshold in both appearances, verifies **AC-1**, **AC-3**.
- Failure case: a colour token is edited to a value that fails 4.5:1, and `pnpm check` fails with the offending pair named, verifies **AC-1**.
- Failure case: with audio never unlocked and the device muted, a ready event still produces a visible badge and a live region announcement, verifies **AC-9**.
- Failure case: with `prefers-reduced-motion: reduce`, no base component animates and the skeleton does not pulse, verifies **AC-10**.
- Failure case: a status pill rendered in greyscale is still distinguishable by icon and word alone, verifies **AC-5**.
- Keyboard and assistive technology: the dialog is opened by keyboard, holds focus, closes on Escape, and returns focus to the button that opened it, verifies **AC-6**, **AC-7**.
- Keyboard and assistive technology: an invalid field announces its message and exposes `aria-invalid`, verifies **AC-8**.
- Auth/permission: a production build contains no `/design` route and no gallery code, verifies **AC-15**.
- Regression: navigating from a kitchen route back to `/` leaves the document at the `admin` density rather than stuck at kitchen, verifies **AC-2**.
- Regression: an icon inside a status pill measures three different sizes across the three surfaces, proving it rides the density layer rather than a fixed pixel prop, verifies **AC-2**, **AC-5**.

## Build plan

The project builds by Tracer Bullet, so the order below stands a thin thread through the whole
system first (tokens, one surface shell, a handful of real components, and all three enforcement
gates) and proves it before thickening. Steps 1 to 7 are that thread: after step 7 there is a
working, measured, contrast checked, axe checked design system visible in all three densities and
both appearances. Steps 8 onward add components to a frame already proved.

1. Install and clean up: `@fontsource-variable/geist` and `@fontsource-variable/geist-mono`, `lucide-react`, the shadcn CLI plus `class-variance-authority`, `clsx`, and `tailwind-merge`, and `vitest-axe` with `axe-core`. Delete the template leftover `web/public/icons.svg`. Declare the font faces with `font-display: swap` and a real fallback stack, satisfies **AC-12**.
2. Write the four token layers in `web/src/styles/index.css`: palette; semantic roles under `:root` and `[data-theme='dark']`, with the light, print, and forced colors blocks, the light set written under both its media query and `[data-theme='light']`; the three density blocks including `--border-width`; and the `@theme inline` bridge with both the colour mapping and the radius `calc()` chain. Replace the placeholder `--text-kitchen` and `--spacing-touch` the scaffold left behind. Add `<meta name="color-scheme" content="dark light">` to `index.html`, satisfies **AC-1**, **AC-2**, **AC-3**, **AC-16**.
3. Own `data-surface` in one pathname keyed effect in `root-layout.tsx`, defaulting to `admin` for `/` and anything unmatched, and build `SurfaceShell` with the skip link, the landmarks, its own content width token, and `ConnectionStatus` moved out of `root-layout.tsx`, satisfies **AC-2**, **AC-6**.
4. Build the thread's components: `Icon` (sized by class, never Lucide's `size` prop), `Button`, `Card`, `StatusPill`, `ElapsedTime`. Add the `status.*` keys to `common.json`, satisfies **AC-2**, **AC-5**, **AC-12**, **AC-14**.
5. Write `web/scripts/contrast-pairs.ts` as a list of variable name pairs tagged `text` or `interface`, and the script that resolves them by parsing `index.css` with `postcss` across every appearance scope. Wire it into the root `check` script and adjust any seed colour that fails, satisfies **AC-1**.
6. Raise `jsx-a11y` to its strict set in `web/eslint.config.js`, add the `no-restricted-syntax` rule refusing physical direction utilities in JSX class strings, add the shared axe helper with `color-contrast` disabled, and cover every component built so far in both appearances (through `data-theme`) and all three densities, satisfies **AC-4**, **AC-6**, **AC-13**.
7. Build the `/design` route, mounted only under `import.meta.env.DEV`, showing every token and component in both appearances and all three densities, satisfies **AC-15**, **AC-3**, **AC-11**.
8. Build `Field`, `Input`, and `Select` with the label, error, and describedby wiring, satisfies **AC-8**, **AC-14**.
9. Vendor and adapt the shadcn `Dialog`, satisfies **AC-7**.
10. Build `LiveRegion` as the one announcement mechanism, then `Toast` and `Alert` on top of it, including the audio unlock on first interaction, satisfies **AC-9**.
11. Build `Skeleton` and `EmptyState`, and confirm the reduced motion rule holds for both, satisfies **AC-10**.
12. Build `DataTable` with table semantics, `aria-sort`, and keyboard reachable rows, satisfies **AC-4**, **AC-6**.
13. Sweep every component for logical properties, checking the rule added in step 6 catches what it should, satisfies **AC-13**.
14. Run the forced colors pass over the `/design` route and fix what disappears, satisfies **AC-11**.
15. Finish the print block against a real kitchen ticket and a real bill layout, satisfies **AC-16**.
16. Write `docs/design.md`: tokens, the scale per surface, the component list with states, and the accessibility level, satisfies **AC-17**.

## Consequences

**Positive**:

- One component serves three very different contexts, so features 12, 13, and 17 build screens rather than rebuild a look. The kitchen legibility requirement in feature 13's done when clause is satisfied by the token layer before feature 13 starts.
- Vendored shadcn components inherit the density and the palette without being edited, so the CLI stays usable for the whole life of the project.
- The accessibility level stops being a document and becomes three build failures. A colour edit that breaks contrast, markup that breaks a rule, and a component that breaks axe all fail before review.
- Every later feature's acceptance criteria can say "meets the baseline" and mean something checkable.
- Nothing in the database changes, so this feature can land in parallel with feature 6 and ahead of feature 7 without a merge conflict in the schema.

**Negative / tradeoffs**:

- Two appearances doubles the contrast surface, the axe runs, and the visual review. The light theme has to be genuinely designed and genuinely tested, not inverted, and it will be used less than the dark one, so it will rot faster unless the gates catch it.
- Overriding Tailwind's own scale names per surface is clever, and clever has a cost: an engineer reading `h-9` in a component has to know it means three different heights. This is documented in `docs/design.md` and in `web/AGENTS.md`, and it is still the thing most likely to confuse someone new.
- The cascade does not reach everything, so "no per surface code, ever" is a strong rule with three named exceptions: border widths, container widths, and Lucide's `size` prop. Each has a stated handling above, and each is a place someone can still get it wrong by reaching for the obvious utility.
- Geist has narrower language coverage than Inter. Feature 6 may need a fallback face for a language Geist does not cover, and that is a change to the font stack rather than a token edit.
- The first pass carries `DataTable` and `Toast`, which the thin order thread does not use. That is real speculative work against the project's Tracer Bullet approach, accepted knowingly to stop three later features each inventing their own.
- Two font families instead of one is a second file to subset, preload, and pay for on first load.
- The audio unlock is genuinely fragile across browsers and will need real device testing in feature 12. The design here limits the damage rather than removing it, because the visual alert never depends on it.

**Neutral**:

- The `/design` route is a new pattern in the codebase: a route that exists only in development. It needs the conditional mount to be obvious, or someone will eventually ship it.
- `docs/design.md` becomes a living document that has to be kept in step with the tokens. AC-17 makes drift a verification failure rather than a silent one.
- Restaurants cannot brand their screens. Revisit when feature 22 puts a customer facing QR menu in front of actual diners, which is a different audience with a different answer.

## Follow-up

- [ ] Five community skills were installed during this design and are not yet in a context file: `accessibility-compliance`, `wcag-audit-patterns`, and `accessibility` are project wide and belong in root `AGENTS.md` `## Agent skills`; `design-tokens` and `lucide-icons` are web only and belong in `web/AGENTS.md` `## Agent skills`. `/audit` or `/sync` owns writing them.
- [ ] `web/AGENTS.md` needs three new gotchas once this is built: that `data-surface` lives on the document element because of portals, that Tailwind's own scale names are redefined per surface, and that the `/design` route is development only.
- [ ] Feature 6 should confirm Geist covers the second language it adds, and pick a fallback face if it does not.
- [ ] Feature 13 owns the real late threshold per restaurant. `ElapsedTime` takes it as a prop so nothing has to be rewritten, but the threshold in `docs/design.md` is a placeholder until then.
- [ ] Playwright joins continuous integration at slice 1 per `AGENTS.md`. When it does, add a keyboard walk and a forced colors pass over a real screen, since both are checked by hand in this feature.
- [ ] The `pencil` MCP server is configured but failed to start this session (`ENOENT` on its binary). Worth fixing or removing from the configuration, since a broken server is noise on every session.
