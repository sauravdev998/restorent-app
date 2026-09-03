# Verify: accounts, restaurants, and roles · spec 0006 · updated 2026-09-04

_Steps derived from spec 0006 acceptance criteria and its Value sourcing table. `/check verify` runs
these; `/test` locks the durable ones._

Start from a clean database (`pnpm migrate`), then `pnpm db:seed`, `pnpm dev:api`, `pnpm dev:web`.
Two browsers, or one plus a private window, wherever a step says "second device".

## UI / manual

- [ ] Visit `/admin` signed out → land on `/sign-in?next=%2Fadmin`, and the admin screen never
      renders, not even for a frame (watch with the network throttled) → AC-20
- [ ] Register at `/register` with country India → land on `/admin` signed in, and the header shows
      the restaurant → AC-1, AC-3, AC-20
- [ ] Reload the page → still signed in, same name and restaurant, no second sign in → AC-6
- [ ] Register a second restaurant with the same email address → the address box says "already
      taken", nothing else is created, and the first restaurant is untouched → AC-2
- [ ] Register with a nine character password → the password box says "too short" and no account is
      created → AC-5
- [ ] Submit the registration form with the restaurant name blank → that box says "required", and
      the response is this product's error shape rather than the JSON extractor's → AC-2
- [ ] Sign out from `/account` → land on `/sign-in`, and the browser's cookie for `session` is gone
      → AC-13
- [ ] Sign in with the right address and a wrong password, then with an address nobody has → the two
      answers are word for word identical, and neither box is marked invalid → AC-4
- [ ] Sign in on a second device, sign out on the first → the second is still working → AC-13
- [ ] Change your password at `/account` with the wrong current password → "that is not right"
      beside the current password box, and nothing changes → AC-14
- [ ] Change it with the right one → this screen stays signed in, the second device is signed out on
      its next action → AC-14
- [ ] As an admin, edit the restaurant's name, address, timezone, language, and number format at
      `/admin/settings` → all five stick after a reload, and no currency field is offered → AC-15
- [ ] Set the timezone to `Not/AZone` → that box is refused, and the stored value is unchanged →
      AC-15
- [ ] Set your own language to Hindi at `/account` → the screen turns Hindi and stays Hindi after a
      reload on a different device → AC-16
- [ ] Set it back to "whatever the restaurant uses" → the screen follows the restaurant's language
      again → AC-16
- [ ] Walk to `/kitchen` as an admin → redirected to `/admin` rather than shown a refusal → AC-20
- [ ] With the kitchen screen open, revoke that session (`UPDATE sessions SET revoked_at = now()`)
      → the connection badge goes to closed within 15 seconds and the browser lands on `/sign-in`
      → AC-18, AC-19
- [ ] Leave a screen open with no other request for 10 minutes, then act → still signed in, because
      the stream's heartbeat slid the session → AC-7, AC-18
- [ ] Sign out, choose Hindi on the sign in screen, sign in as somebody whose personal language is
      English → the app opens in English, and the sign in screen on that device stays Hindi → AC-21
- [ ] Tab through `/sign-in` and `/register` → the skip target, the heading, and the language
      switcher are all reachable, and the focus ring is visible on each → AC-21

## Commands

- [ ] `pnpm db:seed` → creates one restaurant and one admin, and prints the credentials that are in
      `.env.example` → AC-22
- [ ] `pnpm db:seed` a second time → says it already exists and creates nothing → AC-22
- [ ] `APP_ENV=production pnpm db:seed` → refuses to run → AC-22
- [ ] `curl -i localhost:8080/api/me` with no cookie → `401 unauthenticated` → AC-6
- [ ] `curl -i -X POST localhost:8080/api/auth/sign-out -b "session=<a valid value>"` with no
      `Origin` and no `Sec-Fetch-Site` → `403` → AC-12
- [ ] The same with `-H "Origin: https://elsewhere.example" -H "Host: localhost:8080"` → `403`;
      with `Origin: http://localhost:8080` → `204` → AC-12
- [ ] `curl -i -X PATCH localhost:8080/api/restaurant -b "session=<a waiter's>" -d '{"name":"x"}'`
      → `403 forbidden`, and no row changed → AC-8, AC-15
- [ ] Six wrong sign ins for one address inside a quarter of an hour → the sixth answers `429` with
      a `Retry-After` header, while another address in the same restaurant signs in normally →
      AC-10
- [ ] `curl -i -X POST .../api/auth/sign-in` six times for one address, all in parallel → at most
      five are counted, because the advisory lock serialises the count and the insert → AC-10
- [ ] `grep -rn "restaurant_id=\|x-restaurant-id\|RestaurantScope\|current-restaurant\|restaurant-settings" api/src web/src`
      → nothing → AC-9
- [ ] `SELECT count(*) FROM sessions WHERE absolute_expires_at <= expires_at` → 0, because the
      check constraint refuses it → AC-7
- [ ] `SELECT action, before, after FROM audit_log ORDER BY occurred_at` after registering, changing
      a password, and editing settings → exactly three rows, in that order, and no `$argon2` string
      anywhere in them → AC-17
- [ ] `SELECT token_hash FROM sessions` → 32 raw bytes per row, and none of them is the cookie value
      → AC-3
- [ ] After a sign in, `SELECT count(*) FROM login_attempts WHERE email = <another address>` is
      unchanged, so the sweep touched only the address that signed in → AC-23
- [ ] `pnpm check` → green: format, clippy, ESLint, typecheck, contrast, locale parity, both test
      suites, both builds → AC-21
- [ ] `pnpm sqlx:check && pnpm client:check` → no difference, so the committed cache and the
      generated client match the code → AC-1

## Value sourcing

One per row of the spec's table, exercising the edge that breaks if the source is wrong.

- [ ] Register in India, then in the United Kingdom → the first gets INR, `Asia/Kolkata`, `en-IN`;
      the second GBP, `Europe/London`, `en-GB`. Neither takes anything from the browser's own
      locale or timezone → AC-1
- [ ] Register with `countryCode: "in"` (lower case) → accepted, and `restaurants.country_code` is
      `IN` → AC-1
- [ ] Register with `email: "Ada@Example.com"` → the bundle returns exactly that, and signing in as
      `ADA@EXAMPLE.COM` works → AC-1
- [ ] Register with `countryCode: "FR"` → refused with `fields.countryCode=unknown_country`, because
      the country list is the only source → AC-1
- [ ] Sign in twice → the two cookie values differ, and neither appears in `sessions.token_hash` →
      AC-3
- [ ] Set the API container's clock forward an hour and slide a session → `expires_at` moves by
      Postgres's clock, not the container's, so a second container agrees about when it ends → AC-7
- [ ] Send `CloudFront-Viewer-Address: 203.0.113.7:52384` with `APP_ENV=production` → the attempt
      row's `ip` is `203.0.113.7`, not the socket peer → AC-11
- [ ] Send an IPv6 viewer address (`2001:db8::1:52384`) → the row's `ip` is `2001:db8::1`, not
      `2001` → AC-11
- [ ] Change the restaurant's formatting locale from `en-IN` to `en-US` → a four figure amount
      regroups (`1,23,456.78` becomes `123,456.78`) while the interface language does not move →
      AC-15
- [ ] Change the restaurant's timezone → every timestamp on screen shifts, and none of them follows
      the device → AC-15
- [ ] Sign in as a chef with a personal language of Hindi → the kitchen screen stays in the
      restaurant's language, and `/account` is in Hindi → AC-16
- [ ] With a valid session for restaurant A, read every list screen → only A's rows, proven against
      a real Postgres as `app_api` → AC-6, AC-9

## Acceptance-criteria coverage

- AC-1 registration and the country's five settings · covered by the register steps, the country
  sourcing steps, and `tests/accounts.rs::registering_takes_every_setting_from_the_country_row`
- AC-2 one transaction, and the field error shape · the duplicate address and blank field steps,
  and `a_duplicate_address_leaves_no_restaurant_behind`
- AC-3 the cookie and its hash · the sign out cookie step, the token steps, `cookie.rs` tests
- AC-4 identical refusals · the wrong password step and the two `SignIn` screen tests
- AC-5 the password rule · the short password step and the `credentials.rs` tests
- AC-6 resolved every request, no cache · the reload step, the `curl /api/me` step, the isolation
  step
- AC-7 sliding and the ceiling · the ten minute step, the constraint query,
  `sliding_moves_the_expiry_and_never_the_ceiling`, `a_session_past_its_ceiling_is_refused...`
- AC-8 the role gate · the waiter `PATCH /api/restaurant` step and the `actor.rs` tests
- AC-9 the placeholders are gone · the grep step and the stream address test
- AC-10 both throttle buckets · the six attempts step, the parallel step,
  `one_address_running_out_of_attempts_does_not_stop_another`
- AC-11 the client address · the two viewer address steps and the `client_address.rs` tests
- AC-12 the origin check · the three curl steps and the `origin.rs` tests
- AC-13 signing out one device · the two device step and `signing_out_on_one_device...`
- AC-14 changing your own password · the two password steps and
  `changing_a_password_keeps_this_session_and_ends_the_others`
- AC-15 the restaurant settings · the settings steps and the timezone refusal step
- AC-16 `PATCH /api/me` writes only your own row · the personal language steps
- AC-17 the three audit rows · the audit log query and
  `the_audit_rows_name_what_happened_and_carry_no_hash`
- AC-18 the stream re resolves on its heartbeat · the revoke while open step and the ten minute step
- AC-19 one `401` path · the revoke while open step and the `signed-out.ts` tests
- AC-20 role landing and the signed out redirect · the `/admin` signed out step, the admin to
  kitchen step
- AC-21 no hard coded text, formatted, accessible · the tab through step, `pnpm check`, and
  `auth-screens.test.tsx`
- AC-22 `pnpm db:seed` and the test fixture · the three seed commands and
  `common::register_and_sign_in`
- AC-23 the two scoped sweeps · the `login_attempts` step and
  `the_attempt_sweep_touches_only_the_address_that_signed_in`
