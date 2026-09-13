# Verify: staff accounts · spec 0009 · updated 2026-09-13

_Steps derived from spec 0009 acceptance criteria and from its Value sourcing table. `/check verify` runs these; `/test` locks the durable ones._

A note on the last group. The Value sourcing steps are the ones worth running slowly: the design time
gate only says a source was named, and these are what say the named source is the one the code
actually reads. Each varies the input that breaks if it is wrong.

## UI / manual

Sign in as an admin at `/admin/staff` unless a step says otherwise.

- [ ] Add somebody with a name, an address, a password, and a role → the dialog turns into a hand over panel showing the address and the password together, and says they must change it at first sign in → AC-1, AC-18
- [ ] In that dialog press "Suggest a strong one" → a 16 character password appears and the field becomes readable in the same tap → AC-18
- [ ] Press "I have handed these over" → the dialog closes and the list shows the new person, "Never" under last signed in, and "Has not chosen a password yet" → AC-1, AC-6, AC-18
- [ ] Add somebody with an address that already has an account → `already_taken` appears beside the address box and nobody is created. Repeat with the address of a deactivated person, and with an address at another restaurant → all three read identically → AC-2
- [ ] Add somebody with a blank name, a malformed address, and a nine character password at once → all three boxes are marked in one response → AC-3
- [ ] Sign in as the person just created → the change screen appears, with no other screen drawn on the way and no application shell around it → AC-4, AC-21
- [ ] On that screen, give the wrong current password → the box says so, the screen stays, and they still owe a change → AC-5
- [ ] Give the right one → they land on their role's surface and the flag is cleared → AC-5
- [ ] As an admin, rename somebody → the list shows the new name and that person stays signed in wherever they were → AC-7
- [ ] Change somebody's role → their next tap on their own device is refused and they are sent to sign in → AC-8
- [ ] Change somebody's role to the role they already hold → it succeeds, the version does not move, and they stay signed in → AC-8
- [ ] Open the rename dialog, change that person from a second admin's browser, then save the first → `staff_changed`, the dialog shows the current name and version, and saving again works → AC-14
- [ ] Reset somebody's password → the panel shows the new one once, they are signed out everywhere, and the list shows them owing a change again → AC-9
- [ ] Switch off a waiter who has a party at a table and an unserved round → it succeeds, the table stays open and still names them, and their next request is refused → AC-10
- [ ] Bring that account back → the role, the password, and whether they owed a change are all exactly as they were → AC-11
- [ ] Try each of role change, reset, and switch off on your own row → the buttons are not offered, and the API refuses all three with `cannot_act_on_self` → AC-12
- [ ] With one admin in the restaurant, demote or switch off that admin → `last_admin`, and the restaurant keeps an admin → AC-12
- [ ] Aim an edit with a stale version at somebody who has been switched off → `staff_inactive`, not `staff_changed` → AC-14
- [ ] Bring back an account that is already active → `staff_inactive` → AC-13
- [ ] Read the whole screen with a keyboard only, then with a screen reader → every row action names the person it acts on, the deactivated section is its own named region, and the tables carry captions → AC-21
- [ ] Switch the interface to Hindi → every string on the screen and in every dialog is translated, with no English left → AC-21

## Commands

- [ ] `cargo test --manifest-path api/Cargo.toml --test staff -- --test-threads=1` → 11 pass → AC-1, AC-2, AC-6 to AC-14, AC-16, AC-17
- [ ] `cargo test --manifest-path api/Cargo.toml --test isolation -- --test-threads=1` → passes, run as `app_api` → AC-16
- [ ] `cargo clippy --manifest-path api/Cargo.toml --all-targets -- -D warnings` → clean → AC-3
- [ ] `pnpm --filter web test` → passes
- [ ] `pnpm sqlx:check && pnpm client:check` → no difference, so the committed cache and client match the code → AC-15
- [ ] `pnpm --filter web locales` → parity holds across English and Hindi → AC-21
- [ ] Sign in as a waiter and as a chef, then call all seven `/api/staff` endpoints → `403` from every one, from both → AC-15
- [ ] While a password is owed, call `/api/floor`, `/api/menu` and `/api/events` → `403 password_change_required` from each; call `/api/staff` as that same person → plain `403`, because the role is checked first → AC-4, AC-15
- [ ] Open `/api/events` as somebody settled, have an admin reset their password, and hold the stream → it closes within one 15 second heartbeat → AC-4, AC-10
- [ ] On a **fresh** database: `pnpm migrate && pnpm db:seed`, then sign in as the seeded waiter and the seeded chef → neither meets a password form. Run `pnpm db:seed` again → nothing changes → AC-20
- [ ] `pnpm e2e` after that seed → the two device scenario passes unchanged → AC-20
- [ ] Read `audit_log` for one person who has been through all six actions → six rows, one per action, each naming the acting admin, and no password or hash in any value → AC-17

## Value sourcing

One per row of the spec's table, each varying the thing that breaks if the source is wrong.

- [ ] Create somebody while signed in as an admin of restaurant A, then read the row as an admin of restaurant B → `404`. The restaurant came from the session and there is no way to send one → AC-16
- [ ] Type an address with capitals → the list and the hand over panel show exactly what was typed, and signing in with the lowered form works → AC-1
- [ ] Create two people at the same instant with the same address → exactly one is created, and the other reads `already_taken` from the index rather than from a pre check → AC-2
- [ ] Create somebody and read the row directly → `must_change_password` is true and `version` is 1, neither of them sendable by the caller → AC-1
- [ ] Change the restaurant's timezone to one twelve hours away, then reload the staff screen → every "last signed in" moves by twelve hours, so it is formatted with the restaurant's zone and not the device's → AC-6
- [ ] Change the restaurant's formatting locale to `hi-IN` → the same timestamps are written in that locale's numerals and order → AC-6
- [ ] Give two people names differing only in letter case, and two more differing only after a space → the list order is stable across reloads and identical in two browsers, because it is computed in SQL → AC-6
- [ ] Create somebody, never sign in as them, and read their row → `last_sign_in_at` is null and the screen says "Never" as a translated word, not an English sentence → AC-6
- [ ] Have two admins demote each other at the same instant → exactly one succeeds, and the count was taken under the restaurant's advisory lock → AC-12
- [ ] Issue a deactivate and a reactivate at the same instant → the row ends in one of the two intended states and the loser reads `staff_inactive` → AC-13
- [ ] Change a role and immediately make a request on that person's other device → `401`, because every session row of theirs was revoked in the same transaction → AC-8
- [ ] Force the revocation statement to fail and confirm the role change rolls back with it → the change and its revocation commit together or neither does → AC-8
- [ ] Read the audit row for a rename → `before` and `after` carry the same four fields, and the display name is the trimmed value that was stored → AC-3, AC-17
- [ ] Sign in as somebody who owes a change and read `GET /api/me` → `staff.mustChangePassword` is true on the one bundle every screen reads → AC-19
- [ ] Provoke each of the four new conflicts and read the response → each carries its own code, and the screen shows a translated sentence rather than the English `message` → AC-14

## Acceptance-criteria coverage

- AC-1 covered by create, list, and the row read · AC-2 by the three taken address steps · AC-3 by the field error step, clippy, and the trimmed name row
- AC-4 by the change screen step and the three refused endpoints · AC-5 by the wrong and right current password steps · AC-6 by the list, the timezone, the locale, and the order steps
- AC-7 by the rename step · AC-8 by the role change, the no op, and the revocation steps · AC-9 by the reset step
- AC-10 by the mid service switch off and the stream close · AC-11 by the bring back step · AC-12 by the self action, last admin, and racing demotion steps
- AC-13 by the already active and the racing deactivate steps · AC-14 by the stale save, the fixed order, and the conflict code steps
- AC-15 by the waiter and chef refusal step and `client:check` · AC-16 by the cross restaurant read and the isolation suite
- AC-17 by the audit log read and the rename row · AC-18 by the create dialog steps · AC-19 by the bundle step
- AC-20 by the fresh database seed and `pnpm e2e` · AC-21 by the outside the shell, keyboard, screen reader, and Hindi steps
