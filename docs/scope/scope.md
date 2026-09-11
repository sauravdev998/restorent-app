# Scope: Restaurant Operations Platform

A web platform many restaurants sign up for, where an owner runs the restaurant and their waiters and chefs run the floor and the kitchen: orders taken at the table, tickets live in the kitchen, food marked ready per dish, and a bill at the end of the meal.

**Build approach:** Tracer Bullet (a thin thread pierces every layer and works, then you thicken it).
**Workflow:** GA (after `/develop`: `/check verify`, then `/test`, then a fresh model `/check review`, then `/document`). The project's default rigor tier; a feature's own tier tag (e.g. `· Beta`) overrides it.

_You are in charge. Every box below is a **suggestion**, not a gate: run any, skip any, and mark a feature `done` when you decide it is. The workflow records what you actually did (including "skipped"), it never requires a step. The one thing it asks is that a load bearing decision be written down (a spec), not that any check be run._

## At a glance

| # | Feature | Phase | Status |
|---|---------|-------|--------|
| 1 | Stack and architecture | Foundation | done |
| 2 | Coding standards and tooling | Foundation | done |
| 3 | Error and crash monitoring | Foundation | dropped |
| 4 | Core data model | Foundation | done |
| 5 | Design system and accessibility baseline | Foundation | done |
| 6 | Language and text foundation | Foundation | done |
| 7 | Accounts, restaurants, and roles | Foundation | done |
| 8 | The thin order thread | Slice 1 | done |
| 9 | Menu management | Slice 2 | in-progress |
| 10 | Staff accounts | Slice 2 | planned |
| 11 | Tables and floor plan | Slice 2 | planned |
| 12 | Waiter service flow | Slice 3 | planned |
| 13 | Kitchen display | Slice 3 | planned |
| 14 | Bill generation, currency, and tax | Slice 4 | planned |
| 15 | Closing a bill and recording payment | Slice 4 | planned |
| 16 | Kitchen ticket and bill printing | Slice 4 | planned |
| 17 | Admin order monitor | Slice 5 | planned |
| 18 | Sales reports | Slice 5 | planned |
| 19 | Public marketing page and signup | Slice 6 | planned |
| 20 | Privacy, terms, and cookie consent | Slice 6 | planned |
| 21 | Subscription plans and paid signup | Deferred | planned |
| 22 | Customer QR self ordering | Deferred | planned |
| 23 | Splitting and merging bills | Deferred | planned |
| 24 | Password recovery by email | Slice 6 | planned |
| 25 | Dish photos | Deferred | planned |
| 26 | Dish sizes and add ons | Deferred | planned |
| 27 | Dish names in a second language | Deferred | planned |

## Foundations

Nothing here is optional groundwork you can skip past: every slice below stands on it. Build them in this order, cheapest ground first.

### 1. Stack and architecture · done
Choose the stack and scaffold a runnable project, so every later slice is built on real structure rather than guesses. This is the one place tools, frameworks, hosting, and providers get chosen.
**Done when:** the stack is recorded in a spec, and the empty scaffold boots locally and passes a build.
spec [0001](../specs/0001-stack-and-architecture/index.md) · code in `api/`, `web/`, `infra/`
- [x] Decide the stack (spec): `/architect stack and architecture`
- [x] Scaffold from the decision: `/develop stack and architecture`
- [x] Verify it: `/check verify stack and architecture`
- [x] Test it: `/test`
- [x] Review it: `/check review`

### 2. Coding standards and tooling · done
Capture the conventions from the real scaffolded project, then install lint, formatting, type strictness, and a pre commit hook so all later code follows one shape.
**Done when:** root `AGENTS.md` reflects the real stack, and lint, format, and the pre commit hook run clean.
verify [0002](../specs/0002-coding-standards-and-tooling/verify.md) · context in `AGENTS.md`, `api/AGENTS.md`, `web/AGENTS.md`, `infra/AGENTS.md` · tooling in `lefthook.yml`, `.github/workflows/ci.yml`, `.prettierrc.json`, `infra/eslint.config.mjs`
- [x] Capture conventions and tooling choices: `/audit`
- [x] Install the tooling: `/develop tooling`

### 3. Error and crash monitoring · dropped
Dropped on 8 August 2026: you do not want it. Kept as a row so the numbering and the history stay honest, and so a later run does not quietly plan it again.
What stands in for it: structured logs going to CloudWatch, already decided in spec [0001](../specs/0001-stack-and-architecture/index.md). That tells you what happened once you go looking; it does not tell you a waiter's screen broke mid shift. Accepted trade off.
No boxes. If you change your mind, run `/architect error and crash monitoring` and this row comes back to life.

### 4. Core data model · done
The entities everything else is built on: restaurants, staff and their roles, menu categories and dishes, tables, bills, order rounds, order lines with per dish status, and payment records. Multi restaurant separation lives here, and it is the most expensive thing in the project to get wrong.
**Done when:** the schema supports one open bill per table with many rounds, per dish status, per restaurant currency and tax settings, and strict data separation between restaurants, all without a breaking change when later slices land.
spec [0003](../specs/0003-core-data-model/index.md) · code in `api/migrations/0002_core_data_model.sql`, `api/scripts/init-roles.sql`, `api/src/domain/`, `api/src/infrastructure/db/`, `api/tests/`
- [x] Design it (spec): `/architect core data model`
- [x] Build it: `/develop core data model`
  - [x] Schema and isolation: the six enum types, all sixteen tables, composite tenant keys, indexes, and row level security (AC-1, AC-2, AC-3, AC-4, AC-9, AC-11, AC-13, AC-15)
  - [x] The two bootstrap lookups and bill numbering: the `auth_lookup` role, the login and session functions, the gapless counter (AC-6, AC-12)
  - [x] Rust types and repository plumbing: identifier newtypes, the six enums, the real event entities, the eleven scoped operations (AC-7, AC-8, AC-10, AC-13)
  - [x] Money and audit: the bill close snapshot, rounding to the restaurant's currency, the audit log (AC-5, AC-9, AC-14)
  - [x] Tests and generated artifacts: the integration tests against a real Postgres as `app_api`, and the refreshed `.sqlx` cache (AC-1 through AC-15)
- [x] Verify it: `/check verify core data model`
- [x] Test it: `/test core data model`
- [x] Review it (fresh model): `/check review core data model`
- [x] Document it: `/document core data model`

### 5. Design system and accessibility baseline · done
The visual language and base components every screen uses, built for three very different contexts: an admin on a desktop, a waiter on a phone, a chef on a kitchen screen read from a distance. The accessibility target is set here and then applied by every later feature rather than being its own row.
**Done when:** `design.md` covers type, colour, spacing, and the base components; components handle keyboard use and focus; the chosen accessibility level is written down and the base components meet it.
spec [0004](../specs/0004-design-system-and-accessibility/index.md) · design in `docs/design.md` · code in `web/src/styles/index.css`, `web/src/shared/ui/`, `web/src/app/design/`, `web/scripts/check-contrast.ts`, `web/eslint.config.js`, `web/src/test/axe.tsx`
- [x] Design it (spec): `/architect design system and accessibility baseline`
- [x] Build it: `/develop design system and accessibility baseline`
  - [x] Tokens and the console look: the fonts, the icons, the shadcn helpers, and the four token layers covering both appearances, print, and forced colours (AC-1, AC-2, AC-3, AC-12, AC-16)
  - [x] The frame and the thread's components: who owns the surface density, the shell, and `Icon`, `Button`, `Card`, `StatusPill`, `ElapsedTime` (AC-2, AC-5, AC-6, AC-12, AC-14)
  - [x] The three gates and the gallery: the contrast script, the lint and axe rules, and the development only `/design` route (AC-1, AC-4, AC-6, AC-11, AC-13, AC-15)
  - [x] The rest of the components: `Field`, `Input`, `Select`, `Dialog`, `LiveRegion`, `Toast`, `Alert`, `Skeleton`, `EmptyState`, `DataTable` (AC-4, AC-7, AC-8, AC-9, AC-10)
  - [x] Finishing: the logical property sweep, the forced colours pass, the print block, and `docs/design.md` (AC-11, AC-13, AC-16, AC-17)
- [x] Verify it: `/check verify design system and accessibility baseline`
- [x] Test it: `/test design system and accessibility baseline`
- [x] Review it (fresh model): `/check review design system and accessibility baseline`
- [x] Document it: `/document design system and accessibility baseline`

### 6. Language and text foundation · done
Every piece of text in the app comes from a translation file from the first screen onward, plus how a user's language is chosen and stored. Cheap now, painful to retrofit once twenty screens exist.
**Done when:** no screen has hard coded user facing text, a second language can be added by dropping in one file, and a staff member's language choice sticks across sessions.
spec [0005](../specs/0005-language-and-text-foundation/index.md) · catalogue in `locales/` · code in `web/src/shared/i18n/`, `web/src/shared/format/`, `web/src/locales/`, `api/src/domain/language.rs`, `api/migrations/0003_language_and_formatting.sql` · gates in `web/scripts/check-locales.ts`, `web/eslint.config.js`
- [x] Design it (spec): `/architect language and text foundation`
- [x] Build it: `/develop language and text foundation`
  - [x] The thread, top to bottom: the shared catalogue, migration 0003, the Rust newtypes, the two resolvers, loading on demand, the switcher, and Hindi for the shell (AC-2, AC-3, AC-4, AC-5, AC-6, AC-7, AC-8, AC-9, AC-10, AC-15, AC-17)
  - [x] Every string moved into the split: the four namespaces, the sixteen call sites, and full Hindi key parity (AC-1, AC-2, AC-5)
  - [x] Formatting cut loose from language: the memoised Intl formatters, money from an exact decimal, the restaurant's timezone, and error codes mapped to keys (AC-1, AC-3, AC-12, AC-13)
  - [x] The three gates and the font: the lint rule, the parity check in continuous integration, the pseudo language, and Devanagari (AC-1, AC-2, AC-14, AC-16)
  - [x] Mixed language text and the written rules: the language marked wrapper for restaurant typed data, and `docs/design.md` (AC-11, AC-12)
- [x] Verify it: `/check verify language and text foundation`
- [x] Test it: `/test language and text foundation`
- [x] Review it (fresh model): `/check review language and text foundation`
- [x] Document it: `/document language and text foundation`

### 7. Accounts, restaurants, and roles · done
An owner registers a restaurant and becomes its admin, staff sign in, and every request is limited to that person's restaurant and role. Real authentication and real data separation from the very first slice, never faked.
**Done when:** an owner can register a restaurant and sign in as its admin; a signed in user only ever sees their own restaurant's data; a waiter cannot reach admin or chef screens and a chef cannot reach admin or waiter screens, on the server as well as in the interface.
spec [0006](../specs/0006-accounts-restaurants-and-roles/index.md) · verify [0006](../specs/0006-accounts-restaurants-and-roles/verify.md) · shared list in `locales/countries.json` · api in `api/migrations/0004_accounts_and_sessions.sql`, `api/src/domain/{country,credentials,session,throttle}.rs`, `api/src/infrastructure/passwords.rs`, `api/src/infrastructure/db/repository/{accounts,sessions}.rs`, `api/src/presentation/{cookie,dto,origin}.rs`, `api/src/presentation/extract/{actor,client_address,json}.rs`, `api/src/presentation/handlers/{auth,me}.rs`, `api/src/bin/seed.rs` · web in `web/src/shared/session/`, `web/src/shared/countries.ts`, `web/src/shared/api/field-errors.ts`, `web/src/app/routes/`, `web/src/admin/routes/restaurant-settings.tsx` · infra in `infra/lib/platform-stack.ts`
- [x] Design it (spec): `/architect accounts, restaurants, and roles`
- [x] Build it: `/develop accounts, restaurants, and roles`
  - [x] The thread, top to bottom: the countries file, migration 0004, the password and session plumbing, the four auth endpoints, the `Actor` extractor replacing the placeholder, and a sign in screen that reaches a real surface (AC-1, AC-2, AC-3, AC-4, AC-5, AC-6, AC-9, AC-13, AC-23)
  - [x] The session's whole life: the sliding refresh and its exactly one row assertion, the absolute ceiling, the stream's heartbeat, and the one 401 path in the client (AC-7, AC-18, AC-19)
  - [x] Roles made structural: the role requirement carried in the handler's own type, and the three route groups with their landing redirect (AC-8, AC-20)
  - [x] Standing in front of the door: both throttle buckets counted in Postgres, the client address plus the two `infra/` changes, and the origin check (AC-10, AC-11, AC-12)
  - [x] The rest of the surface: the register, account, and settings screens, their three write endpoints, the audit rows, the two sweeps, the seed command, and deleting both placeholders (AC-14, AC-15, AC-16, AC-17, AC-21, AC-22)
- [x] Verify it: `/check verify accounts, restaurants, and roles`
- [x] Test it: `/test accounts, restaurants, and roles`
- [x] Review it (fresh model): `/check review accounts, restaurants, and roles`
- [x] Document it: `/document accounts, restaurants, and roles`

## Slice 1: the thin order thread

One narrow path pushed through every layer, working for real. No breadth: one table, one dish, one round, the plainest screens. This proves the whole pipe connects, which is the scariest risk in the project, and it is also the walking skeleton. Everything after this thickens one segment of this thread.

### 8. The thin order thread · done
A waiter opens a bill on a table and adds one dish, the kitchen sees the ticket appear live, the chef marks the dish done, the ticket flips to ready and the waiter's screen updates with a sound, and the waiter closes the bill with a total. Real database, real login, real screens, narrow on purpose.
**Done when:** on two devices at once, a dish sent by the waiter appears on the kitchen screen within a second or two without a refresh; marking it done flips the ticket to ready and alerts the waiter the same way; closing the bill records a total; and the order's state changes are safe when two people act at the same time.
spec [0007](../specs/0007-the-thin-order-thread/index.md) · verify [0007](../specs/0007-the-thin-order-thread/verify.md) · no migration: it reaches the schema and the eleven operations spec [0003](../specs/0003-core-data-model/index.md) already built · api in `api/src/presentation/handlers/{menu,service,billing}.rs`, `api/src/presentation/dto.rs`, `api/src/domain/error.rs` (`ConflictKind`), `api/src/infrastructure/db/repository/{service,billing,catalog,accounts}.rs`, `api/src/bin/seed.rs`, `api/tests/order_thread.rs` · web in `web/src/waiter/`, `web/src/kitchen/`, `web/src/shared/events/{query-keys,server-clock}.ts`, `web/src/shared/ui/stream-warning.tsx` · browser test in `web/e2e/order-thread.spec.ts`, `web/playwright.config.ts`
- [x] Design it (spec): `/architect the thin order thread`
- [x] Build it: `/develop the thin order thread`
  - [x] Ground for the thread to run on: the grown seed (tables, menu, a waiter and a chef) and the conflict codes that let a refusal be read in the reader's own language (AC-12, AC-13, AC-17)
  - [x] The thread, top to bottom: the floor and menu reads, opening a table, sending a round, the kitchen queue, marking a dish ready, and the two screens that make it visible on two devices (AC-1, AC-3, AC-4, AC-5, AC-7, AC-14)
  - [x] Live updates narrowed and the clock made honest: the entity keyed query keys with their fan out map, and ages computed against the server's time rather than the tablet's (AC-6, AC-15)
  - [x] Back to the waiter and out: the visit document, the serve action, the ready alert that announces and chimes once per round, and the close that writes the total and frees the table (AC-8, AC-9, AC-10, AC-11, AC-12)
  - [x] Honest when it breaks, and proven: connection states with both screens still usable, pending taps with no cache optimism, the two device Playwright run, and the rest of the tests and words (AC-2, AC-13, AC-16, AC-18, AC-19)
- [x] Verify it: `/check verify the thin order thread`
- [x] Test it: `/test the thin order thread`
- [x] Review it (fresh model): `/check review the thin order thread`
- [x] Document it: `/document the thin order thread`

## Slice 2: one restaurant set up for real

Thicken the setup segment, so a real restaurant's own menu, staff, and tables drive the thread rather than the one hardcoded dish and table from slice 1.

### 9. Menu management · in-progress
The admin builds the real menu: categories, dishes with a price, and a switch to mark a dish unavailable when the kitchen runs out, which immediately stops waiters ordering it.
**Done when:** an admin can create, edit, reorder, and remove categories and dishes; marking a dish unavailable removes it from the waiter's ordering screen at once; a dish already on an open bill is unaffected.
spec [0008](../specs/0008-menu-management/index.md) · migration `api/migrations/0006_menu_management.sql` · api in `api/src/presentation/handlers/{admin_menu,menu,service}.rs`, `api/src/infrastructure/db/repository/{catalog,service}.rs`, `api/src/domain/{menu,catalog,enums,error,event,audit}.rs`, `api/src/presentation/extract/actor.rs`, `api/src/bin/seed.rs`, `api/tests/menu.rs` · web in `web/src/admin/`, `web/src/kitchen/routes/kitchen-menu.tsx`, `web/src/kitchen/components/kitchen-tabs.tsx`, `web/src/waiter/basket.ts`, `web/src/shared/api/{menu,use-dish-availability,call-error}.ts`, `web/src/shared/ui/{switch,diet-mark}.tsx`, `web/src/styles/index.css` · browser test in `web/e2e/menu.spec.ts`
- [x] Design it (spec): `/architect menu management`
- [x] Build it: `/develop menu management`
  - [x] The thread, top to bottom: migration 0006, the `menu_category` event kind, the two combined roles, the admin menu read, creating a dish, the availability switch, the plainest admin screen and kitchen Menu tab, and the two browser Playwright run (AC-2, AC-9, AC-10, AC-13, AC-14, AC-16, AC-17, AC-18, AC-19, AC-20)
  - [x] The whole edit surface and its rules: category create and rename, dish edit and move, version checks, field errors, live name uniqueness, the audit rows, the price input, and the diet mark with its tokens (AC-1, AC-3, AC-13, AC-14, AC-15, AC-17)
  - [x] Order: the two reorder endpoints and dnd-kit drag and drop with keyboard, touch, and translated announcements (AC-4, AC-5)
  - [x] Remove and restore: the archive rules and their locks, the Archived section, and the restore dialog (AC-6, AC-7, AC-8, AC-17)
  - [x] The waiter's side and proof: basket flagging and `dish_not_orderable`, the base `Switch`, every state and translation, and the full tests (AC-9, AC-10, AC-11, AC-12, AC-16, AC-19, AC-20)
- [ ] Verify it: `/check verify menu management`
- [ ] Test it: `/test menu management`
- [ ] Review it (fresh model): `/check review menu management`
- [ ] Document it: `/document menu management`

### 10. Staff accounts · needs a decision
The admin creates waiter and chef accounts, hands out access, changes someone's role, and shuts off an account when a person leaves. Staff never register themselves.
**Done when:** an admin can create a staff member with a role, that person can sign in and lands on the right screen for their role, an admin can change a role or deactivate an account, and a deactivated account is refused at once.
- [ ] Design it (spec): `/architect staff accounts`

### 11. Tables and floor plan · needs a decision
The admin defines the restaurant's tables, optionally grouped into sections, and the waiter picks a real table when opening a bill. Occupied tables are visible at a glance.
**Done when:** an admin can define tables and sections; a waiter opening a bill picks from the real tables; a table with an open bill shows as occupied to every waiter; and a table cannot hold two open bills at once.
- [ ] Design it (spec): `/architect tables and floor plan`

## Slice 3: the service loop, thickened

Thicken the two screens the restaurant actually lives in all evening. This is where the product stops being a demo and starts being usable on a busy night.

### 12. Waiter service flow · needs a decision
The waiter's real working screen: keep one bill open per table across the whole meal, send each round to the kitchen as its own ticket, watch every round's progress live, and get a sound and a badge the moment food is ready to pick up. Includes the free text note per line, such as no onions.
**Done when:** a waiter can add a second and third round to an open bill and each goes to the kitchen as its own ticket; the waiter's list shows every open order's live status; when a round becomes ready the waiter gets a sound and a clear badge and can mark it served; and per line notes reach the kitchen ticket unchanged.
- [ ] Design it (spec): `/architect waiter service flow`

### 13. Kitchen display · needs a decision
The chef's real working screen, readable across a kitchen: incoming tickets appear live in the order they arrived, each shows how long it has been waiting, and the chef taps each dish done as it comes off the pass. The ticket flips to ready by itself when the last dish lands.
**Done when:** a new ticket appears without a refresh; tickets are ordered oldest first and show elapsed time with a visible warning once one waits too long; tapping a dish marks only that dish; the ticket flips to ready automatically on the last dish and leaves the active queue; and the whole screen is legible at kitchen distance and usable with wet or gloved hands.
- [ ] Design it (spec): `/architect kitchen display`

## Slice 4: money

Thicken the closing segment: what the customer actually pays and what the restaurant keeps a record of. Bills and reports both depend on the tax rules being right, so they are decided here rather than patched in later.

### 14. Bill generation, currency, and tax · needs a decision
Turn a finished meal into a correct bill: every round totalled, the restaurant's own currency applied, its own tax rules applied, and an optional service charge. Each restaurant sets its own, since they may be in different countries.
**Done when:** an admin can set the restaurant's currency, tax rules, and service charge; a bill shows lines, subtotal, each tax component, service charge, and total; amounts and rounding are correct to the currency; and a closed bill keeps the rates it was charged at even if the restaurant changes them later.
- [ ] Design it (spec): `/architect bill generation, currency, and tax`

### 15. Closing a bill and recording payment · needs a decision
The customer has paid at the till or by card machine, so a staff member records how, closes the bill, and frees the table for the next party. No payment provider in the product.
**Done when:** a staff member can close a bill and record the payment method; the table is freed for a new bill immediately; a closed bill cannot be edited, only viewed; and closing is refused while any dish on the bill is still unserved unless it is explicitly voided with a reason.
- [ ] Design it (spec): `/architect closing a bill and recording payment`

### 16. Kitchen ticket and bill printing · needs a decision
Paper, because kitchens and customers still want it: a kitchen ticket printed when a round is sent, and a printable customer bill at the end.
**Done when:** sending a round produces a kitchen ticket on paper or as a clean printable sheet; a closed bill produces a printable customer bill carrying the restaurant's details and the tax breakdown; and a printing failure is surfaced to the staff member rather than swallowed.
- [ ] Design it (spec): `/architect kitchen ticket and bill printing`

## Slice 5: oversight

Thicken the admin's view. The floor already works; this is the owner watching it and learning from it.

### 17. Admin order monitor · Beta
The admin's live view over the whole floor: every ongoing order with its status and age, and a searchable history of completed ones. Read only, and it leans entirely on the live update pattern and the design system already decided.
**Done when:** an admin sees every ongoing order updating live with its table, waiter, status, and how long it has been open, and can look up a past order by table, date, or waiter.
- [ ] Build it: `/develop admin order monitor`

### 18. Sales reports · needs a decision
What the owner opens the laptop for: revenue by day, best and worst selling dishes, order volume by hour so staffing can match it, and activity per staff member.
**Done when:** an admin can pick a date range and see revenue, order count, average bill, dish ranking, volume by hour, and per staff activity; figures agree with the underlying bills; and the report loads quickly on a restaurant with a year of history.
- [ ] Design it (spec): `/architect sales reports`

## Slice 6: public face and launch readiness

The product works. This is what a restaurant sees before they trust it, and what the law expects to be there.

### 19. Public marketing page and signup · needs a decision
A public page explaining what the platform does, with a clear route into registering a restaurant, built so search engines and social previews handle it properly.
**Done when:** the page loads fast and renders for search engines, carries page metadata, a sitemap, and a social preview card, and its call to action leads into restaurant registration.
- [ ] Design it (spec): `/architect public marketing page and signup`

### 20. Privacy, terms, and cookie consent · Beta
The legal pages a real signup product needs, plus a consent banner that actually controls what runs before consent is given.
**Done when:** privacy and terms pages exist and are linked from the public page and from signup; the consent banner records a choice and nothing non essential runs before consent; and the choice can be changed later.
- [ ] Build it: `/develop privacy, terms, and cookie consent`

### 24. Password recovery by email `from spec 0006` · needs a decision
Spec [0006](../specs/0006-accounts-restaurants-and-roles/index.md) chose admin resets a password over an emailed link, which leaves one hole: an owner who is the only admin and forgets their password has nobody to ask. Closing it means an email provider (AWS SES on this stack), a single use token, and the rules around it. Worth doing before a restaurant that is not yours signs up, and before feature 21 charges anybody money.
**Done when:** somebody who cannot sign in can request a link, set a new password from it, and every other session of theirs is revoked; the link works once and expires; and asking for a link tells a stranger nothing about whether that address has an account.
- [ ] Design it (spec): `/architect password recovery by email`

## Deferred

Out of scope for the current build pass, kept so the plan stays honest. Both are things you asked for and both are large: each is closer to a second product than a feature, and neither should compete with getting the core loop right.

- **21. Subscription plans and paid signup**: monthly plans per restaurant, a paid signup flow, a payment provider, invoices, and what happens when a payment fails · needs a decision
- **22. Customer QR self ordering**: the customer scans a code at the table, browses the menu, and sends rounds to the kitchen without a waiter, which needs its own public surface, its own session handling, and its own abuse controls · needs a decision
- **23. Splitting and merging bills** `from spec 0003`: guests paying separately, and two joined tables paying as one. The schema in spec [0003](../specs/0003-core-data-model/index.md) permits both (lines carry a bill reference, and a visit owns the table rather than a bill), and nothing implements either. It needs the waiter screens for choosing which lines go where, which is the real work · needs a decision
- **25. Dish photos** `from spec 0008`: an image per dish on the admin and waiter screens. Needs upload, S3 storage, and resizing, new infrastructure the menu feature deliberately left out · needs a decision
- **26. Dish sizes and add ons** `from spec 0008`: half or full portions, extra cheese. Changes how a line's price is worked out and what a kitchen ticket shows, so it touches the order thread as much as the menu · needs a decision
- **27. Dish names in a second language** `from spec 0008`: an English name beside a Hindi one, say. Needs a translations shape for restaurant typed text and a rule for which name each screen shows · needs a decision

## Legend

**The decision box.** Every feature carries exactly one, the sub task whose label ends with `(spec)`. Its wording varies (`Design it (spec)` normally, `Decide the stack (spec)` on Stack and architecture), so skills locate it by that `(spec)` suffix, never by an exact label. Every other box is an execution box and `/architect` never ticks one.

**Feature lifecycle**: the scope updates as a feature moves; each row is what it shows and who sets it:

| State | Set by | The feature shows |
|---|---|---|
| `planned` · needs a decision | `/scope` | one box: `Design it (spec): /architect <feature>` |
| `in-progress` (designed) | **`/architect` at spec capture** | `Design it` ticked; spec linked; `Build it: /develop <feature>` plus 2 to 5 milestones; the tier's closing boxes (`Verify it`, `Test it`, `Review it`, `Document it`); any surfaced follow up enrolled |
| `in-progress` (building) | `/develop` | milestone sub boxes tick one by one; code pointer filled |
| `in-progress` (verified) | `/check verify` | `Build it` plus milestones ticked; `Verify it` ticked |
| `done` | **you, when you decide it is** (any skill sets it when you say so); `/sync` reconciles | the boxes you ran are ticked, ones you skipped are recorded as skipped; the tier's last stage (`Beta` and `GA` → after `/test`) is the *suggested* point to call it done, never a gate; `/sync` captures conventions |

- **Next step** = the first unticked box (always a command or a tracked milestone).
- **needs a decision** = run `/architect` first; otherwise straight to `/develop` (or `/audit` for standards and tooling). The tag drops once the spec is captured.
- **Atomic build tasks live in the spec's `## Build plan`, not here**: the scope carries only the milestone rollup.
- **Status** `planned` → `in-progress` → `done`, plus `existing` (pre workflow) and `dropped` (de scoped, kept for history).
- **Approach tag** beside a heading (e.g. `· Facade`) overrides the project default for that feature; no tag = inherits it.
- **Workflow tier tag** beside a heading (e.g. `· Beta`) overrides the project default `**Workflow:**` tier for that one feature; no tag = inherit. Two features carry a lighter `· Beta` tag because they are read only or static surfaces resting on patterns already decided elsewhere.
- **Workflow** (header line) is the project default tier, the stages each feature *suggests* running **after** `/develop`: **Prototype** = nothing; **Alpha** = `/check verify`; **Beta** = `/check verify` then `/test`; **GA** = adds a fresh model `/check review` then `/document`. `done` is your call, not gated on these; a skipped stage is recorded as skipped.
- **Accessibility** is set once in feature 5 and then applied by every later feature's acceptance criteria, rather than being a row of its own.
- **Pointer line** (`spec <n> · code in <path>`): the spec link added by `/architect`, the code path by `/develop`.
