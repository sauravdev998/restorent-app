-- Database roles, per spec 0001.
--
-- Three roles, and the split is load bearing. Postgres skips row level security
-- for a table's owner, so an API connecting as the owner would bypass every
-- policy silently and the tenant backstop would look present in code while doing
-- nothing in reality.
--
--   restaurant_owner  owns the schema and runs migrations (POSTGRES_USER here)
--   app_api           what the running API connects as: owns nothing, holds only
--                     the table grants it needs, is never a superuser
--   (on RDS)          the master user is used for neither
--
-- This file runs once, on first container start, as restaurant_owner.

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
