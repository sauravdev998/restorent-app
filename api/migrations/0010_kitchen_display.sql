-- ===========================================================================
-- 0010: kitchen display, per spec 0012
--
-- No new table. Three columns on `restaurants` and two index changes:
--
--   * the two ageing thresholds a restaurant sets for itself, amber then red.
--     They default to the values the pass already behaves as though it has, so
--     nothing seeded changes meaning the moment this lands
--   * a version number on the restaurant, so an edit made from a settings form
--     that has gone stale is refused rather than merged over somebody else's
--     change. The row has been editable since spec 0003 with last write wins;
--     this is the column that ends it
--   * the queue index widened to cover the ready half of a read that has
--     selected both statuses since spec 0007, and a second index so the Ready
--     area's own ordering is an index read rather than a sort of everything
--
-- Every column is not null with a default, so every existing row is valid the
-- moment this runs and there is no backfill. The existing grants in 0002 cover
-- added columns, and no policy changes.
-- ===========================================================================

-- ---------------------------------------------------------------------------
-- restaurants: the two thresholds, and optimistic concurrency at last
-- ---------------------------------------------------------------------------

ALTER TABLE public.restaurants
    ADD COLUMN kitchen_warning_after_seconds int NOT NULL DEFAULT 600,
    ADD COLUMN kitchen_late_after_seconds    int NOT NULL DEFAULT 900,
    ADD COLUMN version                       int NOT NULL DEFAULT 1;

-- The first two bounds are the same rule the handler checks in Rust, kept here
-- so a future writer cannot store a threshold of nine seconds by forgetting a
-- line. The third is the one that matters: amber after red is a state the pass
-- cannot draw sensibly, and the handler checks the merged pair first so a
-- refusal can name the field that was wrong. This is the backstop, not the
-- expected error path.
ALTER TABLE public.restaurants
    ADD CONSTRAINT restaurants_kitchen_warning_in_range
        CHECK (kitchen_warning_after_seconds BETWEEN 60 AND 14400),
    ADD CONSTRAINT restaurants_kitchen_late_in_range
        CHECK (kitchen_late_after_seconds BETWEEN 60 AND 14400),
    ADD CONSTRAINT restaurants_kitchen_warning_before_late
        CHECK (kitchen_warning_after_seconds < kitchen_late_after_seconds);

-- ---------------------------------------------------------------------------
-- order_rounds: index the whole of what the kitchen read asks for
-- ---------------------------------------------------------------------------

-- The kitchen read has filtered on both statuses since spec 0007 while this
-- index covered only one of them, so the ready half of every read was a scan.
DROP INDEX public.order_rounds_queue_idx;

CREATE INDEX order_rounds_queue_idx
    ON public.order_rounds (restaurant_id, sent_at)
    WHERE status IN ('queued', 'ready');

-- The Ready area orders by when the food came off the pass, not by when the
-- ticket was sent, so the widened index above cannot serve it. The ready set is
-- small in practice; this makes the capped read's ORDER BY ... LIMIT an index
-- read rather than a sort of everything open.
CREATE INDEX order_rounds_ready_idx
    ON public.order_rounds (restaurant_id, ready_at)
    WHERE status = 'ready';

COMMENT ON COLUMN public.restaurants.kitchen_warning_after_seconds IS
    'Seconds a ticket may wait before the pass draws it amber. Always below kitchen_late_after_seconds.';

COMMENT ON COLUMN public.restaurants.kitchen_late_after_seconds IS
    'Seconds a ticket may wait before the pass draws it red. The value the pass hardcoded before this column existed.';

COMMENT ON COLUMN public.restaurants.version IS
    'Incremented by every write that changes the row. An edit naming an older version is refused as stale.';
