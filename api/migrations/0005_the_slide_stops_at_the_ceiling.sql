-- ===========================================================================
-- The sliding expiry is allowed to reach the ceiling, not to pass it
--
-- 0004 wrote the pair as `absolute_expires_at > expires_at`, strictly greater.
-- That reads correctly right up until a session gets old. `expires_at` slides
-- to fourteen days ahead on every use; `absolute_expires_at` never moves. Once
-- a session is inside fourteen days of its ninety day ceiling, "fourteen days
-- ahead" is past the ceiling, and the slide's own UPDATE violated this check.
--
-- What that looked like in practice: the request still succeeded, because the
-- extractor treats a failed slide as a line in the log rather than a refusal,
-- but `last_seen_at` never moved. So the same UPDATE failed again on the next
-- request, and the next, for the final two weeks of the session's life, each
-- one writing a database error into the logs for a session that was working
-- perfectly well.
--
-- The invariant that was actually meant is "the ceiling is never earlier than
-- the sliding expiry". `>=` says that. The slide is clamped to the ceiling in
-- the same change (`sessions::slide`), so a session near the end now expires
-- exactly at its ceiling, which is what the ceiling is for.
--
-- Equality is not a session that is already dead: `resolve_session` refuses on
-- `expires_at > now()` and `absolute_expires_at > now()`, and when the two are
-- equal they simply fail together, at the ceiling, which is the intent.
-- ===========================================================================

ALTER TABLE public.sessions
    DROP CONSTRAINT sessions_absolute_expiry_is_the_later_one;

ALTER TABLE public.sessions
    ADD CONSTRAINT sessions_absolute_expiry_is_never_the_earlier_one
        CHECK (absolute_expires_at >= expires_at);

COMMENT ON COLUMN public.sessions.absolute_expires_at IS
    'Written once at sign in and never moved. The sliding refresh moves expires_at only, and stops at this value, so a session used every day still ends here.';

-- ===========================================================================
-- resolve_session returns last_seen_at
--
-- The Rust side used to work out "when was this session last used" by
-- subtracting the session lifetime from `expires_at`, which was exact only
-- while `expires_at` was always `last_seen_at` plus that lifetime. Clamping the
-- slide to the ceiling ends that, and a derived value that is quietly wrong for
-- the last fourteen days of every session is worse than one more column.
--
-- So the lookup returns the real thing. A DROP and CREATE rather than a
-- CREATE OR REPLACE, because Postgres will not let a replacement change the
-- returned columns. Everything else is held exactly as 0002 and 0004 left it:
-- the same argument, still STABLE, still SECURITY DEFINER, still owned by
-- auth_lookup, still executable only by app_api. The ownership and the grant
-- are re asserted rather than assumed, because a DROP takes them with it, and
-- a function owned by the schema owner returns no rows under FORCE ROW LEVEL
-- SECURITY, which is a product where nobody can sign in.
-- ===========================================================================

DROP FUNCTION public.resolve_session(bytea);

CREATE FUNCTION public.resolve_session(p_token_hash bytea)
RETURNS TABLE (
    session_id     uuid,
    staff_id       uuid,
    restaurant_id  uuid,
    role           public.staff_role,
    expires_at     timestamptz,
    last_seen_at   timestamptz
)
    LANGUAGE sql
    STABLE
    SECURITY DEFINER
    SET search_path = pg_catalog
    AS $$
        SELECT sess.id, sess.staff_id, sess.restaurant_id, st.role,
               sess.expires_at, sess.last_seen_at
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
    'The other of exactly two paths that read across restaurants. Owned by auth_lookup. A session that is revoked, past its expiry, or past its absolute ceiling returns no row.';

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
