# Verify: kitchen display · spec 0012 · updated 2026-09-21

_Steps derived from spec 0012 acceptance criteria and its Value sourcing table. `/check verify` runs these; `/test` locks the durable ones._

Run against a seeded database, with a waiter's browser and a chef's browser open at once. Against
Neon each send takes about ten seconds, so the browser steps are far quicker against the local
container on port 5434.

Two notes on what the build settled where the spec left the wording open:

- A crossed threshold pair is refused on `kitchenLateAfterSeconds` with `before_start`, whichever of
  the two the edit named. The pair is a range and its end coming before its start is what went
  wrong, and both boxes sit together on the settings screen.
- The two audit actions are stored flat, `line_ready_undone` and
  `restaurant_kitchen_thresholds_changed`, rather than with the dots the spec writes them with. The
  `action` column exists to be matched on and the other thirty one are flat.

## UI / manual

- [ ] Chef signs in → the pass has a Cooking area and a Ready to collect area, both headed → AC-1, AC-6
- [ ] Waiter sends a round while the chef's pass is open, untouched → the ticket appears with no refresh, marked New, with the kitchen chime once audio is unlocked → AC-1, AC-14
- [ ] Send three rounds a minute apart → they read oldest sent first; send a fourth → it lands last and the first three do not move → AC-2
- [ ] Watch one ticket past ten minutes → the clock turns amber with its own icon; past fifteen → red with the late icon and the word "Late" in its hidden text → AC-3
- [ ] Tap Done on one dish of a two dish ticket → only that dish moves; the other still shows Done and the ticket still reads Cooking → AC-4
- [ ] Tap Done on the last cooking dish → the ticket flips to Ready by itself, with no second tap, and leaves the Cooking area → AC-5
- [ ] The same ticket now in Ready to collect → it says Plated and its clock counts from when it was plated, not from when it was sent; it stays there until the waiter marks it served → AC-6
- [ ] Tap "Back on" on a plated dish → it returns to Cooking, and a ticket that had gone Ready comes back to the Cooking area with it → AC-7
- [ ] After that undo, read `audit_log` → one `line_ready_undone` row naming the chef, the dish, and the round → AC-8
- [ ] Tap "All done" on a four dish ticket → one request, every dish plated, the ticket flips once → AC-9
- [ ] Tap "Ran out" on a cooking dish → the dish is cancelled, the waiter's table screen shows the new subtotal with no refresh, and `audit_log` names the chef as the actor → AC-10
- [ ] The cancelled dish → it is in a separate Cancelled strip at the foot of its ticket, out of the cook list, with its reason in words, and it flashed once as it happened → AC-12
- [ ] Send a 140 character note (try Hindi) → it reaches the pass whole, wrapped over as many lines as it needs, never truncated and never clipped → AC-13
- [ ] Reload the pass with nine tickets already cooking → no chime, no New marks: a reload is not nine arrivals → AC-14
- [ ] On a freshly loaded pass nobody has touched → "Sound is off" is shown at kitchen size; tap anywhere → it goes, and the next ticket chimes. Mute the tablet → the New mark and the announcement still fire → AC-15
- [ ] Scroll to the bottom of a long pass, have the waiter send a round → the scroll does not move, and a "New work above" marker appears with "Go to the top" → AC-16
- [ ] Stop the API → the pass shows a full width "This screen is not live" band and the tickets dim; every button still works. Start the API → the band clears and the queue refetches → AC-17
- [ ] Leave the pass open on a tablet for longer than its screen timeout → the screen stays awake; walk to the Menu tab → the hold is released. On a browser with no wake lock → the pass still works → AC-18
- [ ] Admin opens Settings → a "When the kitchen screen warns" section with both thresholds in minutes; save 5 and 12 → the chef's pass turns amber at 5 minutes and red at 12 → AC-20
- [ ] Admin saves the warning alone, high enough to cross the stored late value → refused, naming the late box, nothing stored → AC-20
- [ ] Two admins open Settings, both save → the second is refused with "Somebody saved these settings first", the form reloads with the winner's values, and nothing of the first admin's is lost → AC-21
- [ ] Waiter signs in and opens `/kitchen` → sent to their own surface; a chef opens `/admin/settings` → sent to theirs → AC-22
- [ ] Switch the restaurant language to Hindi → every string on the pass is Hindi, including the Cancelled reasons, the two banners, and the sound prompt → AC-23
- [ ] Two chefs on two tablets tap the same dish at the same instant → one succeeds, the other reads "Another chef marked that dish first" and its screen refetches to the real state → AC-24
- [ ] **Three metre legibility, the measurement spec 0005 asked this feature to make**: stand three metres from the pass on the real kitchen screen → the table label, the dish names, and the clock are all readable, and the Done target is hittable with a gloved hand → AC-23, spec 0005 follow up

## Commands

- [ ] `pnpm migrate` → migration 0010 applies; `\d restaurants` shows the three columns and the three named check constraints → AC-3, AC-20
- [ ] `psql -c "SELECT indexdef FROM pg_indexes WHERE indexname IN ('order_rounds_queue_idx','order_rounds_ready_idx')"` → the queue index covers `status IN ('queued','ready')` and the ready index is on `(restaurant_id, ready_at)` → AC-19
- [ ] `cargo test --manifest-path api/Cargo.toml --test kitchen_display` → 18 pass, including both race tests → AC-7, AC-9, AC-20, AC-21, AC-24
- [ ] `cargo test --manifest-path api/Cargo.toml` → the whole suite, with the widened void and the required version not breaking spec 0011 → AC-11, AC-21
- [ ] `cargo clippy --manifest-path api/Cargo.toml --all-targets -- -D warnings` → clean → build hygiene
- [ ] `pnpm --filter web test` → the pass's own 24 tests pass, including the not live band, the hidden count, the amber band, and the undo → AC-3, AC-7, AC-9, AC-12, AC-17, AC-19
- [ ] `pnpm --filter web test kitchen-home` → the axe run inside it reports no serious or critical violation at kitchen density in both appearances → AC-23
- [ ] `pnpm sqlx:check && pnpm client:check` → no difference → build hygiene
- [ ] `pnpm e2e` → the two device scenario still passes with the Ready area in place → AC-1, AC-5

## Value sourcing (one step per row)

- [ ] Cooking age from `order_rounds.sent_at`: compare the clock on a ticket with `now() - sent_at` in Postgres → they agree within a second
- [ ] Ready age from `order_rounds.ready_at`: plate a ticket, wait a minute, compare its Ready area clock with `now() - ready_at` → they agree, and it is **not** counting from `sent_at`
- [ ] The server's clock, not the tablet's: set the tablet twenty minutes fast → every age on the pass still reads true (the `serverTime` offset)
- [ ] Amber from `restaurants.kitchen_warning_after_seconds`: change it in admin Settings → the response's `warningAfterSeconds` changes and the pass turns amber at the new time, with no code change
- [ ] Red from `restaurants.kitchen_late_after_seconds`: the same, and `DEFAULT_LATE_AFTER_SECONDS` in `elapsed-time.tsx` is never the value a real pass uses
- [ ] Hidden work from the count beyond the cap: open more than 120 rounds in one restaurant → the read returns 120 and `truncatedCount` above zero, and the pass says how many are not shown; a normal restaurant gets `0` and no banner
- [ ] Table label and round number: rename a table in admin → the ticket shows the new label with no refresh (the `dining_table` event reaches the kitchen key); the round number matches `order_rounds.sequence_no`
- [ ] Cancellation reason from `void_reason_code`: cancel with each of the four codes → the pass shows that code's translated words, and the free text appears only for `other`
- [ ] Which cards are new: this is the screen's own memory, not a column. Open two chef tablets, send one round → both mark it new; reload one → it marks nothing new while the other still shows the mark
- [ ] Who marked a dish ready, from the session: tap Done as chef A → `order_lines.ready_by_staff_id` is A's id and `ready_at` is set with it
- [ ] Undo actor from the session: undo as chef B a dish chef A marked → the audit row names B, and the line's `ready_by_staff_id` and `ready_at` are both `NULL`
- [ ] Undo action name: the audit row's `action` is `line_ready_undone`
- [ ] The version the admin screen submits: read `GET /api/me` → `restaurant.version`; save Settings → the response carries version + 1 and the screen holds it without a second read
- [ ] The other threshold when only one is sent: save the warning alone → `kitchen_late_after_seconds` is unchanged in Postgres, and the pair was still checked (try a crossing value and get refused)
- [ ] Threshold audit action: change a threshold → one `restaurant_kitchen_thresholds_changed` row whose `before` and `after` both carry all seven settings
- [ ] The chime itself: it is synthesised through the shared `AudioContext`, three descending notes, and is audibly different from the waiter's two rising ones. Play both back to back with the same device volume
- [ ] Whether sound is allowed: with audio locked the prompt shows and no chime plays; after one tap the prompt goes and the next ticket chimes
- [ ] Tenant scope on the hidden count: a chef of restaurant A never sees restaurant B's tickets, and `truncatedCount` never counts them either

## Acceptance-criteria coverage

- AC-1 live arrival · AC-2 ordering and a fourth ticket · AC-3 amber and red from the restaurant's own columns · AC-4 one tap, one dish · AC-5 automatic flip · AC-6 the Ready area and its own age · AC-7 undo, and the undo and serve race · AC-8 the undo audit row · AC-9 All done in one transaction · AC-10 the chef void and the bill · AC-11 a refused reason and a served dish (API suite) · AC-12 the Cancelled strip and its flash · AC-13 a whole 140 character note · AC-14 the chime and the New mark · AC-15 the locked audio prompt · AC-16 the held scroll and the marker · AC-17 the not live band and the dim · AC-18 the wake lock · AC-19 the cap and the hidden count · AC-20 the thresholds and the merge · AC-21 the stale refusal · AC-22 roles (OpenAPI document) and isolation (API suite) · AC-23 Hindi, axe at kitchen density, and the three metre measurement · AC-24 two chefs on one dish (API suite)
