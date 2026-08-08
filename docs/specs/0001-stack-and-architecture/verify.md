# Verify: stack and architecture · spec 0001 · updated 2026-08-08

_Spec 0001 is a decision record, so it carries no numbered acceptance criteria. These steps come
from the scope's **Done when** ("the empty scaffold boots locally and passes a build") and from the
spec's **Load bearing implementation notes**, which are the parts that break quietly if they
regress. `/check verify` runs these; `/test` locks the durable ones._

Last run by `/check verify` on 2026-08-08: **13 of 16 steps passed, 2 failed, 1 blocked.** See the
three findings recorded against the unticked steps below.

## Commands

- [x] `pnpm db:up` → the container reports healthy, and `app_api` exists with no attributes (no
      Superuser, no Bypass RLS). Check with
      `docker exec restaurant-db psql -U restaurant_owner -d restaurant -c "\du"`.
      → spec: database roles
- [x] `pnpm migrate` → applies `0001_bootstrap`, and
      `docker exec restaurant-db psql -U restaurant_owner -d restaurant -c "\df public.*"`
      lists `current_restaurant_id` and `notify_entity_change`.
      → Done when: scaffold boots
- [x] `pnpm check` → format, clippy denying warnings, cargo tests, web typecheck, web lint, web
      tests, and both builds all pass.
      → Done when: passes a build
- [x] `env -u DATABASE_URL cargo build --manifest-path api/Cargo.toml` → succeeds with no database
      reachable, proving the committed `.sqlx` cache is what continuous integration will compile
      against.
      → spec: SQLx offline cache
- [x] `pnpm sqlx:check` → exits 0. Change a query without running `pnpm sqlx:prepare` and it must
      fail.
      → spec: compile time checked SQL
- [x] `pnpm client:check` → exits 0. Rename a field in a Rust response struct and it must fail,
      because that is the whole point of the generated seam.
      → spec: API contract

## The live path (the riskiest thing in the spec)

Start the API with `pnpm dev:api`, then:

- [x] `curl -s http://127.0.0.1:8080/api/health` → `{"serving":true,"database":"up","listener":"up"}`
      → spec: health checking
- [x] Open a stream for restaurant A and another for restaurant B:
      `curl -sN -H "X-Restaurant-Id: 11111111-1111-1111-1111-111111111111" http://127.0.0.1:8080/api/events`
      and the same with `2222...`. Then
      `curl -X POST -H "X-Restaurant-Id: 1111..." http://127.0.0.1:8080/api/dev/notify`.
      → A receives one `entity_changed` event whose `entity_id` matches the POST response.
      **B receives nothing.** This is the tenant isolation check and it must never be skipped.
      → spec: realtime fan out, tenant isolation
- [x] `curl -i http://127.0.0.1:8080/api/events` with no restaurant at all → `401`.
      → spec: sessions
- [x] Leave a stream open for more than 60 seconds with no events → it stays open, because the
      server sends a comment heartbeat every 15 seconds.
      → spec: keep the event stream alive
- [ ] Stop Postgres (`docker compose stop db`) while a stream is open → the stream ends, and
      `/api/health` starts returning 503 with `"listener":"down"`. Start it again and the listener
      reconnects on its own.
      → spec: if an instance's listener connection dies, close that instance's streams

      **FAILED on 2026-08-08.** The stream did end, and the listener did reconnect on its own once
      Postgres came back. But health never returned 503. It **blocked for 30.0 seconds and then
      returned 408** with no body, so it never said which component was down. Measured: healthy
      `200` in 1ms; database down `408` in `30.005683s`; with the target group's own 5 second
      timeout, no response at all. Cause: `Database::is_reachable` waits on the pool's default 30
      second acquire timeout, and the router's 30 second timeout layer fires first. The load
      balancer would still replace the task (its check times out), but each probe also parks a pool
      waiter for 30 seconds, at a 15 second probe interval, during an outage.

## The production image

- [x] `docker build -f api/Dockerfile -t restaurant-api .` → builds.
- [x] Run it with `APP_ENV` left at its image default of `production`, then:
      - `GET /api/health` → `200`
      - `GET /api/events` **with** an `X-Restaurant-Id` header → `401`. The header must be ignored
        outside development, or the placeholder becomes a way into any restaurant's data.
      - `POST /api/dev/notify` → `404`. Development routes must not exist in production.
      - `docker logs` → one JSON object per line, which is what CloudWatch collects.
      → spec: observability, sessions

## The web app

- [x] `pnpm dev:web` with the API running → `http://localhost:5173/` renders, `/api/health` proxies
      through, and the health panel shows the database and listener as up.
- [x] Click "Send a test event" on the home screen → the event count rises and the last event
      appears, without a refresh. Passed in Chrome on 2026-08-08: the badge read `Connected`,
      `1 event received`, and `Last event: probe f7c4899b-0e46-4f51-a1b8-827357a2b06f`.
- [ ] The stream status badge is correct within a second of the page loading.

      **FAILED on 2026-08-08.** The badge read `Closed` for the whole first 15 seconds, then
      flipped to `Connected` only once the first event arrived. Cause: nothing is written to the
      stream when it opens, so the response headers are not flushed until the first event or the
      first 15 second heartbeat. Measured directly against the API: 8 seconds of a quiet stream
      returned **0 bytes**, not even headers; at 20 seconds the headers and the first chunk arrived
      together. A browser fires `EventSource.onopen` on headers, so `onopen` is late by up to 15
      seconds. That matters well beyond the badge: the refetch on open lives in `onopen`, so
      **every connect and every reconnect has a blind window of up to 15 seconds** in which the
      screen is live but has not resynchronised. That is the exact gap the note below says must be
      closed. Fix by writing one comment byte as soon as the stream is subscribed.
      → spec: notifications are fire and forget, so the client must resynchronise on connect
- [ ] With the app open, restart the API → the stream reconnects and every active query refetches.

      **BLOCKED on 2026-08-08, and the blocker is the Vite dev proxy, not the app.** With the API
      killed, the browser still read `Connected` more than 20 seconds later and the replacement
      process logged zero stream opens. Isolated with two `curl` streams and one `kill -9`: the
      **direct** client exited in the same second as the kill; the **proxied** client hung
      indefinitely. So the dev proxy holds the browser connection open after its upstream dies, and
      the browser never learns to reconnect. Production has no Vite proxy (CloudFront to load
      balancer to task), so this specific behaviour should not occur there, but that is reasoning,
      not evidence. Until the proxy is configured to drop the client when the upstream drops, the
      reconnect and resynchronise path cannot be exercised locally at all, which is a poor place to
      be given features 8 and 13 depend on it.

## Coverage

- Done when, "scaffold boots locally" → the migration, health, and dev server steps.
- Done when, "passes a build" → `pnpm check`, plus the release build and the container build.
- Spec, tenant isolation → the two restaurant stream check, and the production fail closed check.
- Spec, realtime → heartbeat, listener death, and resynchronise on reconnect.
- Spec, the generated seam → `sqlx:check` and `client:check`.
- **Not covered anywhere yet**: row level security policies. Feature 4 owns the schema, so there is
  no tenant scoped table to write a policy on. The `current_restaurant_id()` helper and the scoped
  transaction exist and are exercised; the policies that read them do not exist.

## Extra evidence gathered on 2026-08-08

Worth keeping, because these are the things a later change could break silently:

- The transaction scope really is local. Run as `app_api`:
  `BEGIN; SELECT set_config('app.restaurant_id','1111...',true); SELECT current_restaurant_id(); COMMIT;`
  returns the id inside the transaction and **NULL after the commit**, so a pooled connection
  cannot carry one request's restaurant into the next.
- The generated client gate genuinely fails on drift. Renaming `serving` to `is_serving` in the
  health response made `pnpm client:check` exit 1; reverting made it exit 0 again.
- The heartbeat is real: a silent stream held for 70 seconds received exactly 4 comment lines, at
  15, 30, 45, and 60 seconds, and stayed open throughout.
- Graceful shutdown waits for open event streams. After `SIGTERM` the old process released the
  listening socket but stayed alive serving its existing stream. On ECS that means every deploy
  waits out the stop timeout for each connected screen, so a bounded shutdown is worth adding.
