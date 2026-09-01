# Verify: design system and accessibility baseline · spec 0004 · updated 2026-09-02

_Steps derived from spec 0004 acceptance criteria. `/check verify` runs these; `/test` locks the durable ones._

Start the app first: `pnpm dev:web`, then open `http://localhost:5173/design` unless a step says otherwise.

## Commands

- [ ] `pnpm contrast` → passes, and the count it prints is 144 pairs across 3 appearances → AC-1
- [ ] Edit `--console-400` in `web/src/styles/index.css` to `#4a5260`, run `pnpm contrast` → fails, exits non zero, and names `--muted-foreground on --background` and `--status-served` with their measured ratios. Put the value back → AC-1
- [ ] `pnpm --filter web test` → passes. Every base component's axe run covers both appearances and all three densities, six renders each, zero violations → AC-4
- [ ] `pnpm --filter web lint` → passes → AC-4, AC-13
- [ ] Add `className="ml-2 text-left"` to any component, run `pnpm --filter web lint` → fails with the logical property message. Remove it → AC-13
- [ ] `pnpm --filter web build`, then `grep -r "render into a portal on the document" web/dist/assets/` → no match, and no separate gallery chunk is emitted → AC-15
- [ ] `grep -rE "\b(ml|mr|pl|pr)-|text-(left|right)|border-[lr]\b" web/src --include=*.tsx` → no matches outside tests → AC-13
- [ ] `grep -nE "margin-(left|right)|padding-(left|right)|(^|[^-])(left|right):" web/src/styles/index.css` → no matches → AC-13

## UI / manual

### Density and the token cascade

- [ ] On `/design`, switch Density between Admin, Waiter, and Kitchen → every panel rescales: type, spacing, borders, icons, and control heights, with no component containing per surface code → AC-2
- [ ] In the console on `/design`, measure a `Button` under each `data-surface` → height is 36px / 45px / 72px and body text is 14px / 16px / 28px, all at or above the AC-2 minimums of 32 / 44 / 72 and 14 / 16 / 28 → AC-2
- [ ] Measure a `StatusPill`'s icon under each density → three different sizes (16 / 20 / 32px), proving it rides the density layer rather than a fixed pixel prop → AC-2, AC-5
- [ ] Measure the small `Button` (`size="sm"`) on the waiter and kitchen surfaces → at least 44px and 72px, because `target-min` lifts it even though `h-8` alone would be smaller → AC-2

### Value sourcing (one per row of the spec's table)

- [ ] Visit `/kitchen` → `document.documentElement.dataset.surface` is `kitchen`. Visit `/` → it is `admin`. Visit `/nonsense` → it is `admin` → AC-2
- [ ] From `/kitchen`, navigate back to `/` → the density is `admin`, not stuck at `kitchen` → AC-2
- [ ] Open a `Dialog` from the kitchen surface → the dialog, which renders into a portal on `document.body`, is at kitchen density, because the attribute is on the document element rather than a wrapper → AC-2
- [ ] Set the operating system to light, reload → the light theme renders, `color-scheme` is `light`, and native scrollbars and form widgets follow. Set it to dark → the dark theme. Remove any preference → dark → AC-3
- [ ] On `/design`, confirm the two panels show dark and light at once, forced by `data-theme` → AC-3, AC-15
- [ ] Resize the window across the waiter surface → `SurfaceShell`'s content column comes from `--shell-width` (36rem on waiter, 80rem on admin, full width on kitchen), not from a `max-w-*` literal → AC-2
- [ ] Change the language in `web/src/locales/en/common.json` for one `status.*` key → the pill's word changes, proving no string is hardcoded → AC-14
- [ ] Render a `StatusPill` for each of `queued`, `ready`, `served`, `voided` → each maps to the colour and icon in the spec's table, and there is no status the `line_status` / `round_status` enums do not have → AC-5
- [ ] Render `ElapsedTime` with a `since` 90 seconds ago → shows `1:30`, `datetime="PT1M30S"`, and hidden text reading "1 minute, 30 seconds" → AC-12
- [ ] Render `ElapsedTime` with `lateAfterSeconds={900}` and a `since` 20 minutes ago → late colour, warning icon, and the word "Late" in its hidden text → AC-5
- [ ] Confirm money, elapsed times, and bill numbers use the mono face with tabular figures → a column of prices aligns on the decimal point and a ticking clock does not change width → AC-12
- [ ] With the browser tab never clicked or typed in, raise the ready alert → the badge appears and the live region announces; no sound, and nothing throws → AC-9
- [ ] Click once, then raise the alert again → the chime plays as well as the badge and the announcement → AC-9

### Keyboard and assistive technology

- [ ] Load any screen and press Tab once → the skip link slides into view with a visible focus ring and real padding; press Enter → focus lands on `<main>` → AC-6
- [ ] Tab through a whole screen → every interactive element is reachable in reading order, each has a visible ring meeting 3:1, and nothing focused is hidden behind a bar → AC-6
- [ ] Tab to a disabled `Button` → it is still reachable and announced as disabled, and clicking or pressing Enter does nothing → AC-6
- [ ] Open the dialog on `/design` by keyboard → focus moves inside, Tab and Shift+Tab cycle within it, Escape closes it, and focus returns to the button that opened it → AC-6, AC-7
- [ ] With a screen reader, open the dialog → it is announced with its title and its description → AC-7
- [ ] Focus the invalid field on `/design` → the control exposes `aria-invalid`, its `aria-describedby` names both the hint and the error, and the error is announced when it appears → AC-8
- [ ] Show a toast → it announces politely, never takes focus, and its Dismiss button works from the keyboard → AC-9
- [ ] Sort a `DataTable` column by keyboard → the header is a real button, `aria-sort` changes, and the direction is announced → AC-4, AC-6
- [ ] With a screen reader on a `DataTable` row → it reads the row header as well as the cell, so you know which row you are on → AC-4

### Appearance edge cases

- [ ] Set `prefers-reduced-motion: reduce` → no base component animates, the skeleton does not pulse, and the skip link appears without sliding → AC-10
- [ ] Turn on Windows high contrast (or Chrome's forced colours emulation) and open `/design` → every card, button, input, and pill keeps a visible boundary; the ghost button keeps an edge; the disabled button reads as `GrayText`; the skeleton keeps a border; the focus ring is still visible; each status is still distinguishable by icon and word → AC-11
- [ ] Print preview `/design`, and a kitchen ticket and a bill layout once they exist → ink on white, borders visible, no dark fills, nothing prints as a black rectangle → AC-16
- [ ] View any status pill in greyscale → every status is still distinguishable by icon and word alone → AC-5
- [ ] Set `dir="rtl"` on the document → the layout mirrors with no component edited → AC-13

### The gallery and the documentation

- [ ] `/design` renders every colour role, every status, the type scale, and every base component in every state → AC-15
- [ ] Read `docs/design.md` against the running app → the tokens, the per surface scale, the component list with states, and the accessibility level all match what is built → AC-17

## Acceptance-criteria coverage

- AC-1 contrast gate · `pnpm contrast` passes, and the deliberate failure step proves it fires
- AC-2 three densities from one component · measured heights, type sizes, icon sizes, and the surface attribute steps
- AC-3 dark default, real light theme · operating system preference steps and the side by side gallery
- AC-4 axe clean everywhere · `pnpm --filter web test` and the `DataTable` steps
- AC-5 colour is never alone · status pill, greyscale, and late emphasis steps
- AC-6 keyboard and focus · skip link, tab order, disabled button, and dialog steps
- AC-7 dialog behaviour · the keyboard dialog step and the screen reader announcement step
- AC-8 form labelling and errors · the invalid field step
- AC-9 the alert always gets through · the muted and unlocked audio steps, plus the toast step
- AC-10 reduced motion · the reduced motion step
- AC-11 forced colours · the high contrast step
- AC-12 mono tabular figures · the `ElapsedTime` and money alignment steps
- AC-13 logical properties only · the lint rule step, the two greps, and the `dir="rtl"` step
- AC-14 every string translated · the locale edit step
- AC-15 development only gallery · the production build grep and the gallery render step
- AC-16 print is ink on white · the print preview step
- AC-17 `docs/design.md` matches · the documentation read through
