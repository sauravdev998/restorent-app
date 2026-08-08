-- Database roles, per specs 0001 and 0003.
--
-- Three roles, and the split is load bearing. Postgres skips row level security
-- for a table's owner, so an API connecting as the owner would bypass every
-- policy silently and the tenant backstop would look present in code while doing
-- nothing in reality.
--
--   restaurant_owner  owns the schema and runs migrations (POSTGRES_USER here)
--   app_api           what the running API connects as: owns nothing, holds only
--                     the table grants it needs, is never a superuser
--   auth_lookup       owns the two sign in lookup functions and nothing else.
--                     Never connects; it exists only to own them
--   (on RDS)          the master user is used for neither
--
-- This file runs once, on first container start, as restaurant_owner. On RDS it
-- is a manual step, and migration 0002 refuses to run until auth_lookup exists.

CREATE ROLE app_api LOGIN PASSWORD 'local_dev_only';

GRANT CONNECT ON DATABASE restaurant TO app_api;
GRANT USAGE ON SCHEMA public TO app_api;

-- app_api must never create objects: an object it created it would also own,
-- and ownership is exactly what bypasses row level security.
REVOKE CREATE ON SCHEMA public FROM app_api;
REVOKE CREATE ON SCHEMA public FROM PUBLIC;

-- Every table restaurant_owner creates from here on grants data access to
-- app_api automatically, so a new migration needs no grant of its own.
ALTER DEFAULT PRIVILEGES FOR ROLE restaurant_owner IN SCHEMA public
    GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO app_api;

ALTER DEFAULT PRIVILEGES FOR ROLE restaurant_owner IN SCHEMA public
    GRANT USAGE, SELECT ON SEQUENCES TO app_api;

-- ---------------------------------------------------------------------------
-- auth_lookup, per spec 0003.
--
-- Sign in has a chicken and egg problem: you cannot scope a transaction to a
-- restaurant until you know which restaurant the person belongs to, and finding
-- that out means reading a row with no scope set. Two narrow SECURITY DEFINER
-- functions answer it, and a SECURITY DEFINER function runs as its owner.
--
-- The owner cannot be restaurant_owner. FORCE ROW LEVEL SECURITY is exactly what
-- removes an owner's exemption from its own policies, so a function owned by the
-- schema owner would still be filtered, would find app.restaurant_id unset, and
-- would return no rows at all. Nobody would ever sign in.
--
-- So this role owns those two functions, owns no table, and holds only SELECT on
-- staff and sessions plus one named policy on each (migration 0002). NOLOGIN,
-- because nothing ever connects as it.
-- ---------------------------------------------------------------------------
CREATE ROLE auth_lookup NOLOGIN;

GRANT CONNECT ON DATABASE restaurant TO auth_lookup;
GRANT USAGE ON SCHEMA public TO auth_lookup;

REVOKE CREATE ON SCHEMA public FROM auth_lookup;

-- Changing a function's owner requires membership in the role taking ownership,
-- so the migration cannot hand the two functions over without this. It does mean
-- restaurant_owner inherits auth_lookup's unfiltered read on staff and sessions,
-- which is accepted: restaurant_owner runs migrations and nothing else.
GRANT auth_lookup TO restaurant_owner;
