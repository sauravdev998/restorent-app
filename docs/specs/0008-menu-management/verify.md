# Verify: Menu management · spec 0008 · updated 2026-09-12
_Steps derived from spec 0008 acceptance criteria and its Value sourcing table. `/check verify` runs these; `/test` locks the durable ones._

Setup: `pnpm migrate && pnpm db:seed`, then `pnpm dev:api` and `pnpm dev:web`. Sign in as the admin, the waiter, and the chef from `.env.example`, each in its own browser (or its own private window), all at once. The waiter opens a table and stays on its ordering screen throughout.

## UI / manual

### The admin menu screen
- [ ] Admin: header nav `Menu` → `/admin/menu` shows one page, categories as stacked sections, skeletons while it loads → AC-19
- [ ] Every dish row shows its diet mark, name, price in the restaurant's currency (₹, two decimals), an availability switch with the word beside it, Edit, and Remove → AC-2, AC-13
- [ ] A restaurant with no category shows "Your menu is empty" with `Add the first category`, which opens the category dialog → AC-1, AC-19
- [ ] `Add a category` → "Breads" → appears at the end of the admin list at once; the waiter sees nothing yet (no live dish in it) → AC-1
- [ ] `Add a dish to Breads` → name, price `40`, diet `Vegetarian` → at the end of Breads, switched on; the waiter's screen shows it and the Breads heading within about two seconds, no reload → AC-1, AC-2
- [ ] Rename "Breads" to "Breads and rice" → the waiter's heading changes within about two seconds → AC-3
- [ ] Edit a dish: change price, description, diet, and move it to another category → the moved dish lands at the end of the new category; the waiter's screen follows within about two seconds → AC-3
- [ ] The edit form's price box shows the stored price with the currency's own places (`340.00`, not `340.0000`) → Value sourcing (price decimals)

### Field errors (each beside its box, translated)
- [ ] Blank name → "This is required." → AC-14
- [ ] An 81 character dish name → "That is too long." → AC-14
- [ ] `Paneer Tikka ` beside a live `paneer tikka` → "That is already taken." on the name box → AC-14
- [ ] Price `12.345` in rupees → "That has more decimal places than your currency uses." → AC-14
- [ ] Price `-1` → "That cannot be below zero."; `abc` → "That is not a number."; `10000000000` → "That is more than this can hold." → AC-14
- [ ] Price `1,500` (Indian locale) is refused as not a number rather than read as 1.5 or 1500 → Value sourcing (price)
- [ ] Switch the interface to हिन्दी and repeat one refusal → the message is in Hindi → AC-14

### Stale forms
- [ ] Admin opens Edit on a dish; the chef switches that dish off from the kitchen Menu tab; the admin saves → refused with "Somebody changed this dish after you opened it…", the form reloads with the current values, and the dish stays off → AC-9, AC-15
- [ ] Two admin tabs rename the same category; the second save is refused with "Somebody changed this category…" and shows the current name → AC-15
- [ ] Edit a dish into a category that another tab has just removed → "That category has been removed. Choose another one." → AC-15

### Order
- [ ] Drag a dish by its handle with the mouse → the new order holds; the waiter's screen shows the same order → AC-4
- [ ] Drag a category by its handle → the waiter's category order follows → AC-4
- [ ] On a phone or touch emulation, long press a handle and drag → reorders; a quick swipe over a handle scrolls the page instead → AC-4
- [ ] Keyboard only: Tab to a dish handle, Space, Arrow Up, Space → reordered; a screen reader hears "Picked up …", "… moved to position 1 of 3.", "… dropped in position 1 of 3." → AC-4
- [ ] The same in हिन्दी → the announcements are in Hindi; Escape cancels and says so → AC-4
- [ ] A dish cannot be dragged into another category → AC-4
- [ ] Start a drag in one tab, add a dish to that category in another tab, drop → "The menu changed while you were moving things…", and the list shows the server's current order including the new dish → AC-5, Value sourcing (reorder refused)

### Remove and restore
- [ ] `Remove` on a dish → confirmation → the dish leaves the list and appears under Archived at once; it leaves the waiter's screen within about two seconds → AC-6
- [ ] `Remove the Mains category` while it holds dishes → the dialog warns up front, and confirming is refused with "That category still has dishes in it…" → AC-7
- [ ] `Put back` an archived dish whose category is live → the dialog defaults to its old category; it returns at the end, with its price, diet, and availability as they were → AC-8
- [ ] `Put back` a dish whose old category was removed too → the dialog says so and offers a live category → AC-8, Value sourcing (restore category)
- [ ] Archive "Dal makhani", add a new "Dal Makhani", then put the old one back → "Something on the menu already has that name…" → AC-8
- [ ] Archived rows show their diet mark and when they were removed → AC-13

### Availability, the kitchen tab, and the waiter
- [ ] Chef: `Menu` tab beside `The pass` lists every live dish by category with a switch per dish, no prices, kitchen sized → AC-10
- [ ] Chef switches a dish off → the waiter's screen greys it and its `+` refuses within about two seconds; the admin's switch flips too; switching it back on makes it orderable again → AC-9
- [ ] Setting the value a dish already has (via the API) succeeds and writes no audit row → AC-9, AC-17
- [ ] Waiter adds a dish to the basket; the chef switches it off → the basket flags the line ("Off now, cannot be sent"), announces it, and blocks Send until `Take out` → AC-12
- [ ] Admin removes a dish sitting in a waiter's basket → the basket flags it by the name it went in with → AC-12
- [ ] A send that races past the flag is refused whole with the translated message; no ticket reaches the kitchen; the basket then flags the line → AC-12
- [ ] With a dish on an open bill: reprice, rename, move, switch off, and remove it → the table screen's line, its price, and the bill subtotal read exactly as before → AC-11

### Diet mark and accessibility
- [ ] The marks differ by shape (circle, triangle, oval in a square) on the admin rows, archived rows, waiter screen, and chef tab → AC-13
- [ ] A screen reader names each mark ("Vegetarian", "Non vegetarian", "Contains egg"; in Hindi when reading Hindi) → AC-13
- [ ] Windows High Contrast (or Chrome's forced colors emulation): marks become the system text colour, shapes still differ; the switch thumb stays visible → AC-13, AC-9
- [ ] Print preview of the admin menu: marks print in black → AC-13

## Commands
- [ ] `cargo test --manifest-path api/Cargo.toml --test menu` → 15 passed (lifecycle, stale edit, stale rename, availability no op, category race in both orders, stale reorder, name uniqueness, restore clash, basket race, sent lines untouched, tenant isolation, audit rows) → AC-1 to AC-9, AC-11, AC-12, AC-14 to AC-17
- [ ] `cargo test --manifest-path api/Cargo.toml --lib` → includes `domain::menu` price and name rules, `every_menu_endpoint_is_in_the_document_and_says_who_may_call_it`, the combined role table, and the diet wire words → AC-14, AC-16, AC-13
- [ ] `pnpm --filter web test` → includes `admin-menu`, `kitchen-menu`, `table` basket, `basket`, `switch`, `diet-mark`, `format` (`parseDecimalInput`), `query-keys` → AC-9, AC-10, AC-12 to AC-15, AC-18, AC-19
- [ ] `pnpm contrast` → 180 pairs pass, the three diet tokens on four surfaces in dark, light, and print → AC-13
- [ ] `pnpm locales` → English and Hindi at parity → AC-19
- [ ] `E2E_TIMEOUT_MS=120000 pnpm e2e` (seeded database, running API, a free table) → the menu scenario (admin adds, chef switches off, admin removes, waiter sees each live), the keyboard reorder scenario (English and Hindi announcements), and the order thread → AC-2, AC-4, AC-6, AC-9, AC-10, AC-18, AC-20
- [ ] As a chef, `curl` any `/api/admin/menu` endpoint → `403`; as a waiter, `PUT /api/dishes/{id}/availability` → `403`; as an admin, `GET /api/menu` → `403`; signed out → `401`; another restaurant's dish id → `404` → AC-16
- [ ] Live schema: `\d public.dishes` shows `diet dish_diet not null` with no default, `version`, the length checks, `dishes_live_name_key` on `lower(name)`, and `dishes_category_order_idx` → AC-13, AC-14

## Value sourcing checks
- [ ] Restaurant: every request's restaurant is the session's; no body or path names one (tenant test above) → which restaurant
- [ ] Audit actor: each `audit_log` row for a menu change carries the signed in staff id → the audit actor
- [ ] Position: create two dishes, move one, restore one; each lands at the end of its category (`max + 1`) → position
- [ ] Availability on create is `true`; restore keeps what it was → availability
- [ ] Price decimals follow `restaurants.currency_decimals`: change the dev restaurant to a zero decimal currency in a scratch database and `12.5` is refused → price decimals
- [ ] Version: every change bumps it except a reorder (reorder, then rename with the version loaded before the reorder → accepted) → whether the copy is current
- [ ] Reorder refusal shows the refetched server order, never the held one → the order shown afterwards
- [ ] Diet mark colours come from the three tokens; forced colours fall back to `CanvasText` → the diet mark's colour
- [ ] Kitchen tab language is the restaurant's default, not the chef's personal one → kitchen Menu tab language
- [ ] Every refused request is shown from its code, never the API's English message → the sentence shown

## Acceptance-criteria coverage
- AC-1 category create: admin screen steps, `menu` integration tests · AC-2 dish create: admin screen, e2e · AC-3 rename and edit: admin screen, integration · AC-4 drag and drop: order steps, keyboard e2e · AC-5 stale reorder: order steps, integration · AC-6 remove: remove steps, e2e · AC-7 not empty and the race: remove steps, integration race test · AC-8 restore: remove steps, integration · AC-9 availability: kitchen steps, e2e, integration · AC-10 kitchen tab: kitchen steps, Vitest · AC-11 sent lines untouched: waiter step, integration · AC-12 basket: waiter steps, Vitest, integration · AC-13 diet mark: accessibility steps, contrast, Vitest · AC-14 field rules: field error steps, domain unit tests · AC-15 stale forms: stale steps, integration, Vitest · AC-16 roles: curl step, OpenAPI test, role unit tests · AC-17 audit rows: integration audit counts · AC-18 events: live steps throughout, fan out unit tests · AC-19 screen and translations: admin screen steps, locales, axe · AC-20 two browsers: `pnpm e2e`
