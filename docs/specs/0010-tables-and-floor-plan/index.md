# 0010. Tables and floor plan

**Date**: 2026-09-16
**Status**: Accepted

## Summary

This decision gives the admin a real floor to run: sections (rooms such as "Terrace") and the
tables in them, added one at a time or as a numbered range, renamed, reordered by dragging, removed,
and brought back, all on one screen shaped like the menu screen. Today tables exist only because the
seed made four. Waiters already pick real tables and already see which are taken (spec 0007); this
feature adds the admin side, shows seats on the waiter's floor, and adds the rules that keep the two
consistent, above all that a table with a party at it cannot be removed. The build is screens and
endpoints over spec 0003's schema, plus one small migration. No new library.

## Requirements

**User stories**:

- As an admin, I want to set up my real tables and rooms so that waiters open bills on the tables we
  actually have, not four seeded placeholders.
- As an admin with thirty tables, I want to add "T1 to T30" in one step so that setup takes a minute,
  not half an hour.
- As an admin, I want to arrange sections and tables in the order my staff walk the room so that the
  waiter's floor reads like the restaurant.
- As an admin, I want to close the terrace for winter and bring it back in spring with its tables so
  that seasonal changes cost nothing.
- As an admin, I want to see which tables are taken while I edit so that I understand why a busy
  table cannot be removed.
- As a waiter, I want to see how many each table seats so that I put a party of six at a six seater.

**Acceptance criteria** (the contract, each independently checkable):

- **AC-1**: The admin floor lives at `/admin/floor`, reached from a `Tables` link in the admin shell's
  navigation. It is one page: the tables with no section come first as their own group (shown only
  when it holds a table), then each live section as a stacked group in order, each table row showing
  its label, its seats when recorded, and an occupied mark. Table add and edit happen in a `Dialog`.
  A restaurant with no live table and no live section shows an empty state that leads to adding the
  first table. First load shows skeletons.
- **AC-2**: An admin can create a section by name. It appears at the end of the section list on the
  admin screen at once. It appears as a heading on the waiter's floor only once it holds a live table.
- **AC-3**: An admin can create a table with a label, optional seats, and an optional section. It
  lands at the end of its group on the admin screen at once, and on every waiter's floor within about
  two seconds, without a refresh.
- **AC-4**: An admin can add a range of tables in one step: an optional prefix, a first and a last
  whole number (each 1 to 999, first not above last, at most 50 tables), optional seats, and an
  optional section, all shared by every table in the range. It creates one table per number, labelled
  prefix then number (prefix `T`, 1 to 3 gives `T1`, `T2`, `T3`; no prefix gives `1`, `2`, `3`),
  appended to the end of the group in number order. When any generated label matches a live table
  (ignoring letter case), nothing is created, and the refusal is `409 labels_taken` listing every
  clashing label, shown to the admin in the form.
- **AC-5**: An admin can rename a section, and can edit a table's label, seats, and section. A table
  moved to another group lands at the end of it. This is allowed while a party sits at the table. A
  changed label shows on every waiter's floor, on the waiter's table screen, and on the kitchen's
  tickets within about two seconds, without a refresh.
- **AC-6**: An admin can reorder the sections, and the tables within one group (including the no
  section group), by drag and drop with a mouse, with touch, and with the keyboard alone. Every step
  of a drag is announced through a screen reader in the interface language. The order the admin sets
  is the order waiters see. Dragging a table into another group is not possible; moving it is done in
  the edit form.
- **AC-7**: A reorder is sent as the complete list of ids in their new order. When that list does not
  match the live set at the moment it arrives, it is refused with `409 floor_changed`, nothing is
  saved, and the admin's screen refetches and shows the server's current order with a translated
  message.
- **AC-8**: Removing a table first asks for confirmation. A table with an open visit cannot be
  removed: the refusal is `409 table_in_use`, with a translated message to close the table first. This
  holds when a waiter opens that table at the same moment: the database never ends up holding an open
  visit on a table this feature removed. A removed table leaves the admin's working list and appears
  in the Archived section at once, disappears from every waiter's floor within about two seconds, and
  every visit, round, ticket, and bill that referenced it still resolves and shows its label.
- **AC-9**: Removing a section first asks for confirmation, and is refused with `409
  section_not_empty` while it holds a live table. This holds when a table is being created in, moved
  into, or restored into that section at the same moment: no live table ever sits in a section this
  feature removed.
- **AC-10**: An admin can restore an archived table from the Archived section. The restore dialog has
  a section picker, set to the table's old section when that is still live, otherwise to No section.
  The table lands at the end of the chosen group and keeps its label and seats. A label clash with a
  live table is refused with `409 name_taken`; a chosen section removed meanwhile is refused with `409
  section_archived`.
- **AC-11**: An admin can restore an archived section. The restore dialog lists every archived table
  whose section is that one, all ticked, and the admin may untick any. Confirming restores the section
  at the end of the section list and every ticked table into it, in their previous order, in one
  transaction. A section name clash is refused with `409 name_taken`; a ticked table whose label
  clashes with a live table is refused with `409 labels_taken` listing every clashing label. Either
  refusal restores nothing.
- **AC-12**: A section name is required and trimmed, at most 40 characters, and unique among live
  sections ignoring letter case. A table label is required and trimmed, at most 12 characters, and
  unique among the restaurant's live tables (across all sections) ignoring letter case. Seats are
  empty or a whole number from 1 to 50. Each refusal is a field error beside its field, translated
  from its code (`required`, `too_long`, `already_taken`, `not_a_number`, `too_small`, `too_large`,
  and for the range `before_start` and `too_many`).
- **AC-13**: A table edit or section rename carrying a version older than the stored one is refused
  with `409 table_changed` or `409 section_changed`, nothing is written, and the admin sees a
  translated message and the current values. An edit that moves a table into a section removed
  meanwhile is refused with `409 section_archived`.
- **AC-14**: The admin screen's occupied mark follows the floor live: when a waiter opens or closes a
  table, the mark appears or clears within about two seconds, without a refresh. The mark is a word
  and an icon, never colour alone.
- **AC-15**: The waiter's floor shows each table's seats when recorded, hides a section with no live
  table, and shows an empty state telling the waiter to ask the admin when the restaurant has no live
  table.
- **AC-16**: Role limits hold on the server. Every `/api/admin/floor` endpoint is admin only; `GET
  /api/floor` stays waiter only. Anyone else receives `403` before the handler body runs, a signed out
  caller receives `401`, an id belonging to another restaurant reads as `404`, and every endpoint
  appears in the OpenAPI document with its role.
- **AC-17**: Creating (each table of a range counts once), editing or renaming, removing, and
  restoring a table or a section each write exactly one audit row carrying the actor, the entity, and
  the before and after values. A reorder writes none.
- **AC-18**: Every floor change notifies inside the transaction that made it. Table changes use
  `dining_table`; section changes use a new `table_section` entity kind. The fan out map refreshes the
  waiter's floor, the admin floor, the waiter's table screen, and the kitchen tickets on a
  `dining_table` event, the two floors on a `table_section` event, and the admin floor on a `visit`
  event, and nothing else they do not feed.
- **AC-19**: No screen touched here carries a user facing string written into a component; English and
  Hindi keys stay at parity; lint, axe, and the contrast script pass.
- **AC-20**: One Playwright run drives an admin context and a waiter context at once: a table the
  admin creates (with a label unique to that run) appears on the waiter's floor, and after the admin
  removes it, it leaves the waiter's floor, both with no reload. The run never opens a table.

## Decision

**Chosen option**: Option 1: an ordered floor list, edited live, over spec 0003's schema.

Build the admin floor screen and its endpoints on the existing `table_sections` and `dining_tables`
tables, in the same shape as the admin menu (spec 0008), adding one migration (a version number per
row, length and seat limits, case insensitive live name uniqueness) and no new library. Every change is
live the moment it commits. Moving a seated party to another table is left to feature 12 (waiter
service flow).

**Implementation skills**: `axum-web-framework` (`manutej/luxor-claude-marketplace`, `.agents/skills/axum-web-framework/`) · `rust-backend` (`windmill-labs/windmill`, `.agents/skills/rust-backend/`) · `rust-best-practices` (`apollographql/skills`, `.agents/skills/rust-best-practices/`) · `sqlx-postgres` (`daiki48/dotfiles`, `.agents/skills/sqlx-postgres/`) · `postgresql-table-design` (`wshobson/agents`, `.agents/skills/postgresql-table-design/`) · `react-router-data-mode` (`remix-run/agent-skills`, `.agents/skills/react-router-data-mode/`) · `tanstack-query` (`tanstack-skills/tanstack-skills`, `.agents/skills/tanstack-query/`) · `shadcn` (`shadcn/ui`, `.agents/skills/shadcn/`) · `react-i18next` (`yildizberkay/skills`, `.agents/skills/react-i18next/`) · `vitest` (`antfu/skills`, `.agents/skills/vitest/`) · `implementing-drag-drop` (`ancoleman/ai-design-components`, `.agents/skills/implementing-drag-drop/`)

**Settled here, not asked** (each with the runner up):

- **Section events get their own kind, `table_section`**, added to `EntityKind`, `ENTITY_KINDS`, and
  spec 0003's Notifies vocabulary together. The existing test that uses `"table_section"` as its
  example of an unknown string switches to another unknown string. Runner up: notify sections as
  `dining_table`, which names the wrong entity in every log line (the same reasoning as
  `menu_category` in spec 0008).
- **A removed table is refused while occupied, and the refusal is race safe by row locks.** Archiving
  locks the table row `FOR UPDATE`, then checks for an open visit. `require_live_table` (used by
  `open_visit` and `move_visit`) changes from a plain read to `SELECT ... FOR SHARE`. The two lock
  modes conflict, so whichever runs second sees the first one's committed result. Runner up: a
  database trigger on `visits`, which hides the rule where nobody reading the repository looks.
- **A section's emptiness is guarded the same way as a menu category's.** Archiving a section locks it
  `FOR UPDATE` before counting live tables; creating, moving, and restoring a table into a section lock
  that section `FOR SHARE` and require it live (`409 section_archived`).
- **The clashing labels travel in a new optional `labels` member on `ErrorBody`**, a list of strings,
  present only on `409 labels_taken` and absent from every other response, the way `fields` was added.
  The body stays one shape. Runner up: the web works out the clashes from its cached floor, which can
  miss a table a colleague added a second ago.
- **A range, and a section restore, are checked before anything is written.** The range add builds
  every label and checks each against AC-12 (a label over 12 characters is `too_long` on the `prefix`
  field); the section restore takes the labels of the ticked archived tables. Both then lock the
  target section `FOR SHARE` once (requiring it live), read the live labels that clash
  (`lower(label) = any($1)`), and refuse with `409 labels_taken` if any do. Only then do the writes run,
  in the same transaction.
- **A late clash is answered by rolling back, then reading afresh.** A table added by a colleague
  between the check and the write makes the insert (or the restoring update) fail on the live label
  index. Postgres refuses every further statement in a transaction after a failed one, so the handler
  rolls the transaction back, opens a new scoped read transaction, reads the clashing labels there,
  and answers `409 labels_taken` with them. If that read finds none (the other table was removed
  meanwhile), the answer is `409 labels_taken` with an empty `labels`, which the form shows as "a
  label was just taken, try again". No savepoint is used, since nothing else in the codebase does.
  Runner up: a savepoint before the writes, which keeps one transaction but adds a pattern nobody here
  has used.
- **`labels` is ordered as the request would have created them**: number order for a range, the
  tables' previous order for a section restore. Each label is spelled as generated or as stored, not
  as the clashing live table spells it.
- **A section restore with tables writes one audit row and one notify for the section, plus one of
  each per restored table**, matching the range rule in AC-17.
- **The prefix keeps a trailing space**, so `Bar ` then 1 gives `Bar 1`. Leading spaces are removed.
  An empty or absent prefix means bare numbers. Numbers are written plainly, with no leading zeros.
- **A new table goes to the end of its group** (`max(position) + 1` among live tables with the same
  `section_id`, compared with `IS NOT DISTINCT FROM` so the no section group works, `1` when empty).
  Two concurrent creates in the no section group may share a position; order falls back to label, then
  id, and nothing needs positions to be unique. Runner up: lock a restaurant row to serialise creates,
  which buys a tidier number nobody sees.
- **A reorder rewrites positions `1..n` for one group**, after locking its live rows `FOR UPDATE ...
  ORDER BY id` and checking the sent list has the same length and ids as the live set. One endpoint
  serves every group, `PUT /api/admin/floor/table-order` with `sectionId` (null for the no section
  group). Runner up: a per section path like the menu's `dish-order`, which has no path for the no
  section group.
- **Stale detection is a conditional update, `WHERE id = $1 AND version = $2`**, as in spec 0008.
  Every write that changes a row bumps its `version`; a reorder does not.
- **The admin floor query key is `['visit', 'floor', 'admin']`**, under the `visit` prefix, so every
  `visit` event (a table opened or closed) refreshes the occupied mark with no new fan out row. The
  `dining_table` row widens to `[['visit'], kitchenKey]`, so a renamed table reaches the table screen
  and the kitchen tickets, and a new `table_section` row is `[['visit']]`.
- **A confirmed admin write refreshes the admin's own screen directly**, invalidating `['visit',
  'floor']` in its `onSuccess`, as spec 0008 does for the menu. This is a refetch after a confirmed
  write, not cache optimism.
- **A dropped reorder is held in component state while pending**, reusing `web/src/admin/menu/reorderable.tsx`
  (moved to `web/src/admin/shared/` if a second caller makes that cleaner). Refused: drop the held
  order and refetch.
- **The admin floor reads everything in one snapshot** (`begin_scoped_snapshot`), as the waiter's
  floor does, so a table opened between two statements cannot appear in one and not the other.
- **A live table left in an archived section** (possible only for rows made before this feature)
  shows in the no section group on both screens, as the floor read already does. Its edit form shows
  No section, and saving moves it there. The migration does not touch such rows.
- **Seats travel as a JSON integer.** The web form refuses a non whole number itself with the
  existing `not_a_number` message before sending, so the API never produces that code for seats and it
  is not added to any server list for this feature. The API checks the range.
- **No pagination on the floor reads.** A floor is tens of rows, read whole as one room.
- **The admin nav label is `Tables`** and the path is `/admin/floor`, beside `Menu` and `Staff`.

## Rationale

Reasoning and options: see [rationale.md](rationale.md).

## Feature design

**Data model sketch**

One migration, `api/migrations/0008_tables_and_floor_plan.sql`. Everything not listed is spec 0003's
schema, used as it stands.

| Table | Column or constraint | Type and rule | Change |
|---|---|---|---|
| `table_sections` | `version` | `int not null default 1` | new |
| `table_sections` | `table_sections_name_length` | check `char_length(name) <= 40` | new |
| `table_sections` | `table_sections_live_name_key` | unique `(restaurant_id, lower(name))` where `archived_at is null` | replaces the case sensitive index of the same name |
| `dining_tables` | `version` | `int not null default 1` | new |
| `dining_tables` | `dining_tables_label_length` | check `char_length(label) <= 12` | new |
| `dining_tables` | `dining_tables_seats_range` | check `seats is null or seats between 1 and 50` | replaces `dining_tables_seats_positive` |
| `dining_tables` | `dining_tables_live_label_key` | unique `(restaurant_id, lower(label))` where `archived_at is null` | replaces the case sensitive index of the same name |
| `dining_tables` | `dining_tables_group_order_idx` | `(restaurant_id, section_id, position)` where `archived_at is null` | new. Serves the ordered group and the "is this section empty" count |

Names and labels are trimmed by the API before they are written (`require_name` already does), so
`lower(...)` alone is enough to ignore edge spaces. The seeded rows (labels `1` to `4`, section `Main
room`) already satisfy every new rule. If a developer's database holds a row that breaks one, the
migration fails loudly and names the constraint, which is the right direction.

Relationships are unchanged: restaurant 1:N section, restaurant 1:N table, section 0..1:N table
(composite foreign key on `(section_id, restaurant_id)`, nullable), table 1:N visit with at most one
open (`visits_one_open_per_table`). An archived table keeps its `section_id`; that is how a section
restore finds its tables.

Rust side: `version: i32` on `TableSection` and `DiningTable`; new `AuditAction` variants
`DiningTableCreated`, `DiningTableEdited`, `DiningTableArchived`, `DiningTableRestored`,
`TableSectionCreated`, `TableSectionRenamed`, `TableSectionArchived`, `TableSectionRestored`; new
`ConflictKind` variants `TableInUse` (`table_in_use`), `TableChanged`, `SectionChanged`,
`SectionNotEmpty`, `SectionArchived`, `FloorChanged`, `LabelsTaken` (carrying the list); new
`EntityKind::TableSection`. The existing `archive_dining_table` and `archive_table_section` gain the
checks, audit rows, and notify (sections do not notify today). `create_table_section` notifies too.
The seed keeps working through the same functions.

**State transitions**

- **Section**: `live` → `archived` (remove, refused while it holds a live table) → `live` (restore,
  refused on a live name clash, optionally with its archived tables). Rename and reorder happen while
  live.
- **Table**: `live` → `archived` (remove, refused while it has an open visit) → `live` (restore into a
  chosen live section or none, refused on a live label clash). Edit, move, and reorder happen while
  live, occupied or not.
- **Version**: rename, edit, move, archive, and restore each increment `version` by one. A reorder
  does not.
- Occupancy is not a table state. It is the existence of an open visit, as spec 0003 designed it.

**API surface**

Every endpoint sits under the existing router, the origin check, and the 30 second timeout, carries
its role in the handler's own `Actor<R>` type, and is listed in `presentation/openapi.rs`. Admin
handlers live in a new `handlers/admin_floor.rs`. Writes run in `Database::begin_scoped`; the admin
read runs in `begin_scoped_snapshot`.

| Endpoint | Method | Key inputs | Key outputs | Auth | Key errors |
|---|---|---|---|---|---|
| `/api/admin/floor` | GET | none | `groups` in display order: first the no section group (`id: null`, `name: null`, `version: null`), then each live section (`id`, `name`, `version`); each with `tables` (`id`, `label`, `seats`, `version`, `occupied`). `archived`: `sections` (`id`, `name`, `archivedAt`, `tables`: the archived tables whose section it is, `id`, `label`, `seats`, in previous order) and `tables` (`id`, `label`, `seats`, `sectionId`, `sectionName`, `sectionLive`, `archivedAt`) | admin | `401`, `403` |
| `/api/admin/floor/sections` | POST | `name`: string (req) | the section | admin | `400` field errors |
| `/api/admin/floor/sections/{id}` | PUT | `name`: string (req), `version`: int (req) | the section | admin | `400` field errors, `404`, `409 section_changed` |
| `/api/admin/floor/sections/order` | PUT | `ids`: uuid[] (req), every live section exactly once | the reordered list | admin | `409 floor_changed` |
| `/api/admin/floor/sections/{id}/archive` | POST | path id | the archived section | admin | `404`, `409 section_not_empty` |
| `/api/admin/floor/sections/{id}/restore` | POST | `tableIds`: uuid[] (req, may be empty), each an archived table of this section | the section and its restored tables | admin | `400` (an id that is not an archived table of this section), `404`, `409 name_taken`, `409 labels_taken` with `labels` |
| `/api/admin/floor/tables` | POST | `sectionId`: uuid (opt), `label`: string (req), `seats`: int (opt) | the table | admin | `400` field errors, `409 section_archived` |
| `/api/admin/floor/tables/range` | POST | `sectionId` (opt), `prefix`: string (opt), `from`: int (req), `to`: int (req), `seats` (opt) | the created tables, in order | admin | `400` field errors, `409 section_archived`, `409 labels_taken` with `labels` |
| `/api/admin/floor/table-order` | PUT | `sectionId`: uuid or null (req), `ids`: uuid[] (req), every live table of that group exactly once | the reordered group | admin | `404` (unknown section), `409 floor_changed` |
| `/api/admin/floor/tables/{id}` | PUT | `sectionId` (nullable), `label`, `seats` (nullable), `version` (all req) | the table | admin | `400` field errors, `404`, `409 table_changed`, `409 section_archived` |
| `/api/admin/floor/tables/{id}/archive` | POST | path id | the archived table | admin | `404`, `409 table_in_use` |
| `/api/admin/floor/tables/{id}/restore` | POST | `sectionId`: uuid or null (req) | the table | admin | `404`, `409 name_taken`, `409 section_archived` |
| `/api/floor` | GET | none | unchanged shape; a section with no live table is left out | waiter | `401`, `403` |

Field error codes, on the existing `fields` member: `name`: `required`, `too_long`, `already_taken`;
`label`: `required`, `too_long`, `already_taken`; `seats`: `too_small`, `too_large`; `prefix`:
`too_long` (some generated label passes 12 characters); `from`: `too_small`, `too_large`; `to`:
`too_small`, `too_large`, `before_start`, `too_many`. `too_small`, `before_start`, and `too_many` are
new codes, added to the closed set in `ErrorBody`'s doc and to `web/src/shared/api/field-errors.ts`.
A unique violation on either live name index from a create, rename, or edit is caught and turned into
`already_taken` on its field, as spec 0008 does.

**Value sourcing**

| Action | Value produced or displayed | Source |
|---|---|---|
| every request | which restaurant | the session, resolved by `Actor`, passed to `begin_scoped`. No body or path names a restaurant |
| every write | the audit actor | `Actor`'s staff id |
| create section or table | its id | uuid v7 generated in Rust |
| create, move, restore a table | its position | `max(position) + 1` among live tables with the same `section_id` (`IS NOT DISTINCT FROM`), `1` when none |
| create, restore a section | its position | `max(position) + 1` among live sections, `1` when none |
| range add | each label | `prefix` (leading spaces removed) followed by the decimal number, for each number from `from` to `to` |
| range add | each position | the group's `max(position) + 1`, then one more per table, in number order |
| range add, section restore | which labels clash | live `dining_tables` where `lower(label)` is any generated or restored label lowercased, read in the same transaction after the section `FOR SHARE` lock; on a late unique violation, the same read in a fresh scoped transaction after rollback |
| range add, section restore | the order of `labels` | the request's own order: number order for a range, the tables' previous order for a restore |
| range add | whether the target section is live | `table_sections.archived_at is null`, locked `FOR SHARE` once before any insert |
| section restore | audit rows and notifies | one for the section, plus one per restored table |
| edit, rename | whether the caller's copy is current | the `version` the form loaded, compared in the conditional update |
| create, edit, restore table into a section | whether the section is live | `table_sections.archived_at is null`, read `FOR SHARE` |
| archive table | whether it is occupied | a `visits` row with `status = 'open'` for it, read after locking the table `FOR UPDATE` |
| archive section | whether it is empty | count of live tables with that `section_id`, after locking the section `FOR UPDATE` |
| reorder | what the complete list must be | the live ids of that group, read `FOR UPDATE ... ORDER BY id` |
| reorder | the new positions | each id's index in the sent list, `1..n` |
| restore table dialog | the default section | `sectionId` when `sectionLive` is true, otherwise null (No section) |
| restore section dialog | which tables to offer | `archived.sections[].tables`: archived tables whose `section_id` is that section |
| restore section | the restored tables' order | their stored `position`, then label, renumbered after the section's current last table |
| admin read | `occupied` | whether an open `visits` row exists for the table, in the same snapshot |
| admin read | `sectionLive`, `sectionName` | the archived table's `section_id` joined to `table_sections` |
| admin and waiter floors | group and table order | section `position`, then name; table `position`, then label, then id |
| waiter floor | seats | `dining_tables.seats`, already on `FloorTableDto` |
| waiter table screen, kitchen tickets | a table's label | `dining_tables.label`, read live at each fetch (as today), refreshed by the `dining_table` fan out |
| admin screen | a change appearing at once after the admin's own save | the write's `onSuccess` invalidating `['visit', 'floor']` |
| admin screen | the occupied mark staying current | `visit` events invalidating `['visit']`, which covers `['visit', 'floor', 'admin']` |
| reorder refused | the order shown afterwards | a refetch of the admin floor, never the held order |
| drag announcements | every word a screen reader hears | dnd-kit's `announcements` and `screenReaderInstructions`, filled from `t()` in the `admin` namespace |
| range refusal | the clashing labels shown | the `labels` member of the `409 labels_taken` body |
| a refused request | the sentence shown | the `error` code, or each field code, mapped to a key by `shared/api/error-message.ts` and `field-errors.ts` |
| waiter empty state, no section heading | their words | new keys in the `waiter` and `admin` namespaces, English and Hindi |

Fan out map in `web/src/shared/events/query-keys.ts`. A new key `adminFloorKey = ['visit', 'floor',
'admin']` sits beside `floorKey = ['visit', 'floor']`.

| Event entity | Invalidates |
|---|---|
| `visit` | `['visit']` (unchanged; now also covers the admin floor) |
| `dining_table` | `['visit']`, `kitchenKey` (was `floorKey` only) |
| `table_section` | `['visit']` (new) |
| `staff` | `floorKey` (unchanged) |

**Key invariants**

1. No open visit is ever created on, or left on, a table this feature archived. Archive takes `FOR
   UPDATE` on the table before checking; `open_visit` and `move_visit` take `FOR SHARE` through
   `require_live_table`.
2. No live table sits in a section this feature archived. Archive takes `FOR UPDATE` on the section
   before counting; every write that puts a table into a section takes `FOR SHARE` on it and requires
   it live.
3. At most one live table per restaurant per lowercased trimmed label, and one live section per
   lowercased trimmed name. Enforced by the partial unique indexes, not only by a read.
4. A range add or a section restore either writes every row it names or none.
5. A reorder either rewrites every position in its group or none.
6. Every write that changes a row increments its `version` in the same statement.
7. No floor write touches `visits`, `order_rounds`, `order_lines`, or `bills`. Labels are read live
   wherever they are shown; bills keep no copy.
8. Every floor write notifies (`dining_table` or `table_section`) inside its own transaction, and
   every one AC-17 names writes its audit row in that same transaction.

**Security model**

- **Admin only**: every `/api/admin/floor` endpoint, through `Actor<Admin>`.
- **Waiter only**: `GET /api/floor`, unchanged. The admin reads `/api/admin/floor` instead. Chefs see
  neither.
- **Tenant separation is unchanged.** Every statement runs in a `ScopedTx`; an id from another
  restaurant is invisible to row level security and reads as `404`, and the composite foreign key
  refuses a section from another restaurant even if one were named.
- **Browser gates are convenience.** The admin route group hides the screen from other roles; the
  server refuses regardless.
- **Audit log**: required (AC-17), for consistency with every other admin change and to answer "who
  removed table 7".
- **Compliance scope**: none. Table and section data is the restaurant's own layout, with no personal
  and no payment data.

**Configuration required**

None. No environment variable, no secret, no new service, no new dependency.

**Critical test scenarios**

- Happy path, two browsers: the admin creates a uniquely labelled table and the waiter's floor shows
  it with no reload; the admin removes it and it leaves the waiter's floor with no reload. Verifies
  **AC-3**, **AC-8**, **AC-18**, **AC-20**.
- Happy path, API: create two sections, a single table, and a range `T1` to `T5`; edit seats, rename,
  move, reorder both levels; archive and restore; asserting rows, positions, versions, and audit rows
  at each step. Verifies **AC-2**, **AC-3**, **AC-4**, **AC-5**, **AC-6**, **AC-10**, **AC-17**.
- Failure case, the occupancy race: in two real concurrent transactions, archive a table while
  `open_visit` runs on it, in both orders; exactly one wins, and no open visit ever sits on an archived
  table. Verifies **AC-8**.
- Failure case, the section race: archive a section while a concurrent transaction creates, moves, or
  restores a table into it; exactly one wins. Verifies **AC-9**.
- Failure case, range clash: with `T3` live, add `t1` to `t5`; refused `409 labels_taken` with
  `labels: ["t3"]` (the generated spelling), and no table created. Add 51 tables, `from` 5 `to` 2,
  `from` 0, `to` 1000, and prefix `Rooftop bar ` with `to` 10; each refused with its own field code.
  Verifies **AC-4**, **AC-12**.
- Failure case, busy table: a waiter opens a table; the admin's remove is refused `409 table_in_use`
  and the table stays live. The admin then renames that table, and the waiter's table screen and the
  kitchen ticket show the new label. Verifies **AC-5**, **AC-8**.
- Failure case, stale edit: two admin tabs edit the same table; the second save is refused `409
  table_changed` with the current values. Verifies **AC-13**.
- Failure case, stale reorder: send a group order missing a table created a moment earlier, and one
  with an id repeated; both refused `409 floor_changed`. Verifies **AC-7**.
- Failure case, section restore: archive `Terrace` with `P1` and `P2`, create a new live `p2`, restore
  `Terrace` with both ticked; refused `409 labels_taken` with `["P2"]`, nothing restored. Untick `P2`
  and restore; the section and `P1` return. Verifies **AC-11**.
- Failure case, validation: a blank label, a 13 character label, `t1` beside a live `T1`, seats `0`
  and `51`, a 41 character section name. Verifies **AC-12**.
- Keyboard drag: reorder a table with Space, the arrow keys, and Space again, with the announcements
  asserted in English and Hindi. Verifies **AC-6**.
- Waiter floor: a section with only archived tables shows no heading; a restaurant with no live table
  shows the empty state; seats show on each card. Verifies **AC-15**.
- Occupied mark: opening and closing a table flips the admin's mark with no reload. Verifies **AC-14**.
- Auth and permission: a waiter or a chef calling any `/api/admin/floor` endpoint receives `403`, an
  admin calling `GET /api/floor` receives `403`, a signed out caller receives `401`, and restaurant A
  editing restaurant B's table id receives `404`. Verifies **AC-16**.

## Build plan

Tracer Bullet: the first milestone pushes the scope's core promise (an admin adds a real table, every
waiter sees it live, and a busy table cannot be removed) through every layer on the plainest screen,
then the rest thickens it. The migration is small and lands whole in the first milestone.

**Milestone 1: the thread, top to bottom**

1. Write migration `0008_tables_and_floor_plan.sql` per the data model sketch, run it, and add the
   `version` fields to the domain types. Satisfies **AC-12**, **AC-13**.
2. Add `EntityKind::TableSection` (`table_section`), the entry in `ENTITY_KINDS`, `adminFloorKey`, and
   the fan out rows (`dining_table` widened, `table_section` new); update the unknown string test.
   Satisfies **AC-18**.
3. Change `require_live_table` to `FOR SHARE`; give `archive_dining_table` the `FOR UPDATE` lock, the
   open visit check (`TableInUse`), the version bump, and the audit row. Adjust any existing test that
   archives an occupied table. Satisfies **AC-8**, **AC-17**.
4. Build `GET /api/admin/floor`, `POST /api/admin/floor/tables`, and `POST
   /api/admin/floor/tables/{id}/archive`, with `DiningTableCreated` and `DiningTableArchived` audit
   rows and notify. Satisfies **AC-3**, **AC-8**, **AC-14**, **AC-16**, **AC-17**.
5. Build the plainest `/admin/floor` screen (groups, table rows with seats and the occupied mark, an
   add table `Dialog`, remove with the shared confirm dialog) and the `Tables` link in the admin shell.
   Every admin write invalidates `['visit', 'floor']` on success. Satisfies **AC-1**, **AC-3**,
   **AC-8**, **AC-14**.
6. On the waiter's floor (`web/src/waiter/routes/floor.tsx`), render seats on each card (already on
   the wire as `FloorTableDto.seats`) and add the empty state. Empty sections are already left out by
   the `/api/floor` handler; keep that and cover it with a test. Satisfies **AC-15**.
7. Regenerate the `.sqlx` cache and the typed client, and add `web/e2e/floor.spec.ts` with an admin
   and a waiter context (unique label per run, create then remove, never open). Satisfies **AC-20**.

**Milestone 2: sections and the edit surface**

8. Build section create, rename (version check), and archive (`FOR UPDATE`, `SectionNotEmpty`), with
   their audit rows and `table_section` notify; make `create_table_section` notify. Satisfies
   **AC-2**, **AC-9**, **AC-13**, **AC-17**, **AC-18**.
9. Build table edit with move (target section `FOR SHARE` first, then the version check), and the
   section check on table create. Satisfies **AC-5**, **AC-9**, **AC-13**.
10. Add the field validation and codes of AC-12 (new `too_small`, `before_start`, `too_many` in
    `ErrorBody` docs and `field-errors.ts`), the unique violation caught as `already_taken`, and the
    translated conflict messages. Satisfies **AC-12**, **AC-13**.
11. Add section add, rename, and remove to the admin screen, and the edit table dialog with a section
    `Select` and the seats input. Satisfies **AC-1**, **AC-2**, **AC-5**.

**Milestone 3: order**

12. Build `PUT /api/admin/floor/sections/order` and `PUT /api/admin/floor/table-order`, with the group
    lock and `FloorChanged`. Satisfies **AC-6**, **AC-7**.
13. Wire drag handles on the admin screen through the menu's `reorderable.tsx`, with translated
    announcements, touch, and keyboard; refusal refetches. Satisfies **AC-6**, **AC-7**.

**Milestone 4: the range**

14. Add the optional `labels` member to `ErrorBody` and `LabelsTaken` to `ConflictKind`. Satisfies
    **AC-4**.
15. Build `POST /api/admin/floor/tables/range` (label building, the checks, the section lock, the
    clash read, inserts, one audit row per table, and the late clash path: roll back, read afresh,
    answer). Satisfies **AC-4**, **AC-12**, **AC-17**.
16. Add an `Add several` tab or mode to the add table dialog, with a live preview of the first and last
    label and the clash list on refusal. Satisfies **AC-4**.

**Milestone 5: archive and restore**

17. Build table restore (section `FOR SHARE`, `NameTaken`, `SectionArchived`) and section restore with
    `tableIds` (one transaction, `NameTaken`, the same clash check and late clash path as the range,
    `LabelsTaken`), with one audit row and notify for the section plus one per restored table.
    Satisfies **AC-10**, **AC-11**, **AC-17**.
18. Add the Archived section to the admin screen, the restore table dialog with its defaulted section
    picker, and the restore section dialog with its ticked table list. Satisfies **AC-10**, **AC-11**.

**Milestone 6: finish**

19. Skeletons, the admin empty state, Hindi keys at parity, axe and contrast passes, and the integration
    tests for both races. Satisfies **AC-1**, **AC-8**, **AC-9**, **AC-19**.

## Consequences

**Positive**:

- A new restaurant can be set up with no seed and no developer: the admin builds the floor in a
  minute, and the waiter's floor is correct from the first service.
- A busy table can never vanish from under a seated party, as a database guarantee through row locks
  rather than a check someone might skip.
- The admin floor reuses the menu's shape (dialogs, archived list, drag, stale versions), so there is
  one pattern to learn and to test.
- Occupancy on the admin screen comes free from the existing `visit` events.

**Negative / tradeoffs**:

- **Removing a section takes one removal per table first.** Closing a twelve table terrace is thirteen
  confirmations, while restoring it is one. That asymmetry was chosen to avoid surprise side effects;
  a "remove with its tables" option can come later if admins complain.
- **Restaurant wide unique labels** mean two rooms cannot both have a table `1`. Accepted, because
  tickets and bills show only the label.
- **`open_visit` now takes a row lock on the table.** It is held for a few milliseconds, and it only
  conflicts with an admin removing that same table, so service never notices. It is still one more lock
  to remember when reasoning about spec 0003's lock order: it is taken before the visit insert, and no
  path takes it after the bill or counter locks.
- **A renamed table changes the label on live tickets and on any bill still open.** A kitchen mid
  service sees a ticket change its table name. That is intended (a typo fix), but a closed bill has no
  copy of the label either, so a reprinted old bill shows the new name. Feature 14 or 16 should decide
  whether a closed bill snapshots the label.
- **The range add is capped at 50** and uses plain numbers, so `T01` style labels or a 60 table hall
  take two steps.
- **The error body gains a second optional member** (`labels`), used by one code. Every client that
  reads the body must ignore it when absent, which the generated types already make explicit.

**Neutral**:

- One migration, no new library, no new environment variable.
- The Playwright run leaves one archived table per run in the development database. Archived rows
  appear only in the admin's Archived list, so nothing else is affected, but that list grows.
- The screen reader check with a real screen reader is skipped for this feature, by the engineer's
  choice, relying on the announcement tests and axe (as for staff accounts).

## Follow-up

- [ ] Feature 12 (waiter service flow) owns moving a seated party to another table: the `move_visit`
      repository function exists and takes the new `FOR SHARE` lock, but has no endpoint or button.
- [ ] Feature 14 or 16 should decide whether a closed bill keeps a copy of the table label, so a
      reprint shows the table the party actually sat at.
- [ ] A drawn floor map (grid or free placement) was deferred; if it is ever wanted, it adds layout
      columns on top of this ordered list rather than replacing it.
- [ ] Spec 0003's Notifies vocabulary and `table_section` must be updated together when this is built
      (the vocabulary lives in that spec's operation table).
