-- ===========================================================================
-- 0004: accounts, restaurants, and roles
--
-- Four column additions, one new table, and one altered function. Nothing here
-- changes an existing column or an existing policy, so migration 0002 and the
-- tests feature 4 wrote against it are untouched.
--
-- The three columns are `NOT NULL` outright rather than `NOT NULL DEFAULT ...`
-- followed by a drop, because there are no rows yet: the product has no
-- registration path until this migration lands, so `restaurants`, `staff`, and
-- `sessions` are empty by construction. A default here would be a value that
-- exists only to satisfy the migration and then sits in the schema forever,
-- ready to be inherited by a row nobody meant to create.
-- ===========================================================================

-- ===========================================================================
-- restaurants.country_code
--
-- Where the restaurant is. Registration reads the matching row in
-- locales/countries.json and writes that row's currency, decimals, timezone,
-- default language, and formatting locale onto the restaurant, so this column
-- is the record of which row those five came from.
--
-- Upper case only, the same shape as the currency code check right above it in
-- 0002, so a restaurant stored as 'in' and one stored as 'IN' can never be two
-- different countries to a report that groups on this column.
-- ===========================================================================

ALTER TABLE public.restaurants
    ADD COLUMN country_code char(2) NOT NULL;

ALTER TABLE public.restaurants
    ADD CONSTRAINT restaurants_country_code_is_upper_case
        CHECK (country_code ~ '^[A-Z]{2}$');

COMMENT ON COLUMN public.restaurants.country_code IS
    'ISO 3166-1 alpha-2, upper case. Names the row in locales/countries.json that registration took this restaurant''s currency, decimals, timezone, default language, and formatting locale from. Validated in Rust on write; a check constraint cannot read that file.';

-- ===========================================================================
-- staff.last_sign_in_at
--
-- NULL means never signed in, which is exactly what feature 10's staff list
-- has to show for an account an admin created an hour ago.
-- ===========================================================================

ALTER TABLE public.staff
    ADD COLUMN last_sign_in_at timestamptz;

COMMENT ON COLUMN public.staff.last_sign_in_at IS
    'When this person last signed in. NULL means never, which is what feature 10 shows for a freshly created account.';

-- ===========================================================================
-- sessions.absolute_expires_at
--
-- The ceiling a sliding session cannot slide past. `expires_at` moves forward
-- every time the session is used; this one is written once at sign in and never
-- moves again, so a phone in somebody's pocket that is opened every day still
-- stops working eventually.
--
-- The check is what makes the pair meaningful rather than decorative: a row
-- whose ceiling is already behind its expiry is a session that is expired the
-- moment it is created, and it would be created silently.
-- ===========================================================================

ALTER TABLE public.sessions
    ADD COLUMN absolute_expires_at timestamptz NOT NULL;

ALTER TABLE public.sessions
    ADD CONSTRAINT sessions_absolute_expiry_is_the_later_one
        CHECK (absolute_expires_at > expires_at);

COMMENT ON COLUMN public.sessions.absolute_expires_at IS
    'Written once at sign in and never moved. The sliding refresh moves expires_at only, so a session used every day still ends here.';

-- ===========================================================================
-- login_attempts: the one table with no restaurant
--
-- Both throttle buckets are counted here rather than in a process, because
-- there are several containers behind one load balancer and an in process
-- limiter would grant every caller the full allowance on each of them.
--
-- It carries no restaurant_id, no foreign key, and no row level security, and
-- that is deliberate rather than an oversight. A failed sign in happens before
-- anybody knows which restaurant the address belongs to, and often for an
-- address that belongs to no restaurant at all, so there is no tenant to scope
-- it to. It holds no tenant data either: an email address somebody typed and
-- the address they typed it from, nothing that answers a question about any
-- restaurant. It is therefore not a third cross tenant read path; the two named
-- in 0002 are still the only two.
-- ===========================================================================

CREATE TABLE public.login_attempts (
    id           uuid PRIMARY KEY,
    email        text NOT NULL,
    ip           inet,
    attempted_at timestamptz NOT NULL DEFAULT now(),

    CONSTRAINT login_attempts_email_not_blank CHECK (length(btrim(email)) > 0)
);

COMMENT ON TABLE public.login_attempts IS
    'Deliberately the one table with no restaurant_id and no row level security. A failed sign in happens before the restaurant is known, and often for an address belonging to none. Holds no tenant data, so it answers nothing about any restaurant. Both throttle buckets are counted here so they hold across containers rather than per process.';

COMMENT ON COLUMN public.login_attempts.email IS
    'Stored lowered, because the bucket is per account and an account is identified case insensitively.';

COMMENT ON COLUMN public.login_attempts.ip IS
    'The client address, from CloudFront-Viewer-Address outside development and the socket peer in development. NULL when neither is available.';

-- Serves the per address bucket's count and the sign in sweep alike: both ask
-- for one lowered email over a time window.
CREATE INDEX login_attempts_email_recent_idx
    ON public.login_attempts (email, attempted_at DESC);

-- Serves the much more generous per client address bucket.
CREATE INDEX login_attempts_ip_recent_idx
    ON public.login_attempts (ip, attempted_at DESC);

-- Granted directly rather than inherited, so the table is correct on a database
-- where default privileges were never set. app_api still owns nothing.
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'app_api') THEN
        GRANT SELECT, INSERT, DELETE ON public.login_attempts TO app_api;
    ELSE
        RAISE WARNING 'role app_api does not exist; grant access to login_attempts by hand (see api/scripts/init-roles.sql)';
    END IF;
END;
$$;

-- ===========================================================================
-- resolve_session gains the absolute ceiling
--
-- One added condition and nothing else: the same arguments, the same returned
-- columns, still STABLE, still SECURITY DEFINER, still owned by auth_lookup.
--
-- The ownership is re asserted rather than assumed. CREATE OR REPLACE keeps the
-- existing owner, so on a database where 0002 ran correctly this changes
-- nothing; on one where the function was somehow recreated by the schema owner
-- it puts it back, and the alternative is a function that silently returns no
-- rows under FORCE ROW LEVEL SECURITY and a product where nobody can sign in.
-- ===========================================================================

CREATE OR REPLACE FUNCTION public.resolve_session(p_token_hash bytea)
RETURNS TABLE (
    session_id     uuid,
    staff_id       uuid,
    restaurant_id  uuid,
    role           public.staff_role,
    expires_at     timestamptz
)
    LANGUAGE sql
    STABLE
    SECURITY DEFINER
    SET search_path = pg_catalog
    AS $$
        SELECT sess.id, sess.staff_id, sess.restaurant_id, st.role, sess.expires_at
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

-- ===========================================================================
-- The three new audit actions
--
-- No column change: `action` is text with a not blank check, and the closed
-- list lives in Rust. Written down here so the schema reads as a record of what
-- the log can contain.
--
--   restaurant_registered        an owner registered a restaurant
--   password_changed             somebody changed their own password
--   restaurant_settings_updated  an admin edited the restaurant's settings
-- ===========================================================================
