# Web (React 19, Vite)

## Overview

One single page app carrying three surfaces that barely share a screen: admin, waiter, and
kitchen. It talks to the Rust API through a client generated from the API's own OpenAPI
document, holds one server sent event stream open for the whole browser, and uses that stream
to keep the TanStack Query cache honest. A pnpm workspace package. Root `AGENTS.md` holds the
project wide rules; this file holds what is true only here.

## Key files

| File | Owns |
|---|---|
| `src/main.tsx` | Mounts the app: query client, router, i18n, styles |
| `src/app/router.tsx` | The route table, React Router in data mode. Three route groups |
| `src/app/root-layout.tsx` | The shell, and the one place the live stream is held open |
| `src/app/query-client.ts` | Cache defaults. Deliberately quiet on automatic refetching |
| `src/shared/api/client.ts` | The typed client. Every real API call goes through it |
| `src/shared/api/schema.d.ts` | Generated from `api/openapi.json`. Never edit it |
| `src/shared/events/use-live-events.ts` | The stream, and the two rules that keep the cache honest |
| `src/shared/session/current-restaurant.ts` | A development only placeholder for the restaurant id |
| `src/shared/i18n/index.ts` | Translations, set up before the first screen renders |
| `src/test/setup.ts` | The `EventSource` stand in jsdom does not provide |
| `vite.config.ts` | Dev server, the `/api` proxy, and the event stream handling inside it |
| `eslint.config.js` | Type aware rules, scoped to the TypeScript sources |

## Commands

Run from the repository root.

```bash
pnpm dev:web                        # or pnpm --filter web dev
pnpm --filter web test              # vitest run
pnpm --filter web typecheck
pnpm --filter web lint
pnpm client:generate                # regenerate schema.d.ts from the Rust handlers
```

Formatting is a root command, `pnpm format`. This package has no format script of its own.

## Conventions

- **Folders group by feature**, not by kind: `admin/`, `waiter/`, `kitchen/`, `shared/`, plus `app/` for the shell. The three surfaces share `shared/` and little else.
- **Import through `@/`**, never through a chain of `../../..`. The alias is set in both `vite.config.ts` and `tsconfig.app.json`.
- **Every real API call goes through `api` in `src/shared/api/client.ts`.** A hand written `fetch` to `/api` anywhere else drops the typed seam on the floor.
- **`src/shared/api/schema.d.ts` is generated and committed.** ESLint ignores it, continuous integration regenerates it and fails on a difference. Rename a field in Rust and this stops compiling, which is the point.
- **No user facing string is written into a component.** Everything goes through `t()` and `src/locales/<lang>/common.json`.
- **Server state lives in TanStack Query**, shared query options rather than a hook per screen (see `shared/api/health.ts`), so every screen hits one cache entry.
- **Strictness is not negotiable.** `strict`, `noUncheckedIndexedAccess`, `exactOptionalPropertyTypes`, and a lint error on non null assertions.

## Gotchas

- **Two rules in `use-live-events.ts` are load bearing.** Refetch every active query on every stream open, including every reconnect, because Postgres queues nothing for a listener that is not connected. And an event invalidates, it never writes: the message carries a kind and an id, so the client goes back and asks for the row and row level security stays the authority.
- **`EventSource.onerror` covers two different failures**, and only `readyState` tells them apart. A dropped connection is retried by the browser and reads CONNECTING; a response the browser cannot use, a `401` in particular, is fatal and reads CLOSED. Reporting "connecting" for a stream that is never coming back tells staff to wait when they need to reload.
- **jsdom has no `EventSource`**, so `src/test/setup.ts` stubs it. The stub carries `readyState` and the three state constants on purpose: without them both sides of that comparison are `undefined` and every error looks alike in a passing test.
- **The Vite proxy destroys the client response when an event stream's upstream closes or errors**, and the handling is scoped to event streams only. Without it the browser hangs on a stream nobody is writing to and never reconnects. Unscoped, it would replace Vite's diagnosable `502` with a bare connection reset on every ordinary `/api/*` call made while the API is down.
- **One stream per browser, held in `RootLayout`**, not one per screen. Screens read it from the outlet context.
- **`currentRestaurantId()` is a placeholder** that only returns a value in development. The server refuses a client supplied restaurant outside development, so it cannot become a way into production data. Feature 7 deletes the file.
- **shadcn/ui is chosen but not installed yet.** Feature 5 (design system and accessibility baseline) brings it in. Current screens are plain Tailwind.

## Agent skills

- [react-router-data-mode](../.agents/skills/react-router-data-mode/): `remix-run/agent-skills`, route objects, loaders, actions, pending and optimistic UI
- [tanstack-query](../.agents/skills/tanstack-query/): `tanstack-skills/tanstack-skills`, server state, caching, refetching, cache updates from incoming events
- [shadcn](../.agents/skills/shadcn/): `shadcn/ui`, component installation, composition, styling, and forms
- [tailwind-4-docs](../.agents/skills/tailwind-4-docs/): `lombiq/tailwind-agent-skills`, Tailwind v4 utilities, variants, and its CSS based configuration
- [react-i18next](../.agents/skills/react-i18next/): `yildizberkay/skills`, translation setup, namespaces, plurals, interpolation
- [vitest](../.agents/skills/vitest/): `antfu/skills`, web unit tests, mocking, coverage, fixtures

## Related specs

- [0001 stack and architecture](../docs/specs/0001-stack-and-architecture/index.md)

_Drafted by /audit from the repo, worth a quick human pass. Edit freely: once a line stops matching this draft, later runs treat it as curated and will flag rather than overwrite it._
