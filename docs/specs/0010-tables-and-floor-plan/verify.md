# Verify: Tables and floor plan · spec 0010 · updated 2026-09-16
_Steps derived from spec 0010 acceptance criteria and its Value sourcing table. `/check verify` runs these; `/test` locks the durable ones._

Setup: `pnpm migrate && pnpm db:seed`, then `pnpm dev:api` and `pnpm dev:web`. Sign in as the admin, the waiter, and the chef from `.env.example`, each in its own browser (or private window), all at once. The waiter stays on the floor screen unless a step says otherwise. In the shared development database, tables 1, 3, and 4 are occupied and table 2 must stay free for the order thread run: use new tables for any step that opens one, and free them afterwards (send a dish, mark it done and served, close the bill).

## UI / manual

### The admin floor screen
- [x] Admin: header nav `Tables` → `/admin/floor` shows one page, skeletons while it loads, the `Main room` group with tables 1 to 4 → AC-1
- [x] Each table row shows its label, its seats when recorded, and an occupied mark (word and icon) on 1, 3, and 4 → AC-1, AC-14
- [x] On a fresh throwaway restaurant with no tables, the page shows an empty state whose action opens the add table dialog → AC-1
- [x] `Add a section` → "Terrace" → appears at the end at once; the waiter's floor shows no Terrace heading yet → AC-2, AC-15
- [x] `Add a table` → label `P1`, seats `4`, section Terrace → lands at the end of Terrace; the waiter's floor shows a Terrace heading with `P1` and "4 seats" within about two seconds, no reload → AC-3, AC-15
- [ ] `Add a table` with no section → a no section group appears first on both screens → AC-1, AC-3

### Adding a range
- [x] `Add several` → prefix `P`, 2 to 6, seats 2, Terrace → the preview reads `P2` … `P6`; saving adds five tables in number order at the end of Terrace → AC-4
- [x] Prefix `Bar ` (with the space), 1 to 2 → `Bar 1`, `Bar 2` → Value sourcing (range labels)
- [x] With `P3` live, add `p1` to `p5` → refused, the form lists `p3`, and no table is created → AC-4
- [ ] 1 to 51 → "too many"; 5 to 2 → "before start"; 0 to 3 → "too small"; 1 to 1000 → "too large"; prefix `Rooftop bar ` with 1 to 10 → "too long" on the prefix → AC-4, AC-12

### Field errors (each beside its box, translated)
- [x] Blank label → "This is required." → AC-12
- [x] A 13 character label → "That is too long." → AC-12
- [x] `t1` when a live `T1` exists (in any section) → "That is already taken." → AC-12
- [x] Seats `0` → too small; `51` → too large; `2.5` → not a number (refused before sending) → AC-12
- [ ] A 41 character section name, and `terrace` beside a live `Terrace` → too long, already taken → AC-12
- [x] Switch the interface to हिन्दी and repeat one refusal → the message is in Hindi → AC-12, AC-19

### Editing
- [ ] Edit `P2`: label `P20`, seats 6, section none → it lands at the end of the no section group; the waiter's floor follows within about two seconds → AC-5
- [ ] Rename Terrace to "Garden" → the waiter's heading follows within about two seconds → AC-5, AC-18
- [x] Waiter opens a new table `E1`; admin renames it `E2` → the waiter's table screen header and the chef's ticket (after a dish is sent) show `E2` within about two seconds → AC-5, AC-18
- [x] Two admin tabs edit the same table; the second save → "Somebody changed this table…", the form shows current values → AC-13
- [x] Two admin tabs rename the same section; the second save is refused the same way → AC-13
- [ ] Edit a table into a section another tab has just removed → "That section has been removed. Choose another one." → AC-13

### Order
- [x] Drag a table by its handle with the mouse → the new order holds; the waiter's floor shows the same order → AC-6
- [ ] Drag a section → the waiter's section order follows → AC-6
- [x] On touch emulation, long press a handle and drag → reorders; a quick swipe scrolls instead → AC-6
- [ ] Keyboard only: Tab to a handle, Space, Arrow Up, Space → reordered, with the announcements heard (and in Hindi when switched); Escape cancels → AC-6
- [x] A table cannot be dragged into another group → AC-6
- [x] Start a drag in one tab, add a table to that group in another, drop → "The floor changed while you were moving things…", and the list shows the server's order including the new table → AC-7

### Occupancy and removal
- [x] Waiter opens a new table `E3` → the admin's `E3` row shows the occupied mark within about two seconds, no reload → AC-14
- [x] Admin removes `E3` → confirmation → refused with "Someone is sitting at this table. Close it first." `E3` stays → AC-8
- [x] Free `E3` (send, done, served, close) → the mark clears within about two seconds; removing it now works; it moves to Archived at once and leaves the waiter's floor within about two seconds → AC-8, AC-14
- [x] An old closed bill and the table screen history for `E3` still show `E3` → AC-8
- [x] Remove Garden while it holds tables → the dialog warns up front, and confirming is refused with "That section still has tables in it…" → AC-9
- [x] Remove every Garden table, then Garden → Garden moves to Archived with its tables listed under it → AC-9, AC-11

### Restore
- [ ] `Put back` a table whose section is live → the picker defaults to that section; the table returns at the end with its label and seats → AC-10
- [x] `Put back` a table whose section was removed → the picker defaults to No section → AC-10
- [ ] Add a new live `p4`, then `Put back` the archived `P4` → "A table already has that label…" → AC-10
- [ ] `Put back` Garden → the dialog lists its archived tables, all ticked; with the clashing `P4` still ticked, confirming is refused and lists `P4`, nothing restored → AC-11
- [ ] Untick `P4` and confirm → Garden returns at the end with its other tables in their old order; the waiter's floor shows them within about two seconds → AC-11
- [ ] `Put back` a section while another live section has its name → refused with the name taken message → AC-11

### The waiter's floor
- [x] Seats show on each table card that has them → AC-15
- [x] A section whose tables are all removed shows no heading → AC-15
- [x] On a throwaway restaurant with no tables, the waiter sees "No tables yet. Ask your admin to add them." → AC-15

### Accessibility and text
- [x] axe passes on `/admin/floor`, every dialog, and the waiter's floor; the occupied mark is not colour alone → AC-14, AC-19
- [x] `pnpm check` passes, including the English and Hindi key parity and the contrast script → AC-19
- [ ] A real screen reader pass is skipped for this feature, by the engineer's choice; the announcement tests stand in → AC-6

## API / automated
- [x] Integration: archive a table and `open_visit` it in two concurrent transactions, both orders; never an open visit on an archived table → AC-8
- [x] Integration: archive a section while another transaction creates, moves, or restores a table into it; never a live table in an archived section → AC-9
- [x] Integration: a range whose last insert hits a unique violation from a concurrent create rolls back whole and answers `409 labels_taken` with that label (read in a fresh transaction); the same for a section restore → AC-4, AC-11
- [x] `labels` lists clashes in number order for a range and in previous order for a restore → AC-4, AC-11
- [ ] Restoring a section with two tables writes three audit rows and three notifies → AC-17
- [x] Each create, edit, rename, remove, and restore writes exactly one audit row (one per table for a range); a reorder writes none → AC-17
- [x] Each floor write emits `dining_table` or `table_section` inside its transaction; the fan out test covers the new rows → AC-18
- [x] A waiter or a chef on any `/api/admin/floor` endpoint → `403`; an admin on `GET /api/floor` → `403`; signed out → `401`; another restaurant's id → `404`; every endpoint in the OpenAPI document → AC-16
- [x] `pnpm e2e` runs `web/e2e/floor.spec.ts`: create a uniquely labelled table, the waiter sees it, remove it, the waiter loses it, no reload, no table opened → AC-20
- [ ] `pnpm sqlx:check` and `pnpm client:check` pass after regeneration → Build plan task 7
