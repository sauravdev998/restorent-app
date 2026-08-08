-- Bootstrap: the two pieces of database machinery the whole platform rests on.
--
-- This migration deliberately creates no tables. Feature 4 owns the schema.
-- What it creates is the plumbing every later table will use, so that the
-- conventions exist before the first table does rather than being retrofitted
-- around twenty of them.

-- ---------------------------------------------------------------------------
-- Which restaurant the current transaction is acting for.
--
-- The API sets `app.restaurant_id` with set_config(..., true), which is
-- SET LOCAL: it lives for exactly one transaction, so a pooled connection can
-- never carry one request's restaurant into the next request. Every row level
-- security policy feature 4 writes reads the value through this function.
--
-- Returns NULL when nothing set it. A policy comparing a restaurant_id column
-- to NULL matches no rows, so an unscoped transaction sees nothing rather than
-- seeing everything. That failure direction is the safe one and it is the
-- reason this is a function and not an inline current_setting call people
-- would write slightly differently each time.
-- ---------------------------------------------------------------------------
CREATE FUNCTION public.current_restaurant_id() RETURNS uuid
    LANGUAGE sql
    STABLE
    SET search_path = pg_catalog
    AS $$
        SELECT nullif(current_setting('app.restaurant_id', true), '')::uuid
    $$;

COMMENT ON FUNCTION public.current_restaurant_id() IS
    'The restaurant the current transaction is scoped to, or NULL if unscoped. Read by every row level security policy.';

-- ---------------------------------------------------------------------------
-- Tell every API instance that something changed.
--
-- One global channel, because Postgres delivers a notification to every
-- listener on it and each instance then decides who to forward it to. The
-- restaurant id travels in the payload and is what keeps one restaurant's
-- events off another restaurant's stream.
--
-- The payload carries a kind and an id, never row content. A client that gets
-- one goes back and asks for the row, and row level security decides whether it
-- may have it. Putting the row in here would create a second, unguarded way to
-- read data.
-- ---------------------------------------------------------------------------
CREATE FUNCTION public.notify_entity_change(
    p_restaurant_id uuid,
    p_entity        text,
    p_entity_id     uuid
) RETURNS void
    LANGUAGE plpgsql
    VOLATILE
    SET search_path = pg_catalog
    AS $$
    BEGIN
        PERFORM pg_notify(
            'entity_changed',
            json_build_object(
                'restaurant_id', p_restaurant_id,
                'entity',        p_entity,
                'entity_id',     p_entity_id
            )::text
        );
    END;
    $$;

COMMENT ON FUNCTION public.notify_entity_change(uuid, text, uuid) IS
    'Publishes a change on the entity_changed channel. Call it inside the scoped transaction that made the change.';

-- ---------------------------------------------------------------------------
-- Let the API role call them. Guarded, because the role is created by the
-- container init script locally and by hand on RDS, so it may not exist yet
-- when this runs somewhere new.
-- ---------------------------------------------------------------------------
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'app_api') THEN
        GRANT EXECUTE ON FUNCTION public.current_restaurant_id() TO app_api;
        GRANT EXECUTE ON FUNCTION public.notify_entity_change(uuid, text, uuid) TO app_api;
    ELSE
        RAISE WARNING 'role app_api does not exist; grant execute on the helper functions by hand';
    END IF;
END;
$$;
