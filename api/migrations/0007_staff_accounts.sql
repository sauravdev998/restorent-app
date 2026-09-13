-- ===========================================================================
-- 0007: staff accounts, per spec 0009
--
-- An admin can now put real people on the floor: create a waiter or a chef,
-- rename them, change their role, reset their password, and switch the account
-- off when they leave. Everything that job needs and spec 0003 did not have to
-- decide lands here, in one migration:
--
--   * a marker saying the password on this row was written by somebody else,
--     so it is spent the first time its owner signs in
--   * a version number, so an edit made from a form that has gone stale is
--     refused rather than written over somebody else's change
--   * the two length rules the API also checks as field errors
--   * an index on the restaurant, because the staff list is the first query
--     that reads this table by restaurant
--   * resolve_session returning the marker, so the password gate costs no
--     second read on any request in the product
--
-- No table is created, no relationship moves, and no column is dropped. Every
-- row that already exists keeps its password and its role exactly as it is.
-- ===========================================================================

-- ---------------------------------------------------------------------------
-- staff
-- ---------------------------------------------------------------------------

-- True whenever an admin wrote this person's password, at creation or at a
-- reset, and false once that person has written their own. The default is
-- false, so every account that already exists is left alone: those people
-- chose their own password at registration, and there is nothing owed.
ALTER TABLE public.staff
    ADD COLUMN must_change_password boolean NOT NULL DEFAULT false;

COMMENT ON COLUMN public.staff.must_change_password IS
    'Set by every path that writes somebody else''s password, cleared only by that person writing their own. While it is true the account may reach GET /api/me and POST /api/me/password and nothing else.';

-- Bumped by every write that changes the row. The same shape 0006 gave
-- menu_categories and dishes, which makes the conditional update a pattern
-- rather than one feature's quirk.
ALTER TABLE public.staff
    ADD COLUMN version int NOT NULL DEFAULT 1;

-- The same ceilings the API checks as field errors. Both, deliberately: the API
-- check is what tells an admin which box is wrong, and this one is what stops a
-- path that forgot to ask from writing a name nobody can lay out.
ALTER TABLE public.staff
    ADD CONSTRAINT staff_display_name_length
        CHECK (char_length(btrim(display_name)) BETWEEN 1 AND 80);

ALTER TABLE public.staff
    ADD CONSTRAINT staff_email_length CHECK (char_length(email) <= 254);

-- Every other read of this table so far has been by id or by lowered address,
-- both already indexed. The staff list is the first one that asks for a whole
-- restaurant's worth, and it is the read the admin screen makes on every visit.
CREATE INDEX staff_restaurant_idx ON public.staff (restaurant_id);

-- ===========================================================================
-- resolve_session returns must_change_password
--
-- The gate in AC-4 has to run on every request, before any handler body. The
-- extractor already reads this function once per request to learn the
-- restaurant, the staff id, the session id, and the role, so the flag rides
-- along on that same answer and costs no second query.
--
-- A DROP and CREATE rather than a CREATE OR REPLACE, because Postgres will not
-- let a replacement change the returned columns. That is the same reason 0005
-- did it this way when it added last_seen_at, and everything else is held
-- exactly as 0002, 0004, and 0005 left it: the same argument, still STABLE,
-- still SECURITY DEFINER, still owned by auth_lookup, still executable only by
-- app_api. The ownership and the grant are re asserted rather than assumed,
-- because a DROP takes them with it, and a function owned by the schema owner
-- returns no rows under FORCE ROW LEVEL SECURITY, which is a product where
-- nobody can sign in.
-- ===========================================================================

DROP FUNCTION public.resolve_session(bytea);

CREATE FUNCTION public.resolve_session(p_token_hash bytea)
RETURNS TABLE (
    session_id           uuid,
    staff_id             uuid,
    restaurant_id        uuid,
    role                 public.staff_role,
    expires_at           timestamptz,
    last_seen_at         timestamptz,
    must_change_password boolean
)
    LANGUAGE sql
    STABLE
    SECURITY DEFINER
    SET search_path = pg_catalog
    AS $$
        SELECT sess.id, sess.staff_id, sess.restaurant_id, st.role,
               sess.expires_at, sess.last_seen_at, st.must_change_password
        FROM public.sessions AS sess
        JOIN public.staff AS st
          ON st.id = sess.staff_id
         AND st.restaurant_id = sess.restaurant_id
        WHERE sess.token_hash = p_token_hash
          AND sess.revoked_at IS NULL
          AND sess.expires_at > now()
          AND sess.absolute_expires_at > now()
    $$;

COMMENT ON FUNCTION public.resolve_session(bytea) IS
    'The other of exactly two paths that read across restaurants. Owned by auth_lookup. A session that is revoked, past its expiry, or past its absolute ceiling returns no row. Carries must_change_password so the password gate costs no second read.';

ALTER FUNCTION public.resolve_session(bytea) OWNER TO auth_lookup;

REVOKE EXECUTE ON FUNCTION public.resolve_session(bytea) FROM PUBLIC;

DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'app_api') THEN
        GRANT EXECUTE ON FUNCTION public.resolve_session(bytea) TO app_api;
    ELSE
        RAISE WARNING 'role app_api does not exist; grant execute on resolve_session by hand';
    END IF;
END;
$$;

-- ===========================================================================
-- The six new audit actions
--
-- No column change: `action` is text with a not blank check, and the closed
-- list lives in Rust. Written down here so the schema reads as a record of what
-- the log can contain. Two of the six, staff_role_changed and
-- staff_deactivated, were already named by 0002; the other four are new.
--
--   staff_created           an admin created a member of staff
--   staff_edited            an admin changed somebody's display name
--   staff_role_changed      an admin changed somebody's role
--   staff_password_reset    an admin wrote a new password onto somebody's row
--   staff_deactivated       an admin switched an account off
--   staff_reactivated       an admin brought an account back
-- ===========================================================================
