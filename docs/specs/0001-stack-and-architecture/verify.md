# Verify: stack and architecture · spec 0001 · updated 2026-08-08

_Spec 0001 is a decision record, so it carries no numbered acceptance criteria. These steps come
from the scope's **Done when** ("the empty scaffold boots locally and passes a build") and from the
spec's **Load bearing implementation notes**, which are the parts that break quietly if they
regress. `/check verify` runs these; `/test` locks the durable ones._

Every step below was run and passed on 2026-08-08, except the two marked **not yet run**.

## Commands

- [ ] `pnpm db:up` → the container reports healthy, and `app_api` exists with no attributes (no
      Superuser, no Bypass RLS). Check with
      `docker exec restaurant-db psql -U restaurant_owner -d restaurant -c "\du"`.
      → spec: database roles
- [ ] `pnpm migrate` → applies `0001_bootstrap`, and
      `docker exec restaurant-db psql -U restaurant_owner -d restaurant -c "\df public.*"`
      lists `current_restaurant_id` and `notify_entity_change`.
      → Done when: scaffold boots
- [ ] `pnpm check` → format, clippy denying warnings, cargo tests, web typecheck, web lint, web
      tests, and both builds all pass.
      → Done when: passes a build
- [ ] `env -u DATABASE_URL cargo build --manifest-path api/Cargo.toml` → succeeds with no database
      reachable, proving the committed `.sqlx` cache is what continuous integration will compile
      against.
      → spec: SQLx offline cache
- [ ] `pnpm sqlx:check` → exits 0. Change a query without running `pnpm sqlx:prepare` and it must
      fail.
      → spec: compile time checked SQL
- [ ] `pnpm client:check` → exits 0. Rename a field in a Rust response struct and it must fail,
      because that is the whole point of the generated seam.
      → spec: API contract

## The live path (the riskiest thing in the spec)

Start the API with `pnpm dev:api`, then:

- [ ] `curl -s http://127.0.0.1:8080/api/health` → `{"serving":true,"database":"up","listener":"up"}`
      → spec: health checking
- [ ] Open a stream for restaurant A and another for restaurant B:
      `curl -sN -H "X-Restaurant-Id: 11111111-1111-1111-1111-111111111111" http://127.0.0.1:8080/api/events`
      and the same with `2222...`. Then
      `curl -X POST -H "X-Restaurant-Id: 1111..." http://127.0.0.1:8080/api/dev/notify`.
      → A receives one `entity_changed` event whose `entity_id` matches the POST response.
      **B receives nothing.** This is the tenant isolation check and it must never be skipped.
      → spec: realtime fan out, tenant isolation
- [ ] `curl -i http://127.0.0.1:8080/api/events` with no restaurant at all → `401`.
      → spec: sessions
- [ ] Leave a stream open for more than 60 seconds with no events → it stays open, because the
      server sends a comment heartbeat every 15 seconds.
      → spec: keep the event stream alive
- [ ] Stop Postgres (`docker compose stop db`) while a stream is open → the stream ends, and
      `/api/health` starts returning 503 with `"listener":"down"`. Start it again and the listener
      reconnects on its own.
      → spec: if an instance's listener connection dies, close that instance's streams

## The production image

- [ ] `docker build -f api/Dockerfile -t restaurant-api .` → builds.
- [ ] Run it with `APP_ENV` left at its image default of `production`, then:
      - `GET /api/health` → `200`
      - `GET /api/events` **with** an `X-Restaurant-Id` header → `401`. The header must be ignored
        outside development, or the placeholder becomes a way into any restaurant's data.
      - `POST /api/dev/notify` → `404`. Development routes must not exist in production.
      - `docker logs` → one JSON object per line, which is what CloudWatch collects.
      → spec: observability, sessions

## The web app

- [ ] `pnpm dev:web` with the API running → `http://localhost:5173/` renders, `/api/health` proxies
      through, and the health panel shows the database and listener as up.
- [ ] **Not yet run.** Click "Send a test event" on the home screen → the event count rises and the
      last event appears, without a refresh. This is the browser half of the live path and it has
      only been proven with `curl` so far.
- [ ] **Not yet run.** With the app open, restart the API → the stream reconnects and every active
      query refetches. Without that refetch, one network blip is a ticket that never appears.
      → spec: notifications are fire and forget, so the client must resynchronise on connect

## Coverage

- Done when, "scaffold boots locally" → the migration, health, and dev server steps.
- Done when, "passes a build" → `pnpm check`, plus the release build and the container build.
- Spec, tenant isolation → the two restaurant stream check, and the production fail closed check.
- Spec, realtime → heartbeat, listener death, and resynchronise on reconnect.
- Spec, the generated seam → `sqlx:check` and `client:check`.
- **Not covered anywhere yet**: row level security policies. Feature 4 owns the schema, so there is
  no tenant scoped table to write a policy on. The `current_restaurant_id()` helper and the scoped
  transaction exist and are exercised; the policies that read them do not exist.
