-- ===========================================================================
-- 0006: menu management, per spec 0008
--
-- The admin builds the real menu from here on, so the two menu tables grow the
-- three things that job needs and spec 0003 did not have to decide:
--
--   * a diet marker on every dish (veg, non veg, or egg), the familiar square
--     mark a customer reads before anything else on an Indian menu
--   * a version number on every category and dish, so an edit made from a
--     form that has gone stale is refused rather than silently written over
--     somebody else's change
--   * name rules the database holds itself: a length ceiling, and uniqueness
--     among live rows that ignores letter case
--
-- Nothing else about the two tables changes. No relationship moves, no column
-- is dropped, and no row belonging to an order line, a round, or a bill is
-- touched, because a line copied the dish's name and price when it was sent and
-- never reads the menu again.
-- ===========================================================================

-- The diet marker. A closed list, for the same reason every other enum here is
-- one: a screen draws a shape per value, and a value nobody can draw is a bug.
CREATE TYPE public.dish_diet AS ENUM ('veg', 'non_veg', 'egg');

-- ---------------------------------------------------------------------------
-- menu_categories
-- ---------------------------------------------------------------------------

-- Bumped by every write that changes the row: rename, archive, restore. Not by
-- a reorder, which changes only `position`, and no edit form carries that.
ALTER TABLE public.menu_categories
    ADD COLUMN version int NOT NULL DEFAULT 1;

-- The same ceiling the API checks as a field error. Both, deliberately: the API
-- check is what tells an admin which box is wrong, and this one is what stops a
-- path that forgot to ask from writing a heading nobody can lay out.
ALTER TABLE public.menu_categories
    ADD CONSTRAINT menu_categories_name_length CHECK (char_length(name) <= 60);

-- Replaces 0002's case sensitive index of the same name. "Starters" and
-- "starters" are the same heading to anybody reading a menu, so two of them
-- live at once would be two sections a waiter cannot tell apart.
--
-- `lower(name)` alone is enough to also ignore edge spaces, because the API
-- trims every name before it is written.
DROP INDEX public.menu_categories_live_name_key;

CREATE UNIQUE INDEX menu_categories_live_name_key
    ON public.menu_categories (restaurant_id, lower(name))
    WHERE archived_at IS NULL;

-- ---------------------------------------------------------------------------
-- dishes
-- ---------------------------------------------------------------------------

-- Added with a default so the rows already in a development database fill, and
-- the default is dropped straight after, so from here on every insert has to
-- name one. A dish nobody chose a marker for would be drawn as veg, and saying
-- a chicken dish is vegetarian is the one mistake on a menu that is not merely
-- embarrassing.
ALTER TABLE public.dishes
    ADD COLUMN diet public.dish_diet NOT NULL DEFAULT 'veg',
    ADD COLUMN version int NOT NULL DEFAULT 1;

ALTER TABLE public.dishes
    ALTER COLUMN diet DROP DEFAULT;

ALTER TABLE public.dishes
    ADD CONSTRAINT dishes_name_length CHECK (char_length(name) <= 80),
    ADD CONSTRAINT dishes_description_length
        CHECK (description IS NULL OR char_length(description) <= 300);

-- One live dish per name, ignoring case. The kitchen and the bill read a line
-- by its copied name, so two live "Paneer Tikka" dishes at different prices
-- would be two lines on a bill nobody could tell apart.
CREATE UNIQUE INDEX dishes_live_name_key
    ON public.dishes (restaurant_id, lower(name))
    WHERE archived_at IS NULL;

-- Serves the menu read in printed order, and the "does this category still hold
-- a live dish" check an archive makes before it is allowed.
CREATE INDEX dishes_category_order_idx
    ON public.dishes (restaurant_id, category_id, position)
    WHERE archived_at IS NULL;

COMMENT ON COLUMN public.dishes.diet IS
    'The veg, non veg, or egg mark. Required on every insert; the default used to fill existing rows was dropped in the same migration.';

COMMENT ON COLUMN public.dishes.version IS
    'Incremented by every write that changes the row except a reorder. An edit naming an older version is refused as stale.';

COMMENT ON COLUMN public.menu_categories.version IS
    'Incremented by every write that changes the row except a reorder. A rename naming an older version is refused as stale.';
