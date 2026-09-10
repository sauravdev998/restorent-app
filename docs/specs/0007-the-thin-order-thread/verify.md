# Verify: the thin order thread · spec 0007 · updated 9 September 2026

_Steps derived from spec 0007's acceptance criteria and its **Value sourcing** table. `/check verify` runs these; `/test` locks the durable ones._

Everything below assumes `pnpm db:seed`, a running API, and the web app. The three
seeded accounts are in `.env.example`: `admin@`, `waiter@`, and `chef@example.test`,
all with the password `development-only-password`.

Two of the steps want two browsers at once. Use two profiles or one normal window
and one private window, not two tabs: the point is two sessions, and two tabs share
one cookie jar.

## UI / manual

### The floor and opening a table

- [x] Sign in as the waiter → you land on the floor, which lists every seeded table in section then position order (1, 2, 3, 4) → AC-1
- [x] Tap `Open table` on a free one → you land on that table's screen, and going back to the floor shows it occupied, naming you and the time it was opened → AC-1
- [x] With that table open in a second browser signed in as a second waiter, tap `Open table` on the same table → the refusal reads "Somebody has already opened that table. Its screen now shows where it stands.", never the API's English, and the floor redraws showing it occupied without a reload → AC-2, AC-13
- [x] The occupied table names whoever opened it, and any waiter can still tap `View` and act on it → AC-1

### The menu

- [x] The ordering screen lists both seeded categories with their dishes in position order → AC-3
- [x] Fish pakora appears, struck through and marked "Off tonight", and its `+` is disabled → AC-3
- [ ] As an admin, archive a dish, then reload the waiter's ordering screen → the archived dish is gone entirely, unlike the unavailable one → AC-3

### Sending, and the kitchen

- [x] Build a basket of two dishes and reload before sending → the basket is empty and nothing was written; no ticket exists → AC-4
- [x] Send the basket → exactly one ticket appears, numbered 1, with one line per basket entry at the right quantity → AC-4
- [x] With a chef signed in on a second device, watch the kitchen screen while the waiter sends → the ticket appears with no refresh and no tap, showing the table label, the round number, each dish with its quantity, and how long it has waited → AC-5
- [x] Send a second round on the same table → it appears below the first, oldest first → AC-6
- [x] Chef taps `Done` on one dish of a two dish ticket → only that dish changes and the ticket still reads "Cooking" → AC-7
- [x] Chef taps `Done` on the last dish → the ticket flips to "Ready" by itself → AC-7
- [x] Two chefs tap `Done` on the same dish at once → one wins, the other reads "Another chef marked that dish first. The ticket now shows where it stands." and the screen redraws to the truth → AC-13

### Back to the waiter

- [x] When the ticket goes ready, the waiter's screen (untouched) raises a badge naming the table and the round, and a screen reader announces the same sentence → AC-8
- [x] Dismiss the alert, then let another live event land on that table → the same round is not announced a second time → AC-8
- [x] Tap `Mark served` → every dish on the ticket reads "Served", the ticket leaves the kitchen screen without a refresh, and the waiter's screen updates too → AC-9
- [x] The table screen shows each round with its status, each dish with quantity and line total, and the bill's running subtotal → AC-10

### Ending the meal

- [x] With a dish still cooking, tap `Close the bill` → refused, and the sentence reads "A dish on this bill has not reached the table yet. Mark everything served first.", not the API's English → AC-12
- [x] With everything served, tap `Close the bill` → the screen shows the bill number and the figures the close wrote: subtotal, service charge if any, each tax, and total → AC-11
- [x] Back on the floor → the table reads free, with no refresh, and can be opened again straight away → AC-11

### When it breaks

- [x] Stop the API with both screens open → both show a band saying live updates have stopped, announced politely, and both stay usable → AC-16
- [x] With the API stopped, the screens still render what they have; restart it → on reconnect both catch up everything they missed with no reload → AC-16
- [x] Both new screens pass an axe pass, render at their own surface density (the kitchen's buttons are visibly larger than the admin's), and show no untranslated string; switch to Hindi and every word on both changes → AC-19

## Value sourcing

One step per row of the spec's **Value sourcing** table, each varying the input that
breaks if the value is read from the wrong place.

- [ ] Sign in as a waiter of a second restaurant → the floor, the menu, and the pass show only that restaurant's rows; no request anywhere carries a restaurant id → the session, and nothing else
- [ ] Open a table as waiter A, send as waiter B → the visit records A as opener and the round records B as sender → `Actor`'s staff id
- [x] Open a table, then check the floor → occupied; close it → free. Never both → the open `visits` row
- [x] Mark a dish ready on an occupied table → that table shows the ready marker on the floor → a round in `ready` on the open visit
- [ ] Reorder the seeded sections or tables by position → the floor's order follows, not the labels' alphabetical order → section then table position
- [ ] Set a dish unavailable → it stays on the menu, greyed; archive it → it disappears → `live_dishes`, which keeps one and drops the other
- [x] Edit a dish's price, then send it → the new line carries the new price; a line sent before the edit still carries the old one → the price copied at send
- [x] Open a table with a guest count and one without → the first records it, the second records null → the optional field on the open form
- [x] Send two rounds on one visit → they number 1 then 2, and a third on a different visit also starts at 1 → `max(sequence_no) + 1` within the visit
- [x] Send a round, then read the bill's subtotal before closing → it already includes that round → lines assigned at send
- [x] Set the device clock twenty minutes fast, then load the kitchen screen → a ticket sent one minute ago still reads about one minute → `server_time` and the offset, never the device's own reading
- [ ] Change the restaurant's formatting locale to `en-IN` and back → the grouping on every figure changes and the language does not → `formatting_locale`, not the interface language
- [ ] Close a bill, then change the restaurant's service charge → the closed bill still shows what it charged → the figures `close_bill` wrote
- [ ] Read a closed bill's money → it is written in the bill's own `currencyCode` and `currencyDecimals`, not today's restaurant setting → the bill's own copy
- [ ] Provoke each refusal in turn → each shows its own sentence, and switching to Hindi changes all of them → the `error` code mapped by `shared/api/error-message.ts`
- [x] Watch the network while a chef marks a dish → the waiter's browser refetches the visit and the kitchen queue, and does not refetch the menu or anything else → the fan out map

## Commands

- [x] `pnpm db:seed` → creates one section, four tables, two categories, six dishes with one unavailable, and the waiter and chef beside the admin → AC-17
- [x] `pnpm db:seed` again → says the same thing and duplicates nothing; the counts are unchanged → AC-17
- [x] `cargo test --manifest-path api/Cargo.toml --test order_thread` → 13 pass, covering the floor, the menu, the queue, the document, every refusal code, the snapshot rule, and tenant isolation → AC-1, AC-3, AC-4, AC-6, AC-9, AC-10, AC-12, AC-13, AC-14
- [x] `cargo test --manifest-path api/Cargo.toml --test concurrency -- --test-threads=2` → 7 pass, including two dishes on one ticket marked at once still leaving it ready → AC-7, AC-13
- [x] `pnpm --filter web test` → 252 pass, including the fan out map, the clock offset, and the alert firing once per round → AC-6, AC-8, AC-15
- [x] `pnpm e2e` → the two device scenario passes: the ticket arrives on the chef's screen with no reload, the alert reaches the waiter on both channels, and the bill closes with a number → AC-5, AC-8, AC-18
- [x] `pnpm check`, `pnpm sqlx:check`, `pnpm client:check` → all clean → AC-14, AC-19

Against a database that is not on the same network, `pnpm e2e` needs
`E2E_TIMEOUT_MS` raised; the reason is written in `web/playwright.config.ts`.

## Acceptance-criteria coverage

- **AC-1** floor, opening, live occupancy · covered by the floor steps and `the_floor_shows_every_table_and_who_is_sitting_at_the_occupied_ones`
- **AC-2** a table cannot be opened twice · the two browser step and `two_waiters_racing_for_a_table_leave_exactly_one_party_at_it`
- **AC-3** the menu keeps an unavailable dish and drops an archived one · the menu steps and `the_menu_keeps_an_unavailable_dish_and_drops_an_archived_one`
- **AC-4** one round, one line per entry, nothing written before sending · the send steps and `sending_a_ticket_puts_its_dishes_on_the_bill_at_once`
- **AC-5** the ticket appears live · the two device step and `pnpm e2e`
- **AC-6** the queue's contents, order, and honest ages · the kitchen steps, `the_kitchen_queue_is_the_work_left_oldest_first`, and `server-clock.test.ts`
- **AC-7** one dish at a time, the last one flips the ticket · the two `Done` steps and `the_last_dish_off_the_pass_makes_the_whole_ticket_ready`
- **AC-8** the alert, once per round, on both channels · the alert steps, `table.test.tsx`, and `pnpm e2e`
- **AC-9** serving moves every waiting dish and clears the pass · the serve step and `carrying_a_ticket_out_moves_every_dish_that_was_waiting`
- **AC-10** the table screen's rounds, lines, and running subtotal · the table screen step and `a_visit_reads_back_as_the_whole_meal_and_closes_with_a_number`
- **AC-11** closing writes the number and the figures and frees the table · the closing steps and the same test
- **AC-12** closing too early is refused and the sentence is translated · the refusal step and `every_closing_refusal_names_what_happened`
- **AC-13** one winner, and the loser is told what happened · the two racing steps, `every_service_refusal_names_what_happened`, and the concurrency suite
- **AC-14** roles hold on the server and reach the document · the second restaurant step, `none_of_the_new_reads_can_see_another_restaurants_service`, and `pnpm client:check`
- **AC-15** an event invalidates only what its entity feeds · the network step, `query-keys.test.ts`, and `use-live-events.test.tsx`
- **AC-16** a closed stream warns and leaves both screens usable · the two "when it breaks" steps
- **AC-17** the seed, and running it twice · the two `pnpm db:seed` commands
- **AC-18** one Playwright run, two contexts, the whole thread · `pnpm e2e`
- **AC-19** no literal strings, right density, accessibility gates pass · the accessibility step, `pnpm check`, and the axe cases in `table.test.tsx`
