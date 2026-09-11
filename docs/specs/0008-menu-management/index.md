# 0008. Menu management

**Date**: 2026-09-10
**Status**: In Progress

## Summary

This decision gives the admin a real menu to run: categories and dishes they create, rename, price,
reorder by dragging, remove, and bring back, all on one screen laid out like the printed menu. The
chef (and the admin) can switch a dish off when the kitchen runs out, and every waiter's ordering
screen greys it within a second or two, with nothing refreshed by hand. Every dish now carries the
familiar veg, non veg, or egg mark. Lines already sent to the kitchen never change, whatever happens
to the menu afterwards. The build is mostly screens and endpoints over the schema spec 0003 already
made, plus one small migration and one new library (dnd-kit, for drag and drop).

## Requirements

**User stories**:

- As an admin, I want to build my restaurant's real menu (categories, dishes, prices, the veg mark)
  so that waiters take orders from what we actually serve, not a seeded placeholder.
- As an admin, I want to drag dishes and categories into the order my printed menu uses so that the
  waiter's screen reads the way the customer's menu does.
- As a chef, I want to switch off a dish the moment we run out, from the kitchen screen, so that no
  waiter sells a plate I cannot make.
- As a waiter, I want a dish that just ran out to grey out on my screen and in my unsent basket so
  that I never promise it and never send a ticket the kitchen will bounce.
- As an admin, I want a removed dish to be recoverable so that a seasonal dish comes back without
  being typed in again, and a mistaken click costs nothing.

**Acceptance criteria** (the contract, each independently checkable):

- **AC-1**: An admin can create a category by name. It appears at the end of the category list on
  the admin screen at once, and on every waiter's ordering screen within about two seconds, without a
  refresh, as soon as it holds a live dish (a category with no live dish stays hidden from waiters,
  as spec 0007 already does).
- **AC-2**: An admin can create a dish with a category, a name, an optional description, a price, and
  a diet marker. It is created available, lands at the end of its category, and appears on every
  waiter's ordering screen and on the chef's Menu tab within about two seconds, without a refresh.
- **AC-3**: An admin can rename a category, and can edit a dish's name, description, price, diet
  marker, and category. A dish moved to another category lands at the end of it. Every change
  reaches waiter screens and the chef's Menu tab within about two seconds, without a refresh.
- **AC-4**: An admin can reorder the categories, and the dishes within one category, by drag and drop
  with a mouse, with touch, and with the keyboard alone. Every step of a drag is announced through a
  screen reader in the interface language. The order the admin sets is the order waiters see.
  Dragging a dish into a different category is not possible; moving it is done in the edit form.
- **AC-5**: A reorder is sent as the complete list of ids in their new order. When that list does not
  match the live set at the moment it arrives (a dish was added, moved, or removed meanwhile), it is
  refused with `409 menu_changed`, nothing is saved, and the admin's screen refetches and shows the
  server's current order with a translated message.
- **AC-6**: Removing a dish or a category first asks for confirmation. A removed item leaves the
  admin's working list and appears in the Archived section at once, disappears from every other
  screen within about two seconds, and every
  order line, round, and bill that referenced it still resolves and reads exactly as before.
- **AC-7**: Removing a category that still holds a live dish is refused with `409
  category_not_empty`. This holds when a dish is being created in or moved into that category at the
  same moment: no live dish ever sits in an archived category.
- **AC-8**: An admin can restore an archived category or dish from the Archived section. Restoring a
  dish asks which live category to put it in, defaulting to its old one when that is still live; it
  lands at the end of that category and keeps its name, description, price, diet marker, and
  availability. A restored item is back in the admin's working list at once and on every other
  screen within about two seconds. A restore whose name clashes with a live item of the same kind is refused with `409
  name_taken`.
- **AC-9**: An admin or a chef can switch a dish's availability. Switched off, it greys on every
  waiter's ordering screen and cannot be added to a basket within about two seconds, without a
  refresh; switched on, it is orderable again. It stays as set until someone switches it back.
  Setting the value it already has succeeds and changes nothing.
- **AC-10**: The kitchen screen has a Menu tab beside the ticket list, listing every live dish grouped
  by category with a switch per dish, at kitchen density. Availability is the only menu change a chef
  can make.
- **AC-11**: No menu change touches a line already sent. After a dish on an open bill is edited,
  repriced, renamed, moved, switched off, or removed, every existing line keeps its status, name,
  price, and line total, and the open bill's subtotal is unchanged.
- **AC-12**: When a dish in a waiter's unsent basket becomes unavailable or is removed, the basket
  flags that line within about two seconds and blocks sending until the waiter takes it out. A send
  that races past the flag is refused whole with `409 dish_not_orderable`: no round is created, and
  the waiter's screen refetches the menu and flags the line.
- **AC-13**: Every dish carries a required diet marker, one of veg, non veg, or egg, drawn as the
  standard mark on the waiter's ordering screen, the chef's Menu tab, and the admin screen (live and
  Archived rows alike). All three marks sit inside a square outline: veg is a filled circle, non veg
  a filled triangle, egg a filled oval. It is never colour alone: each mark has its own shape and a
  translated accessible name, the marks pass the contrast gate in dark, light, and print, and they
  stay distinct under forced colours.
- **AC-14**: Names are required and trimmed, at most 60 characters for a category, 80 for a dish, and
  300 for a description. Dish names are unique among live dishes, and category names among live
  categories, ignoring letter case and edge spaces. A price is a decimal string, zero or more, below
  10,000,000,000 (what `numeric(14,4)` holds), with no more decimal places than the restaurant's
  currency uses. Each refusal is a field error beside the field it concerns, translated from its code
  (`required`, `too_long`, `already_taken`, `not_a_number`, `negative`, `too_large`,
  `too_many_decimals`).
- **AC-15**: An edit or rename carrying a version older than the stored one is refused with `409
  dish_changed` or `409 category_changed`, nothing is written, and the admin sees a translated message
  and the current values. An availability switch is never refused as stale, and it makes any open
  edit form for that dish stale.
- **AC-16**: Role limits hold on the server. Every `/api/admin/menu` endpoint is admin only; the
  availability switch is admin or chef; `GET /api/menu` is waiter or chef. Anyone else receives `403`
  before the handler body runs, a signed out caller receives `401`, an id belonging to another
  restaurant reads as `404`, and every endpoint appears in the OpenAPI document with its role.
- **AC-17**: Creating, renaming or editing (including a move and a price change), removing, and
  restoring a dish or a category, and changing a dish's availability, each write exactly one audit
  row carrying the actor, the entity, and the before and after values. A reorder writes none, and an
  availability switch to the value it already had writes none.
- **AC-18**: Every menu change notifies inside the transaction that made it. Category changes use a
  new `menu_category` entity kind; the fan out map sends both `dish` and `menu_category` events to the
  admin menu, the waiter's menu, and the chef's Menu tab, and to nothing else they do not feed.
- **AC-19**: The admin menu lives at `/admin/menu`, reached from the admin shell's navigation. It is
  one page with categories as stacked sections, dish add and edit in a `Dialog`, an empty state for a
  restaurant with no menu that leads to adding the first category, and skeletons on first load. No
  screen touched here carries a user facing string written into a component; English and Hindi keys
  stay at parity; lint, axe, and the contrast script pass.
- **AC-20**: One Playwright run drives an admin (or chef) context and a waiter context at once: a dish
  the admin creates appears on the waiter's ordering screen, and a dish switched off greys there, both
  with no reload.

## Decision

**Chosen option**: Option 1: a live, directly edited menu over spec 0003's schema.

Build the admin menu screen, the chef's availability tab, and the endpoints behind them on the
existing `menu_categories` and `dishes` tables, adding one migration (the diet marker, a version
number per row, and case insensitive live name uniqueness) and one web library (`@dnd-kit/core` with
`@dnd-kit/sortable`). Every change is live the moment it commits; there is no draft menu.

**Implementation skills**: `axum-web-framework` (`manutej/luxor-claude-marketplace`, `.agents/skills/axum-web-framework/`) · `rust-backend` (`windmill-labs/windmill`, `.agents/skills/rust-backend/`) · `rust-best-practices` (`apollographql/skills`, `.agents/skills/rust-best-practices/`) · `sqlx-postgres` (`daiki48/dotfiles`, `.agents/skills/sqlx-postgres/`) · `postgresql-table-design` (`wshobson/agents`, `.agents/skills/postgresql-table-design/`) · `react-router-data-mode` (`remix-run/agent-skills`, `.agents/skills/react-router-data-mode/`) · `tanstack-query` (`tanstack-skills/tanstack-skills`, `.agents/skills/tanstack-query/`) · `shadcn` (`shadcn/ui`, `.agents/skills/shadcn/`) · `react-i18next` (`yildizberkay/skills`, `.agents/skills/react-i18next/`) · `vitest` (`antfu/skills`, `.agents/skills/vitest/`) · `implementing-drag-drop` (`ancoleman/ai-design-components`, `.agents/skills/implementing-drag-drop/`) · `accessibility` (`addyosmani/web-quality-skills`, `.agents/skills/accessibility/`)

**Settled here, not asked** (each with the runner up):

- **Category events get their own kind, `menu_category`**, added to `EntityKind`, to `ENTITY_KINDS`,
  and to spec 0003's Notifies vocabulary together. Runner up: notify categories as `dish`, which
  needs no vocabulary change but names the wrong entity in every log line.
- **Two new role requirements, `AdminOrChef` and `WaiterOrChef`**, beside `Admin`, `Waiter`, `Chef`
  in `presentation/extract/actor.rs`, each a one line `permits`. Runner up: a separate
  `/api/kitchen/menu` read for the chef, which duplicates `/api/menu` for no gain.
- **The availability switch sets an absolute value and skips the version check.** "Off" is the
  chef's whole intent, so refusing it as stale would only make them tap twice. It still bumps the
  version when it changes the value. Runner up: toggle semantics, which two quick taps turn into a
  no op.
- **Stale detection is a conditional update, `WHERE id = $1 AND version = $2`.** Zero rows then
  means stale or missing, told apart by one follow up read. Runner up: `SELECT ... FOR UPDATE` and
  compare, one more statement for the same answer.
- **A new item goes to the end of its list** (`max(position) + 1` among live siblings, `1` when
  empty). Positions need not be contiguous; order is `position`, then `name`, then `id`. Runner up:
  renumber on every insert, more writes for nothing a reader sees.
- **A reorder rewrites positions `1..n` for the sibling list**, after locking those rows `FOR
  UPDATE ... ORDER BY id` (a fixed lock order, so two concurrent reorders queue rather than deadlock)
  and checking that the sent list has the same length as the live set and the same ids, which also
  refuses a list with one id repeated and another missing. No unique constraint on `position`, so no
  deferred constraint is needed mid rewrite.
- **The availability switch only touches a live dish.** Its update matches `id = $1 AND archived_at
  IS NULL`; a dish removed a moment earlier reads as `404`, the same as an unknown id.
- **A dropped reorder is held in component state while its request is pending**, not in the query
  cache. On success the admin menu is invalidated; on refusal the held order is dropped and the admin
  menu is refetched, so the list shows the server's current order, never the stale one held before
  the conflict. This keeps spec 0007's rule of no cache optimism.
- **A confirmed admin write refreshes the admin's own screen directly.** Every successful admin menu
  mutation invalidates `['dish']` (the admin menu and the waiter menu) in its `onSuccess`, so the
  admin sees the change at once instead of waiting on the event round trip. This is not cache
  optimism: spec 0007's rule forbids writing an unconfirmed result into the cache, and this is a
  refetch after a confirmed one. Every other screen still learns of the change through the event.
  Runner up: rely on the event alone, which leaves the admin looking at the old row for a second or
  two after saving.
- **The dish's category is checked before its version on an edit.** The target category is locked
  `FOR SHARE` and must be live (`409 category_archived`), then the conditional version update runs
  (`409 dish_changed`). An edit that is both stale and aimed at a removed category therefore reports
  the category, the thing the admin must change first.
- **The dish rows in the admin screen are a sortable list (`ul` of row cards), not `DataTable`.**
  `DataTable` has real table semantics for sorting data; a drag list reads better to a screen reader
  as a list, and dnd-kit's sortable preset expects one.
- **A new base `Switch` component** in `web/src/shared/ui/`, from the shadcn switch (Radix), meeting
  the spec 0004 component standard. Nothing in `shared/ui/` switches a boolean today.
- **Price input** goes through a new `parseDecimalInput` in `web/src/shared/format/`, accepting ASCII
  digits with either `.` or the formatting locale's decimal separator, refusing grouping separators,
  and producing a plain decimal string. The wire carries the price as a string, never a JSON number,
  as `MenuDishDto.price` already does. The API is the authority on every rule in AC-14.
- **No pagination on the menu reads.** A restaurant menu is a few hundred rows at most, read whole as
  one printed menu; paging it would break the single page layout the admin chose.
- **Length limits are checked by the API as field errors and backed by check constraints** in the
  migration, the same pairing `not_blank` already uses.

## Rationale

Reasoning and options: see [rationale.md](rationale.md).

## Feature design

**Data model sketch**

One migration, `api/migrations/0006_menu_management.sql`. Everything not listed is spec 0003's
schema, used as it stands.

| Table | Column or index | Type and rule | Change |
|---|---|---|---|
| (type) | `dish_diet` | enum `veg`, `non_veg`, `egg` | new |
| `menu_categories` | `version` | `int not null default 1` | new |
| `menu_categories` | `menu_categories_name_length` | check `char_length(name) <= 60` | new |
| `menu_categories` | `menu_categories_live_name_key` | unique `(restaurant_id, lower(name))` where `archived_at is null` | replaces the case sensitive index of the same name |
| `dishes` | `diet` | `dish_diet not null` | new. Added with `default 'veg'` so existing development rows fill, then the default is dropped so every insert names one |
| `dishes` | `version` | `int not null default 1` | new |
| `dishes` | `dishes_name_length`, `dishes_description_length` | check `char_length(name) <= 80`, `description is null or char_length(description) <= 300` | new |
| `dishes` | `dishes_live_name_key` | unique `(restaurant_id, lower(name))` where `archived_at is null` | new |
| `dishes` | `dishes_category_order_idx` | `(restaurant_id, category_id, position)` where `archived_at is null` | new. Serves the ordered list and the "is this category empty" check |

Names are trimmed by the API before they are written (`require_name` already does), which is why
`lower(name)` alone is enough to ignore edge spaces.

Relationships are unchanged: restaurant 1:N category, category 1:N dish (composite foreign key on
`(category_id, restaurant_id)`), dish 1:N order line. A line copies the dish's name and price at send
and never reads the dish again.

Rust side: a `Diet` enum mirroring `dish_diet` through `sqlx::Type`; `version: i32` on `MenuCategory`
and `Dish`; `DishEdit` loses `is_available` and gains `category_id`, `diet`, and `version`; new
`AuditAction` variants `MenuCategoryCreated`, `MenuCategoryRenamed`, `MenuCategoryArchived`,
`MenuCategoryRestored`, `DishCreated`, `DishRestored`, `DishAvailabilityChanged` (the existing
`DishEdited` and `DishArchived` stay, with `DishEdited` now recording `category_id` and `diet` in its
before and after); new `ConflictKind` variants `DishChanged`, `CategoryChanged`, `MenuChanged`,
`CategoryNotEmpty`, `CategoryArchived`, `NameTaken`, `DishNotOrderable`.

The seed (`api/src/bin/seed.rs`) sets the right diet on its six dishes, including on a database where
they already exist, so a developer's greyed fish pakora reads as non veg.

**State transitions**

- **Category**: `live` → `archived` (remove, refused while it holds a live dish) → `live` (restore,
  refused on a live name clash). Rename and reorder happen while live.
- **Dish**: `live` → `archived` (remove) → `live` (restore into a chosen live category, refused on a
  live name clash). While live, availability moves `available` ⇄ `unavailable` by the switch only,
  and nothing else writes it.
- **Version**: every write to a row that changes it increments `version` by one: rename, edit, move,
  availability change, archive, restore. A reorder does not, because it changes only `position`,
  which no edit form carries.

**API surface**

Every endpoint sits under the existing router, the origin check, and the 30 second timeout, carries
its role in the handler's own `Actor<R>` type, and is listed in `presentation/openapi.rs`. Admin
handlers live in a new `handlers/admin_menu.rs`; the availability switch sits in `handlers/menu.rs`
beside the read it changes. Writes run in `Database::begin_scoped`; the two reads run in
`begin_scoped_snapshot`, because each assembles a document from several statements.

| Endpoint | Method | Key inputs | Key outputs | Auth | Key errors |
|---|---|---|---|---|---|
| `/api/admin/menu` | GET | none | live categories in order, each with `id`, `name`, `version`, and every live dish (`id`, `name`, `description`, `price`, `diet`, `available`, `version`); `archived` with categories (`id`, `name`, `archivedAt`) and dishes (`id`, `name`, `diet`, `categoryId`, `categoryName`, `categoryLive`, `archivedAt`); `currencyCode`, `currencyDecimals` | admin | `401`, `403` |
| `/api/admin/menu/categories` | POST | `name`: string (req) | the category | admin | `400` field errors |
| `/api/admin/menu/categories/{id}` | PUT | `name`: string (req), `version`: int (req) | the category | admin | `400` field errors, `404`, `409 category_changed` |
| `/api/admin/menu/categories/order` | PUT | `ids`: uuid[] (req), every live category exactly once | the reordered list | admin | `409 menu_changed` |
| `/api/admin/menu/categories/{id}/archive` | POST | path id | the archived category | admin | `404`, `409 category_not_empty` |
| `/api/admin/menu/categories/{id}/restore` | POST | path id | the category | admin | `404`, `409 name_taken` |
| `/api/admin/menu/categories/{id}/dish-order` | PUT | `ids`: uuid[] (req), every live dish of that category exactly once | the reordered list | admin | `404`, `409 menu_changed` |
| `/api/admin/menu/dishes` | POST | `categoryId` (req), `name` (req), `description` (opt), `price`: decimal string (req), `diet` (req) | the dish | admin | `400` field errors, `409 category_archived` |
| `/api/admin/menu/dishes/{id}` | PUT | `categoryId`, `name`, `description` (nullable), `price`, `diet`, `version` (all req) | the dish | admin | `400` field errors, `404`, `409 dish_changed`, `409 category_archived` |
| `/api/admin/menu/dishes/{id}/archive` | POST | path id | the archived dish | admin | `404` |
| `/api/admin/menu/dishes/{id}/restore` | POST | `categoryId` (req) | the dish | admin | `404`, `409 name_taken`, `409 category_archived` |
| `/api/dishes/{id}/availability` | PUT | `available`: bool (req) | the dish | admin or chef | `404` |
| `/api/menu` | GET | none | unchanged, plus `diet` on each dish | waiter or chef (was waiter) | `401`, `403` |
| `/api/visits/{id}/rounds` | POST | unchanged | unchanged | waiter | the unavailable or archived dish refusal becomes `409 dish_not_orderable` (today a plain `400 invalid`) |

Field error codes, on the existing `fields` member of the error body: `name`: `required`,
`too_long`, `already_taken`; `description`: `too_long`; `price`: `required`, `not_a_number`,
`negative`, `too_large`, `too_many_decimals`; `diet`: `required`; `categoryId`: `required`. A unique
violation on either live name index is caught and turned into `name: already_taken`, the way
`handlers/auth.rs` already turns `email_taken` into a field error, so a race between two creates reads
the same as a plain clash.

**Value sourcing**

| Action | Value produced or displayed | Source |
|---|---|---|
| every request | which restaurant | the session, resolved by `Actor`, passed to `begin_scoped`. No body or path names a restaurant |
| every write | the audit actor | `Actor`'s staff id |
| create category or dish | its id | uuid v7 generated in Rust |
| create category or dish, move, restore | its position | `max(position) + 1` over live siblings in the same transaction (`1` when there are none), under the sibling lock described in Key invariants |
| create dish | its availability | `true`. The create form has no switch |
| create or edit dish | the price | the `price` string, parsed with `rust_decimal`, checked against AC-14 |
| create or edit dish | how many decimals a price may carry | `restaurants.currency_decimals`, read in the same transaction |
| create or edit dish | the largest price | `numeric(14,4)`'s bound: below 10,000,000,000 |
| edit, rename | whether the caller's copy is current | the `version` the form loaded, compared in the conditional update |
| edit dish | whether the move target is live | `menu_categories.archived_at is null`, read `FOR SHARE` |
| reorder | what the complete list must be | the live sibling ids, read `FOR UPDATE` in the same transaction |
| reorder | the new positions | the index of each id in the sent list, `1..n` |
| archive category | whether it is empty | count of live dishes in it, after locking the category row `FOR UPDATE` |
| restore dish | its category | the `categoryId` input, which the dialog defaults to `categoryId` from the archived list when `categoryLive` is true |
| restore | whether the name is free | the live name index, caught as a unique violation and returned as `409 name_taken` |
| availability switch | whether anything changes | the stored `is_available` compared to `available`. Equal: no write, no version bump, no audit row, no notify |
| admin and waiter screens, chef tab | a price's currency and decimals | `restaurants.currency_code` and `currency_decimals`, carried on both menu responses, written through `shared/format` |
| every screen | the diet mark | `dishes.diet`, drawn by a new `DietMark` in `shared/ui/`: a square outline holding a filled circle (veg), triangle (non veg), or oval (egg) |
| every screen | the diet mark's colour | three new tokens, `--diet-veg` (green), `--diet-non-veg` (brown), `--diet-egg` (amber). `/develop` picks the values; the contrast script holds each at 3:1 or better against its surface in dark, light, and print, and under forced colours the marks fall back to `CanvasText`, their shapes carrying the meaning |
| admin screen | a live item appearing, changing, or leaving at once after the admin's own save | the admin write's `onSuccess` invalidating `['dish']`, per the settled item above |
| reorder refused | the order shown afterwards | a refetch of `['dish', 'admin']`, never the order held before the conflict |
| every screen | the diet mark's accessible name | `t('common:diet.veg')`, `t('common:diet.non_veg')`, `t('common:diet.egg')` |
| admin archived list | whether a dish's old category can take it back | `categoryLive`, derived from the category's `archived_at` in the admin read |
| waiter basket | whether a basket line is still orderable | the cached menu (`['dish', 'menu']`): a dish absent from it, or present with `available: false`, flags the line |
| send round | whether each dish is orderable | `dishes.archived_at is null and is_available`, read by `send_round` as today |
| drag announcements | every word a screen reader hears | dnd-kit's `announcements` and `screenReaderInstructions`, filled from `t()` in the `admin` namespace |
| live update | what to refetch | the fan out map below |
| a refused request | the sentence shown | the `error` code, or each field code, mapped to a key by `shared/api/error-message.ts` and `field-errors.ts` |
| kitchen Menu tab | its language | the restaurant's default language, as every kitchen screen (spec 0005) |

Fan out map changes in `web/src/shared/events/query-keys.ts`. A new key `adminMenuKey = ['dish',
'admin']` sits beside `menuKey = ['dish', 'menu']`; the chef's tab reads `menuKey`.

| Event entity | Invalidates |
|---|---|
| `dish` | `['dish']`, `['visit']` (unchanged) |
| `menu_category` | `['dish']` (new) |

**Key invariants**

1. No live dish sits in an archived category. Archiving a category locks its row `FOR UPDATE` before
   counting live dishes; creating, moving, and restoring a dish lock the target category `FOR SHARE`
   and require it live. The two lock modes conflict, so the check and the write cannot interleave.
2. At most one live dish, and one live category, per restaurant per lowercased trimmed name. Enforced
   by the two partial unique indexes, not only by a read before the write.
3. `is_available` is written only by the availability endpoint. No edit, create, move, or restore
   path writes it (create sets `true`; restore keeps what it was).
4. Every write that changes a row increments its `version` in the same statement.
5. A reorder either rewrites every position in the sibling list or none of them.
6. No menu write touches `order_lines`, `order_rounds`, or `bills`. A line's name and price were
   copied at send and are never recomputed.
7. Every menu write notifies (`dish` or `menu_category`) inside its own transaction, and every
   consequential one in AC-17 writes its audit row in that same transaction.
8. No client names a price for an order line or a restaurant for anything. The admin names a dish's
   price; `send_round` alone copies it onto a line.

**Security model**

- **Admin only**: every `/api/admin/menu` endpoint, through `Actor<Admin>`.
- **Admin or chef**: the availability switch, through `Actor<AdminOrChef>`. A chef's whole menu power
  is this one boolean.
- **Waiter or chef**: `GET /api/menu`, through `Actor<WaiterOrChef>`. The admin reads the fuller
  `/api/admin/menu` instead.
- **Tenant separation is unchanged.** Every statement runs in a `ScopedTx`; an id from another
  restaurant is invisible to row level security and reads as `404`, and the composite foreign key
  refuses a category from another restaurant even if one were named.
- **Browser gates are convenience.** The admin route group and the kitchen tab hide what a role
  cannot do; the server refuses regardless.
- **Audit log**: required (AC-17), because a price change is money and spec 0003 already demands it.
- **Compliance scope**: none new. Menu data is the restaurant's own business data, with no personal
  data and no payment data.

**Configuration required**

None. No environment variable, no secret, no new service. One new web dependency pair,
`@dnd-kit/core` (6.3.x) and `@dnd-kit/sortable` (10.x), both declaring React `>=16.8` as their peer,
which React 19 satisfies. The newer `@dnd-kit/react` is deliberately not used while it is below 1.0.

**Critical test scenarios**

- Happy path, two browsers: the admin creates a dish and the waiter's ordering screen shows it with
  no reload; the chef switches it off from the Menu tab and the waiter's screen greys it with no
  reload. Verifies **AC-2**, **AC-9**, **AC-10**, **AC-18**, **AC-20**.
- Happy path, API: create two categories and three dishes, edit a price, move a dish, reorder both
  lists, archive and restore, asserting rows, positions, versions, and audit rows at each step.
  Verifies **AC-1**, **AC-2**, **AC-3**, **AC-6**, **AC-8**, **AC-17**.
- Failure case, stale edit: load a dish (version 3), switch it off (version 4), then save the edit
  form carrying version 3; the save is refused `409 dish_changed` and the dish stays off. Verifies
  **AC-9**, **AC-15**.
- Failure case, the category race: archive a category while a concurrent transaction creates a dish
  in it; exactly one wins, and the database never holds a live dish in an archived category.
  Verifies **AC-7**.
- Failure case, stale reorder: send a dish order missing a dish created a moment earlier, and another
  with one id repeated in place of a missing one; both refused `409 menu_changed`, no position
  changed, and the screen shows the server's current order, including the new dish. Verifies
  **AC-5**.
- Failure case, removal races: switch availability on a dish archived a moment earlier (`404`), and
  save an edit that is both stale and aimed at an archived category (`409 category_archived`).
  Verifies **AC-9**, **AC-15**.
- Failure case, the basket race: a waiter's basket holds a dish, the chef switches it off, and the
  send goes out before the refetch lands; the round is refused whole with `409 dish_not_orderable`,
  no round or line exists, and the line is flagged after the refetch. Verifies **AC-12**.
- Failure case, sent lines untouched: with a dish on an open bill, reprice, rename, move, switch off,
  and archive it; every existing line and the bill's subtotal read exactly as before. Verifies
  **AC-11**.
- Failure case, validation: a blank name, an 81 character dish name, `Paneer Tikka ` beside a live
  `paneer tikka`, a price of `12.345` in rupees, `-1`, `abc`, and `10000000000`, each refused with its
  own field code and a translated sentence. Verifies **AC-14**.
- Failure case, restore clash: archive `Dal makhani`, create a new `Dal Makhani`, restore the old one;
  refused `409 name_taken`. Verifies **AC-8**.
- Keyboard drag: reorder a dish with Space, the arrow keys, and Space again, with the announcements
  asserted in English and Hindi, and no pointer used. Verifies **AC-4**.
- Diet mark: each mark renders a distinct shape with its accessible name, and the contrast script
  checks the three diet tokens in dark, light, and print. Verifies **AC-13**.
- Auth and permission: a chef calling any `/api/admin/menu` endpoint, a waiter calling the
  availability switch, and an admin calling `GET /api/menu` each receive `403`; a signed out caller
  receives `401`; restaurant A editing restaurant B's dish id receives `404`. Verifies **AC-16**.

## Build plan

Tracer Bullet: the first milestone pushes the scope's core promise (an admin adds a dish, the kitchen
switches it off, the waiter sees both live) through every layer on the plainest screens, then the
rest thickens it. The migration is small and lands whole in the first milestone, because a column
added in one milestone and constrained in the next buys nothing.

**Milestone 1: the thread, top to bottom**

1. Write migration `0006_menu_management.sql` per the data model sketch, run it, and add the `Diet`
   enum and the `version` fields to the domain types. Satisfies **AC-2**, **AC-13**, **AC-14**.
2. Add `EntityKind::MenuCategory` (`menu_category`), the matching entry in `ENTITY_KINDS`, and the
   fan out row. Satisfies **AC-18**.
3. Add `AdminOrChef` and `WaiterOrChef` to `presentation/extract/actor.rs`; widen `GET /api/menu` to
   `WaiterOrChef` and add `diet` to `MenuDishDto`. Satisfies **AC-10**, **AC-16**.
4. Build `GET /api/admin/menu`, `POST /api/admin/menu/dishes`, and `PUT
   /api/dishes/{id}/availability` (absolute value, live dishes only, no op when equal, version bump,
   `DishCreated` and `DishAvailabilityChanged` audit rows, notify). Every admin write invalidates
   `['dish']` in its `onSuccess`. Satisfies **AC-2**, **AC-9**, **AC-17**.
5. Build the plainest `/admin/menu` screen (sections, dish rows, an add dish `Dialog`, an availability
   toggle), a Menu link in the admin shell, and the kitchen Menu tab at `/kitchen/menu` with a plain
   toggle per dish. Satisfies **AC-2**, **AC-9**, **AC-10**, **AC-19**.
6. Regenerate the `.sqlx` cache and the typed client, and add the Playwright run with an admin and a
   waiter context. Satisfies **AC-20**.

**Milestone 2: the whole edit surface, and its rules**

7. Build category create and rename, and dish edit with move, each with the version check and its
   conflict code (on an edit, the target category check first, then the version), the field errors of AC-14, and the unique violation caught as `already_taken`.
   `DishEdit` loses `is_available`. Satisfies **AC-1**, **AC-3**, **AC-14**, **AC-15**.
8. Add the audit rows for category create and rename, and extend `DishEdited` to record category and
   diet. Satisfies **AC-17**.
9. Add `parseDecimalInput` to `shared/format/`, the dish and category forms on `Field`, `Input`,
   `Select`, and `Dialog`, and the translated field and conflict messages. Satisfies **AC-3**,
   **AC-14**, **AC-15**.
10. Add `DietMark` to `shared/ui/` (square outline; circle, triangle, oval) with the three diet
    tokens, the new pairs in
    `web/scripts/check-contrast.ts`, the forced colours treatment, and the exception written into
    `docs/design.md`; use it on all three screens. Satisfies **AC-13**.

**Milestone 3: order**

11. Build the two reorder endpoints: lock the siblings `ORDER BY id`, check length and id set, rewrite
    `1..n`, refuse with `menu_changed`, notify, no audit row. Satisfies **AC-4**, **AC-5**.
12. Add `@dnd-kit/core` and `@dnd-kit/sortable`; make both lists sortable with the pointer, touch,
    and keyboard sensors, translated announcements and instructions, the pending order held in
    component state, and on refusal the held order dropped, the admin menu refetched, and a
    translated message shown. Satisfies **AC-4**,
    **AC-5**.

**Milestone 4: remove and restore**

13. Build dish archive (extend `archive_dish` with the version bump) and category archive with the
    `FOR UPDATE` then count rule and `category_not_empty`; make create, move, and restore take the
    category `FOR SHARE` and refuse `category_archived`. Add `notify` and audit rows to category
    archive and restore, which today write neither. Satisfies **AC-6**, **AC-7**, **AC-17**.
14. Build both restore endpoints with `name_taken`, and the admin screen's confirm dialogs, Archived
    section, and restore dialog with its category picker. Satisfies **AC-6**, **AC-8**.

**Milestone 5: the waiter's side, the chef's tab finished, and proof**

15. Turn `send_round`'s unavailable or archived refusal into `ConflictKind::DishNotOrderable`, and
    on the waiter's screen flag basket lines against the cached menu, block sending while any is
    flagged, and refetch on `409 dish_not_orderable`. Satisfies **AC-12**.
16. Add the base `Switch` to `shared/ui/` and use it on the chef's tab (kitchen density, grouped by
    category) and the admin rows. Satisfies **AC-9**, **AC-10**.
17. Finish every screen's empty, loading, and error states, the English and Hindi keys at parity
    across `admin`, `kitchen`, `waiter`, and `common`, and the lint and axe passes. Satisfies
    **AC-19**.
18. Write the integration tests for every critical scenario against a real Postgres as `app_api`, the
    Vitest units (`parseDecimalInput`, `DietMark`, `Switch`, the basket flag, the fan out map), and
    extend the Playwright run with the switch off step; then `pnpm sqlx:prepare`, `pnpm
    client:generate`, and `pnpm check`. Satisfies **AC-11**, **AC-16**, **AC-20**.

## Consequences

**Positive**

- A real restaurant can run its own menu, which is the whole point of slice 2: the thread from slice 1
  stops depending on a seeded one.
- The chef closes the gap between "we ran out" and "the waiters know" to a second or two, with no
  admin in the loop.
- Nobody's change is silently undone: the switch and the edit form cannot overwrite each other, and a
  stale edit or reorder is refused rather than merged.
- The kitchen and the bill never see two live dishes with the same name.

**Negative and tradeoffs**

- **A new library joins the web app.** dnd-kit is small and stable, and it is still one more
  dependency to update and one more thing whose keyboard path needs testing on every change.
- **Drag and drop costs more accessibility work than buttons would have.** Announcements, keyboard
  sensors, and touch activation all need translating and testing, and the Playwright run cannot see
  a screen reader.
- **Every edit is live the moment it saves.** There is no draft or preview: an admin reworking the
  menu at 8pm changes the waiters' screens mid service, one save at a time.
- **The diet marks break design.md's colour rule, on purpose.** Green, brown, and amber now mean
  something other than order status, and the three new tokens must pass the contrast gate in every
  appearance.
- **Case insensitive name uniqueness refuses real menus now and then**, such as two "Chicken Tikka"
  dishes in different portions. The admin has to name them apart ("Chicken Tikka (half)").
- **Two role requirements joining the extractor** make the role set less obvious at a glance than
  three single roles. Each is still carried in the handler's own type, so none is invisible.
- **The dietary marker is required**, so entering a menu takes one more choice per dish.

**Neutral**

- Existing development dishes all read as `veg` after the migration until the seed runs again.
- Spec 0003's closed entity vocabulary grows by one, `menu_category`, which must change in the Rust
  enum, `query-keys.ts`, and spec 0003's Notifies list together.
- The price rule depends on the restaurant's currency decimals. Changing currency later (feature 14)
  can leave stored prices with more decimals than the new currency allows; that feature decides what
  happens to them.

## Follow-up

- [ ] The scope row for feature 9 says marking a dish unavailable "removes it from the waiter's
      ordering screen". The engineer confirmed spec 0007's greyed behaviour instead; `/scope` or
      `/sync` should correct that wording.
- [ ] Spec 0007's data model sketch says the ordering menu excludes unavailable dishes, contradicting
      its own AC-3 and its code. A stale line for `/sync` to flag.
- [ ] Spec 0003's Notifies vocabulary gains `menu_category`. A stale line for `/sync` to flag.
- [ ] `docs/design.md` gains the diet mark exception to the colour rule and the `Switch` component.
      `/develop` writes both as part of milestones 2 and 5.
- [ ] `implementing-drag-drop` (`ancoleman/ai-design-components`) was installed during this design and
      is not yet in `web/AGENTS.md` `## Agent skills`. It is web only, so it belongs in the nested file,
      not the root.
- [ ] `accessibility` (`addyosmani/web-quality-skills`) is installed but not listed in any
      `AGENTS.md`. Its reach is every screen, so it belongs in root `AGENTS.md` `## Agent skills`.
- [ ] The dnd-kit Docs MCP server (GitMCP) was declined. Record it on root `AGENTS.md`'s `Declined:`
      line so it is not offered again.
- [ ] Deliberately out of scope, worth their own rows if wanted: dish photos, sizes and add ons, and
      dish names in a second language.
- [ ] Changing the restaurant's currency (feature 14) must decide what happens to prices stored with
      more decimals than the new currency uses.
