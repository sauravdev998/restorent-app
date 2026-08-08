# Verify: stack and architecture · spec 0001 · updated 2026-08-08

_Spec 0001 is a decision record, so it carries no numbered acceptance criteria. These steps come
from the scope's **Done when** ("the empty scaffold boots locally and passes a build") and from the
spec's **Load bearing implementation notes**, which are the parts that break quietly if they
regress. `/check verify` runs these; `/test` locks the durable ones._

Last run by `/check verify` on 2026-08-08 (second run, after the fixes): **16 of 16 steps passed.**
The two failures and the one blocker from the first run were fixed and re run against the real app;
each carries the measurement that proves it below. Two findings from the Sonnet review of the same
scaffold were fixed alongside them and are proven at runtime in "Extra evidence".

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
- [x] Stop Postgres (`docker compose stop db`) while a stream is open → the stream ends, and
      `/api/health` starts returning 503 with `"listener":"down"`. Start it again and the listener
      reconnects on its own.
      → spec: if an instance's listener connection dies, close that instance's streams

      **Failed on the first run, passes now.** The stream ended 0.13 seconds after the container
      stopped. Three health probes during the outage each answered
      `503 {"serving":false,"database":"down","listener":"down"}` in `2.002s`, `2.003s`, `2.003s`,
      well inside the 5 second health check timeout, and each names the component that is down.
      Before the fix this was a bodiless `408` after `30.005683s`. Postgres started again and the
      listener reconnected on its own with no help; health returned
      `{"serving":true,"database":"up","listener":"up"}`. The bound is
      `HEALTH_PROBE_TIMEOUT` (2 seconds) with the pool's own acquire timeout cut to 5 seconds, and
      a unit test keeps both under the timeouts that surround them.

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
- [x] The stream status badge is correct within a second of the page loading.

      **Failed on the first run, passes now.** The page was loaded and screenshotted straight
      after: the badge read `Connected` while the panel still said `No events yet`, so the badge is
      right before any event exists rather than waiting for one. The stream now writes a `: open`
      comment the moment it is subscribed, so headers flush at once: a quiet stream measured
      **1.4ms to first byte direct** and **24ms through the Vite proxy**, against 0 bytes at 8
      seconds before. Because `onopen` fires on headers, the refetch on connect no longer has a
      blind window of up to 15 seconds.
      → spec: notifications are fire and forget, so the client must resynchronise on connect
- [x] With the app open, restart the API → the stream reconnects and every active query refetches.

      **Blocked on the first run, passes now.** The Vite dev proxy was holding the browser
      connection open after its upstream died. With the proxy handing the upstream close down to
      the client, a `kill -9` of the API closed the **proxied** curl client and the **direct** one
      in the same 6 milliseconds (the proxied one used to hang past 25 seconds). In the browser:
      the badge flipped to `Connecting` within 2 seconds of the kill, and when the replacement
      process started it logged the stream reopening at `13:41:20.998` followed by a `/api/health`
      refetch 23 milliseconds later, so the reconnect and the resynchronise both happen. The badge
      returned to `Connected`.

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

## Extra evidence from the second run on 2026-08-08

The two review findings the same change fixed, proven by running the app rather than by reading it:

- **The reconnect backoff really does reset.** Outage one climbed through `retry_in_secs` 1, 2, 4,
  8. Postgres came back, the listener reconnected, and a second outage started at
  `retry_in_secs: 1` again. With the old code that reset sat in a match arm that could never run,
  so outage two would have opened at 16.
- **A stream the browser has abandoned reads `Closed`, not `Connecting`.** Run the API with
  `APP_ENV=production` and every `/api/events` request answers 401, which the browser treats as
  fatal. The badge read `Closed`. Before the fix it read `Connecting` for good, telling staff to
  wait for a reconnect that was never coming.

Still true after the change, re checked:

- The heartbeat schedule is undisturbed by the new open comment: a quiet stream produced `: open`
  at 0.0s, then `:` at 15.006s and 30.007s.
- Tenant isolation holds. Restaurant A received the probe event, restaurant B received its `: open`
  comment and nothing else.
- With `APP_ENV=production`: health 200, `/api/events` 401 both with the header and with the query
  parameter, `POST /api/dev/notify` 404, and one JSON object per log line.

Two things worth watching, neither a failure:

- **Each listener reconnect attempt takes about 30 seconds to fail**, so the backoff ladder barely
  shows: retry log lines during an outage were 31 seconds apart while the delay itself said 1 and
  then 2. `PgListener::connect` builds its own pool and keeps the SQLx default 30 second acquire
  timeout, which the `ACQUIRE_TIMEOUT` on the application pool does not touch. Recovery is still
  reasonable, because the connect attempt succeeds as soon as the database returns inside that
  window (measured: connected 24 seconds after Postgres came back), but the numbers in the backoff
  are close to decorative until that connect is bounded too.
- **The system status panel kept showing "Database Up, Live updates listener Up" while the API was
  dead.** Only the stream badge told the truth. That is TanStack Query holding its last good data
  with no error state on the panel, which is fine for a scaffold and misleading on a kitchen
  screen. Worth a look when the real screens are built.
