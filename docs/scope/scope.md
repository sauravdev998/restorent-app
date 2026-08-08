# Scope: Restaurant Operations Platform

A web platform many restaurants sign up for, where an owner runs the restaurant and their waiters and chefs run the floor and the kitchen: orders taken at the table, tickets live in the kitchen, food marked ready per dish, and a bill at the end of the meal.

**Build approach:** Tracer Bullet (a thin thread pierces every layer and works, then you thicken it).
**Workflow:** GA (after `/develop`: `/check verify`, then `/test`, then a fresh model `/check review`, then `/document`). The project's default rigor tier; a feature's own tier tag (e.g. `· Beta`) overrides it.

_You are in charge. Every box below is a **suggestion**, not a gate: run any, skip any, and mark a feature `done` when you decide it is. The workflow records what you actually did (including "skipped"), it never requires a step. The one thing it asks is that a load bearing decision be written down (a spec), not that any check be run._

## At a glance

| # | Feature | Phase | Status |
|---|---------|-------|--------|
| 1 | Stack and architecture | Foundation | done |
| 2 | Coding standards and tooling | Foundation | planned |
| 3 | Error and crash monitoring | Foundation | planned |
| 4 | Core data model | Foundation | planned |
| 5 | Design system and accessibility baseline | Foundation | planned |
| 6 | Language and text foundation | Foundation | planned |
| 7 | Accounts, restaurants, and roles | Foundation | planned |
| 8 | The thin order thread | Slice 1 | planned |
| 9 | Menu management | Slice 2 | planned |
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

### 2. Coding standards and tooling
Capture the conventions from the real scaffolded project, then install lint, formatting, type strictness, and a pre commit hook so all later code follows one shape.
**Done when:** root `AGENTS.md` reflects the real stack, and lint, format, and the pre commit hook run clean.
- [ ] Capture conventions and tooling choices: `/audit`
- [ ] Install the tooling: `/develop tooling`

### 3. Error and crash monitoring · needs a decision
Know when the app breaks for a waiter mid shift instead of hearing about it from the restaurant. Early, so every later slice reports into it from its first day.
**Done when:** an unhandled error on any screen or server route arrives in the monitoring tool with enough context to find it, and you get alerted.
- [ ] Design it (spec): `/architect error and crash monitoring`

### 4. Core data model · needs a decision
The entities everything else is built on: restaurants, staff and their roles, menu categories and dishes, tables, bills, order rounds, order lines with per dish status, and payment records. Multi restaurant separation lives here, and it is the most expensive thing in the project to get wrong.
**Done when:** the schema supports one open bill per table with many rounds, per dish status, per restaurant currency and tax settings, and strict data separation between restaurants, all without a breaking change when later slices land.
- [ ] Design it (spec): `/architect core data model`

### 5. Design system and accessibility baseline · needs a decision
The visual language and base components every screen uses, built for three very different contexts: an admin on a desktop, a waiter on a phone, a chef on a kitchen screen read from a distance. The accessibility target is set here and then applied by every later feature rather than being its own row.
**Done when:** `design.md` covers type, colour, spacing, and the base components; components handle keyboard use and focus; the chosen accessibility level is written down and the base components meet it.
- [ ] Design it (spec): `/architect design system and accessibility baseline`

### 6. Language and text foundation · needs a decision
Every piece of text in the app comes from a translation file from the first screen onward, plus how a user's language is chosen and stored. Cheap now, painful to retrofit once twenty screens exist.
**Done when:** no screen has hard coded user facing text, a second language can be added by dropping in one file, and a staff member's language choice sticks across sessions.
- [ ] Design it (spec): `/architect language and text foundation`

### 7. Accounts, restaurants, and roles · needs a decision
An owner registers a restaurant and becomes its admin, staff sign in, and every request is limited to that person's restaurant and role. Real authentication and real data separation from the very first slice, never faked.
**Done when:** an owner can register a restaurant and sign in as its admin; a signed in user only ever sees their own restaurant's data; a waiter cannot reach admin or chef screens and a chef cannot reach admin or waiter screens, on the server as well as in the interface.
- [ ] Design it (spec): `/architect accounts, restaurants, and roles`

## Slice 1: the thin order thread

One narrow path pushed through every layer, working for real. No breadth: one table, one dish, one round, the plainest screens. This proves the whole pipe connects, which is the scariest risk in the project, and it is also the walking skeleton. Everything after this thickens one segment of this thread.

### 8. The thin order thread · needs a decision
A waiter opens a bill on a table and adds one dish, the kitchen sees the ticket appear live, the chef marks the dish done, the ticket flips to ready and the waiter's screen updates with a sound, and the waiter closes the bill with a total. Real database, real login, real screens, narrow on purpose.
**Done when:** on two devices at once, a dish sent by the waiter appears on the kitchen screen within a second or two without a refresh; marking it done flips the ticket to ready and alerts the waiter the same way; closing the bill records a total; and the order's state changes are safe when two people act at the same time.
- [ ] Design it (spec): `/architect the thin order thread`

## Slice 2: one restaurant set up for real

Thicken the setup segment, so a real restaurant's own menu, staff, and tables drive the thread rather than the one hardcoded dish and table from slice 1.

### 9. Menu management · needs a decision
The admin builds the real menu: categories, dishes with a price, and a switch to mark a dish unavailable when the kitchen runs out, which immediately stops waiters ordering it.
**Done when:** an admin can create, edit, reorder, and remove categories and dishes; marking a dish unavailable removes it from the waiter's ordering screen at once; a dish already on an open bill is unaffected.
- [ ] Design it (spec): `/architect menu management`

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

## Deferred

Out of scope for the current build pass, kept so the plan stays honest. Both are things you asked for and both are large: each is closer to a second product than a feature, and neither should compete with getting the core loop right.

- **21. Subscription plans and paid signup**: monthly plans per restaurant, a paid signup flow, a payment provider, invoices, and what happens when a payment fails · needs a decision
- **22. Customer QR self ordering**: the customer scans a code at the table, browses the menu, and sends rounds to the kitchen without a waiter, which needs its own public surface, its own session handling, and its own abuse controls · needs a decision

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
