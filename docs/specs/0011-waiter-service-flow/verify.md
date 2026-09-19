# Verify: waiter service flow · spec 0011 · updated 2026-09-19
_Steps derived from spec 0011 acceptance criteria and its Value sourcing table. `/check verify` runs these; `/test` locks the durable ones._

Run against a seeded database. Against Neon each send takes about 10 seconds, so the browser
steps are far quicker against the local container on port 5434.

Note: spec 0011 says a too long note or a bad void reason is refused with `422`. The build answers
`400 invalid` with a `fields` member, like every other field error in this API, by the engineer's
choice during `/develop`. The spec still owes that one word fix.

## UI / manual
- [x] Sign in as the waiter → the Floor view is the landing screen, with Floor and Orders one tap apart at the top → AC-1
- [x] Open a table → its card on Floor says "Your table"; sign in as a second waiter → the same card shows the first waiter's name → AC-1, AC-4
- [x] Mark a dish ready on a table → its Floor card shows "1 ready"; nothing shows while it cooks → AC-1
- [x] Orders with three tables (one with ready food, one cooking, one idle) → ready first, oldest ready on top, then cooking by oldest round, then idle by opened time; a change on the kitchen reorders it with no refresh → AC-2
- [x] Turn on Mine on Floor → other waiters' occupied tables disappear, free tables stay; go to Orders → Mine is still on; reload → still on → AC-3
- [x] Second waiter taps Take over on the first waiter's table → it reads as theirs on both phones with no refresh; one `visit_taken_over` audit row → AC-4
- [x] Two waiters tap Take over on the same table at once → one wins, the other sees "Somebody else took that table over first" and the real owner → AC-4, AC-17
- [x] Send a second and third round to one table → each shows on the kitchen as its own ticket (Round 2, Round 3) and on the table and Orders with its own status → AC-5
- [x] Type "  no onions  " as a note → the kitchen shows "no onions"; a 140 letter Hindi note arrives whole and wrapped; 141 characters shows the counter error and blocks Send → AC-6
- [x] Two soups where one has a note ("One separately") → two lines on the kitchen ticket → AC-6
- [x] Build a basket, walk to Floor, come back, reload → the basket and notes are still there → AC-7
- [x] Build a basket, close the table from another phone, tap Send → the basket is cleared and a toast explains why → AC-7
- [x] Move a party with a saved basket → the basket follows to the new table → AC-7, AC-15
- [x] With the API slowed or the network cut after the request reaches it, tap Send again → one kitchen ticket only; the basket clears after the replay → AC-8
- [x] Waiter on Orders (not the table), chef marks one dish ready → the alert names table and dish, is announced, chimes once audio is unlocked, vibrates on Android; a second dish within 3 seconds joins without a second chime → AC-9
- [x] Another waiter's phone at the same moment → badge only, no alert, no sound → AC-9
- [x] Leave a ready dish unacknowledged for 2 minutes → the chime repeats; tap Acknowledge or open that table → it stops; serving the dish stops it on every phone → AC-10
- [x] Reload with food already ready → badges and Orders order at once, no chime on load, the alert shown silently → AC-11
- [x] Serve one ready dish while another on the round cooks → only that dish moves to served; "Serve all ready" serves every ready dish; with nothing ready it is refused `nothing_ready` → AC-12
- [x] Cancel a queued dish with "Other" and no words → the dialog asks for words; with words → the bill subtotal drops by the line total → AC-13
- [x] Try to cancel a served dish (from a stale screen) → `line_not_voidable` → AC-13
- [x] Kitchen screen after a cancel → the dish stays struck through, labelled Cancelled, no Done button; cancel every dish on a round → the ticket leaves the pass → AC-14
- [x] Move a party to a free table → floors and the kitchen ticket show the new label with no refresh; the move picker lists free tables only → AC-15
- [x] Close a table whose only dish was cancelled → "Nothing to charge", table freed, no bill number used; the next real bill takes the next number → AC-16
- [x] Switch to Hindi → every new word (Orders, Mine, reasons, alert, dialogs, conflicts) is Hindi → AC-19
- [x] Keyboard only: switch Floor and Orders, reach and press Acknowledge, open and complete the cancel dialog → AC-19

## Commands
- [x] `cargo test --manifest-path api/Cargo.toml --test waiter_service` → 21 passed (send key, notes, serving, voids, take over, moves, Orders read, empty close, isolation, races) → AC-4, AC-5, AC-6, AC-8, AC-12 to AC-18
- [x] `cargo test --manifest-path api/Cargo.toml --lib openapi` → every changed endpoint documented as "Waiters only." with 401 and 403 → AC-18
- [x] `pnpm --filter web test` → 425 passed, including the alert hook (grouping, reminder, acknowledge, silent on load), Orders sort, Mine, basket storage, kitchen cancelled and long note → AC-2, AC-3, AC-7, AC-9 to AC-11, AC-14
- [x] `pnpm locales && pnpm contrast && pnpm --filter web lint` → pass → AC-19
- [x] `pnpm e2e` (seeded database, running API, table free) → the two device scenario passes: note reaches the chef, alert on Orders, one dish served, a second round is its own ticket → AC-20
- [x] `pnpm sqlx:check && pnpm client:check` → no difference → build hygiene

## Value sourcing (one step per row)
- [x] Restaurant and actor: a waiter of restaurant B gets `404` on restaurant A's visit, line, round, and table ids → Actor
- [x] Responsible waiter on open: open a table as waiter X → `visits.responsible_staff_id = opened_by_staff_id = X`
- [x] Names on Orders and Floor: rename the waiter in admin staff → the new name shows on Floor and Orders with no refresh (the `staff` event reaches `['visit']`)
- [x] Mine "me": sign in as a different waiter on the same phone → Mine follows the new identity, not the old one
- [x] Mine storage: `localStorage['waiter.mineOnly']` is `true` or `false`; with storage blocked the switch still works for the page
- [x] Ready count: count of `ready` lines across the visit's rounds equals the badge
- [x] Sort: compare the Orders order with `readyAt`, then `sentAt` of rounds with a queued line, then `openedAt`
- [x] Ages: set the phone clock 20 minutes fast → Orders and table ages still read true (server time offset)
- [x] Newly ready: only lines on my tables not in this tab's seen set chime; reload seeds seen silently
- [x] Grouping: two dishes ready 1 second apart → one chime; 4 seconds apart → two chimes
- [x] Reminder: 120 seconds between chimes; `sessionStorage['waiter.acknowledged']` holds acknowledged line ids and drops ones no longer ready
- [x] Vibration: `navigator.vibrate([200, 100, 200])` on Android Chrome; nothing breaks on iOS Safari (skipped on real phones by the engineer on 2026-09-19, no devices here; in desktop Chromium the alert calls `navigator.vibrate([200,100,200])` once per chime)
- [x] Send key: `sessionStorage['waiter.basket.<visitId>']` holds lines and `clientKey`; the key stays the same across a failed send and changes after a successful one
- [x] Note length: 140 Hindi characters pass on the phone and in Postgres (`char_length`), 141 fails both
- [x] Replay answer: resend with the same key → `200` with the same round id, no `bill` event, no new lines
- [x] Serve one: response round status equals the recomputed status of all its lines
- [x] Void code and text: each of the four reason codes shows its translated word on the table screen
- [x] Void subtotal: `bills.subtotal` equals the sum of `line_total` over lines not voided, after every send and void
- [x] Take over expected: the request carries the `responsibleStaffId` the screen showed
- [x] Move choices: an archived or occupied table never appears in the move picker
- [x] Kitchen label after move: the ticket shows the new table label without a refresh
- [x] Empty close: the close response has `status: voided`, number `null`, zero figures
- [x] Kitchen cancelled: a `voided` line renders struck through and untappable
- [x] Words: every new string comes from `waiter`, `kitchen`, or `common`; the retired `round_not_ready` key is gone

## Acceptance-criteria coverage
- AC-1 Floor steps 1 to 3 · AC-2 Orders sort · AC-3 Mine · AC-4 take over and race · AC-5 rounds · AC-6 notes · AC-7 basket survives, discarded, follows a move · AC-8 retry and replay · AC-9 alert and other waiter · AC-10 reminder · AC-11 reload · AC-12 serving · AC-13 voids · AC-14 kitchen cancelled · AC-15 move · AC-16 empty close · AC-17 races (API suite) · AC-18 roles and isolation · AC-19 Hindi, keyboard, gates · AC-20 `pnpm e2e`
