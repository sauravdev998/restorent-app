-- ===========================================================================
-- 0008: tables and floor plan, per spec 0010
--
-- The admin builds the real floor from here on, so the two floor tables grow
-- the same three things the menu tables grew in 0006:
--
--   * a version number on every section and table, so an edit made from a
--     form that has gone stale is refused rather than silently written over
--     somebody else's change
--   * limits the database holds itself: a length ceiling on names and labels,
--     and a seat count between 1 and 50
--   * uniqueness among live rows that ignores letter case
--
-- Nothing else changes. No relationship moves, no column is dropped, and no
-- visit, round, line, or bill is touched: a label is read live wherever it is
-- shown, so there is no copy to update.
--
-- The seeded rows (labels 1 to 4 in "Main room", four seats each) already
-- satisfy every rule. A development row that breaks one makes this migration
-- fail and name the constraint, which is the right direction.
-- ===========================================================================

-- ---------------------------------------------------------------------------
-- table_sections
-- ---------------------------------------------------------------------------

-- Bumped by every write that changes the row: rename, archive, restore. Not by
-- a reorder, which changes only `position`, and no form carries that.
ALTER TABLE public.table_sections
    ADD COLUMN version int NOT NULL DEFAULT 1;

-- The same ceiling the API checks as a field error. Both, deliberately: the API
-- check tells an admin which box is wrong, and this one stops a path that forgot
-- to ask from writing a heading nobody can lay out.
ALTER TABLE public.table_sections
    ADD CONSTRAINT table_sections_name_length CHECK (char_length(name) <= 40);

-- Replaces 0002's case sensitive index of the same name. "Terrace" and
-- "terrace" are the same room to anybody walking the floor.
--
-- `lower(name)` alone is enough to also ignore edge spaces, because the API
-- trims every name before it is written.
DROP INDEX public.table_sections_live_name_key;

CREATE UNIQUE INDEX table_sections_live_name_key
    ON public.table_sections (restaurant_id, lower(name))
    WHERE archived_at IS NULL;

-- ---------------------------------------------------------------------------
-- dining_tables
-- ---------------------------------------------------------------------------

ALTER TABLE public.dining_tables
    ADD COLUMN version int NOT NULL DEFAULT 1;

-- A label is printed on a kitchen ticket and a table card, so it stays short.
-- A seat count of 51 or more is a typo, not a table.
ALTER TABLE public.dining_tables
    DROP CONSTRAINT dining_tables_seats_positive,
    ADD CONSTRAINT dining_tables_label_length CHECK (char_length(label) <= 12),
    ADD CONSTRAINT dining_tables_seats_range
        CHECK (seats IS NULL OR seats BETWEEN 1 AND 50);

-- One live table per label across the whole restaurant, ignoring case. A
-- ticket and a bill show only the label, so two live "T1" tables in two rooms
-- would send food to a table nobody can tell apart.
DROP INDEX public.dining_tables_live_label_key;

CREATE UNIQUE INDEX dining_tables_live_label_key
    ON public.dining_tables (restaurant_id, lower(label))
    WHERE archived_at IS NULL;

-- Serves each group read in displayed order, and the "does this section still
-- hold a live table" check an archive makes before it is allowed. A NULL
-- section_id is an ordinary index key here, so the no section group uses it too.
CREATE INDEX dining_tables_group_order_idx
    ON public.dining_tables (restaurant_id, section_id, position)
    WHERE archived_at IS NULL;

COMMENT ON COLUMN public.table_sections.version IS
    'Incremented by every write that changes the row except a reorder. A rename naming an older version is refused as stale.';

COMMENT ON COLUMN public.dining_tables.version IS
    'Incremented by every write that changes the row except a reorder. An edit naming an older version is refused as stale.';
