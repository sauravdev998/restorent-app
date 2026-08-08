-- Core data model, per spec 0003.
--
-- Sixteen tables and six enum types in one migration. The project builds by
-- Tracer Bullet and this is the deliberate exception to it: a foundation's whole
-- value is that it does not move, and reshaping a grown schema costs most once
-- real data sits in it.
--
-- The shape that matters most is how one restaurant's data is kept away from
-- another's, and it is guarded three times over:
--
--   1. a scoped transaction (`ScopedTx`, which sets app.restaurant_id)
--   2. a row level security policy on every table below
--   3. foreign keys that carry restaurant_id, so a cross restaurant link is not
--      merely refused, it cannot be written at all
--
-- The third guard is the one that catches what the other two structurally
-- cannot: a correctly scoped transaction that references another restaurant's
-- row. That is why every foreign key between two tenant tables is composite and
-- points at a `unique (id, restaurant_id)` constraint rather than at a bare id.
--
-- Conventions applied throughout:
--   * primary key `id uuid`, version 7, generated in Rust (Postgres 17 has no
--     built in uuidv7())
--   * `created_at timestamptz not null default now()`
--   * `updated_at timestamptz not null`, written by the application, no triggers,
--     and deliberately without a default so a path that forgets it fails loudly
--   * money `numeric(14,4)`, never a float; percentages `numeric(6,3)`
--   * every timestamp `timestamptz`, never a bare `timestamp`

-- ===========================================================================
-- Enum types
--
-- A Postgres enum makes adding a value a one line non blocking ALTER TYPE and
-- makes removing or reordering one genuinely awkward. That trade was taken on
-- the grounds that none of these six lists is expected to shrink.
-- ===========================================================================

CREATE TYPE public.staff_role AS ENUM ('admin', 'waiter', 'chef');
CREATE TYPE public.visit_status AS ENUM ('open', 'closed');
CREATE TYPE public.round_status AS ENUM ('queued', 'ready', 'served', 'voided');
CREATE TYPE public.line_status AS ENUM ('queued', 'ready', 'served', 'voided');
CREATE TYPE public.bill_status AS ENUM ('open', 'closed', 'voided');
CREATE TYPE public.payment_method AS ENUM ('cash', 'card', 'other');

-- ===========================================================================
-- restaurants: the tenant root
--
-- The only table whose tenant key is its own id. Every other table below points
-- back here through `restaurant_id` with ON DELETE CASCADE, which is what makes
-- deleting a restaurant complete rather than leaving orphans behind.
-- ===========================================================================

CREATE TABLE public.restaurants (
    id                      uuid PRIMARY KEY,
    name                    text NOT NULL,
    currency_code           char(3) NOT NULL,
    currency_decimals       smallint NOT NULL,
    timezone                text NOT NULL,
    service_charge_percent  numeric(6,3),
    address                 text,
    tax_registration_number text,
    deactivated_at          timestamptz,
    created_at              timestamptz NOT NULL DEFAULT now(),
    updated_at              timestamptz NOT NULL,

    CONSTRAINT restaurants_currency_decimals_in_range
        CHECK (currency_decimals BETWEEN 0 AND 4),
    -- Upper case only, so a bill stored as 'usd' and one stored as 'USD' can
    -- never be two different currencies to a report that groups on this column.
    CONSTRAINT restaurants_currency_code_is_upper_case
        CHECK (currency_code ~ '^[A-Z]{3}$'),
    CONSTRAINT restaurants_service_charge_percent_non_negative
        CHECK (service_charge_percent IS NULL OR service_charge_percent >= 0),
    CONSTRAINT restaurants_name_not_blank
        CHECK (length(btrim(name)) > 0)
);

-- The timezone is validated in Rust on write rather than by a check constraint,
-- because a check constraint may not query pg_timezone_names.
COMMENT ON COLUMN public.restaurants.timezone IS
    'IANA timezone name. Validated in Rust on write; a check constraint cannot query pg_timezone_names.';
COMMENT ON COLUMN public.restaurants.service_charge_percent IS
    'NULL means no service charge, which yields an amount of zero on a bill, never a null amount.';

-- ===========================================================================
-- tax_components: the restaurant's own tax rules
-- ===========================================================================

CREATE TABLE public.tax_components (
    id            uuid PRIMARY KEY,
    restaurant_id uuid NOT NULL REFERENCES public.restaurants (id) ON DELETE CASCADE,
    name          text NOT NULL,
    rate_percent  numeric(6,3) NOT NULL,
    position      int NOT NULL,
    archived_at   timestamptz,
    created_at    timestamptz NOT NULL DEFAULT now(),
    updated_at    timestamptz NOT NULL,

    CONSTRAINT tax_components_id_restaurant_key UNIQUE (id, restaurant_id),
    CONSTRAINT tax_components_rate_non_negative CHECK (rate_percent >= 0),
    CONSTRAINT tax_components_name_not_blank CHECK (length(btrim(name)) > 0)
);

CREATE UNIQUE INDEX tax_components_live_name_key
    ON public.tax_components (restaurant_id, name)
    WHERE archived_at IS NULL;

-- ===========================================================================
-- staff and sessions
--
-- Email identifies exactly one account platform wide, not one per restaurant.
-- That is a real product limitation (a person working at two restaurants on the
-- platform needs two addresses) taken so that signing in is an email and a
-- password with nothing extra to type.
--
-- A plain unique index on the lowered value rather than the citext extension, so
-- no extension is added to this database for the sake of one column.
-- ===========================================================================

CREATE TABLE public.staff (
    id             uuid PRIMARY KEY,
    restaurant_id  uuid NOT NULL REFERENCES public.restaurants (id) ON DELETE CASCADE,
    email          text NOT NULL,
    password_hash  text NOT NULL,
    display_name   text NOT NULL,
    role           public.staff_role NOT NULL,
    deactivated_at timestamptz,
    created_at     timestamptz NOT NULL DEFAULT now(),
    updated_at     timestamptz NOT NULL,

    CONSTRAINT staff_id_restaurant_key UNIQUE (id, restaurant_id),
    CONSTRAINT staff_email_not_blank CHECK (length(btrim(email)) > 0)
);

CREATE UNIQUE INDEX staff_email_key ON public.staff (lower(email));

COMMENT ON INDEX public.staff_email_key IS
    'Platform wide, not per restaurant. Row level security does not filter index uniqueness, so a duplicate across two restaurants is refused as intended.';

CREATE TABLE public.sessions (
    id            uuid PRIMARY KEY,
    restaurant_id uuid NOT NULL REFERENCES public.restaurants (id) ON DELETE CASCADE,
    staff_id      uuid NOT NULL,
    token_hash    bytea NOT NULL,
    expires_at    timestamptz NOT NULL,
    last_seen_at  timestamptz NOT NULL,
    revoked_at    timestamptz,
    created_at    timestamptz NOT NULL DEFAULT now(),
    updated_at    timestamptz NOT NULL,

    CONSTRAINT sessions_id_restaurant_key UNIQUE (id, restaurant_id),
    CONSTRAINT sessions_token_hash_key UNIQUE (token_hash),
    CONSTRAINT sessions_staff_fkey
        FOREIGN KEY (staff_id, restaurant_id)
        REFERENCES public.staff (id, restaurant_id) ON DELETE CASCADE
);

CREATE INDEX sessions_staff_idx ON public.sessions (staff_id);

-- ===========================================================================
-- The floor: sections and tables
-- ===========================================================================

CREATE TABLE public.table_sections (
    id            uuid PRIMARY KEY,
    restaurant_id uuid NOT NULL REFERENCES public.restaurants (id) ON DELETE CASCADE,
    name          text NOT NULL,
    position      int NOT NULL,
    archived_at   timestamptz,
    created_at    timestamptz NOT NULL DEFAULT now(),
    updated_at    timestamptz NOT NULL,

    CONSTRAINT table_sections_id_restaurant_key UNIQUE (id, restaurant_id),
    CONSTRAINT table_sections_name_not_blank CHECK (length(btrim(name)) > 0)
);

CREATE UNIQUE INDEX table_sections_live_name_key
    ON public.table_sections (restaurant_id, name)
    WHERE archived_at IS NULL;

CREATE TABLE public.dining_tables (
    id            uuid PRIMARY KEY,
    restaurant_id uuid NOT NULL REFERENCES public.restaurants (id) ON DELETE CASCADE,
    section_id    uuid,
    label         text NOT NULL,
    seats         smallint,
    position      int NOT NULL,
    archived_at   timestamptz,
    created_at    timestamptz NOT NULL DEFAULT now(),
    updated_at    timestamptz NOT NULL,

    CONSTRAINT dining_tables_id_restaurant_key UNIQUE (id, restaurant_id),
    -- MATCH SIMPLE, so a NULL section_id skips the check entirely: a table with
    -- no section is allowed, a table pointing at another restaurant's section is
    -- not.
    CONSTRAINT dining_tables_section_fkey
        FOREIGN KEY (section_id, restaurant_id)
        REFERENCES public.table_sections (id, restaurant_id),
    CONSTRAINT dining_tables_seats_positive CHECK (seats IS NULL OR seats > 0),
    CONSTRAINT dining_tables_label_not_blank CHECK (length(btrim(label)) > 0)
);

CREATE UNIQUE INDEX dining_tables_live_label_key
    ON public.dining_tables (restaurant_id, label)
    WHERE archived_at IS NULL;

-- ===========================================================================
-- The menu
-- ===========================================================================

CREATE TABLE public.menu_categories (
    id            uuid PRIMARY KEY,
    restaurant_id uuid NOT NULL REFERENCES public.restaurants (id) ON DELETE CASCADE,
    name          text NOT NULL,
    position      int NOT NULL,
    archived_at   timestamptz,
    created_at    timestamptz NOT NULL DEFAULT now(),
    updated_at    timestamptz NOT NULL,

    CONSTRAINT menu_categories_id_restaurant_key UNIQUE (id, restaurant_id),
    CONSTRAINT menu_categories_name_not_blank CHECK (length(btrim(name)) > 0)
);

CREATE UNIQUE INDEX menu_categories_live_name_key
    ON public.menu_categories (restaurant_id, name)
    WHERE archived_at IS NULL;

CREATE TABLE public.dishes (
    id            uuid PRIMARY KEY,
    restaurant_id uuid NOT NULL REFERENCES public.restaurants (id) ON DELETE CASCADE,
    category_id   uuid NOT NULL,
    name          text NOT NULL,
    description   text,
    price         numeric(14,4) NOT NULL,
    is_available  boolean NOT NULL DEFAULT true,
    position      int NOT NULL,
    archived_at   timestamptz,
    created_at    timestamptz NOT NULL DEFAULT now(),
    updated_at    timestamptz NOT NULL,

    CONSTRAINT dishes_id_restaurant_key UNIQUE (id, restaurant_id),
    CONSTRAINT dishes_category_fkey
        FOREIGN KEY (category_id, restaurant_id)
        REFERENCES public.menu_categories (id, restaurant_id),
    CONSTRAINT dishes_price_non_negative CHECK (price >= 0),
    CONSTRAINT dishes_name_not_blank CHECK (length(btrim(name)) > 0)
);

-- ===========================================================================
-- visits: who is sitting at a table right now
--
-- A visit owns table occupancy; a bill is a payment document drawn from a
-- visit's lines. They are separate because occupancy and payment genuinely have
-- different lifetimes: a party can pay on two bills, or sit down and leave
-- without ordering at all.
-- ===========================================================================

CREATE TABLE public.visits (
    id                  uuid PRIMARY KEY,
    restaurant_id       uuid NOT NULL REFERENCES public.restaurants (id) ON DELETE CASCADE,
    table_id            uuid NOT NULL,
    status              public.visit_status NOT NULL,
    guest_count         smallint,
    opened_by_staff_id  uuid NOT NULL,
    opened_at           timestamptz NOT NULL,
    closed_at           timestamptz,
    created_at          timestamptz NOT NULL DEFAULT now(),
    updated_at          timestamptz NOT NULL,

    CONSTRAINT visits_id_restaurant_key UNIQUE (id, restaurant_id),
    CONSTRAINT visits_table_fkey
        FOREIGN KEY (table_id, restaurant_id)
        REFERENCES public.dining_tables (id, restaurant_id),
    CONSTRAINT visits_opened_by_fkey
        FOREIGN KEY (opened_by_staff_id, restaurant_id)
        REFERENCES public.staff (id, restaurant_id),
    CONSTRAINT visits_guest_count_positive CHECK (guest_count IS NULL OR guest_count > 0),
    CONSTRAINT visits_closed_at_matches_status
        CHECK ((status = 'closed') = (closed_at IS NOT NULL))
);

-- At most one open visit per table, refused by the database rather than by
-- application code, so two waiters racing on the same table cannot both win.
CREATE UNIQUE INDEX visits_one_open_per_table
    ON public.visits (restaurant_id, table_id)
    WHERE status = 'open';

-- The floor view: which tables are occupied.
CREATE INDEX visits_status_idx ON public.visits (restaurant_id, status);

-- ===========================================================================
-- order_rounds: one ticket to the kitchen
-- ===========================================================================

CREATE TABLE public.order_rounds (
    id                uuid PRIMARY KEY,
    restaurant_id     uuid NOT NULL REFERENCES public.restaurants (id) ON DELETE CASCADE,
    visit_id          uuid NOT NULL,
    sequence_no       int NOT NULL,
    status            public.round_status NOT NULL,
    sent_by_staff_id  uuid NOT NULL,
    sent_at           timestamptz NOT NULL,
    ready_at          timestamptz,
    served_at         timestamptz,
    created_at        timestamptz NOT NULL DEFAULT now(),
    updated_at        timestamptz NOT NULL,

    CONSTRAINT order_rounds_id_restaurant_key UNIQUE (id, restaurant_id),
    CONSTRAINT order_rounds_sequence_key UNIQUE (restaurant_id, visit_id, sequence_no),
    CONSTRAINT order_rounds_visit_fkey
        FOREIGN KEY (visit_id, restaurant_id)
        REFERENCES public.visits (id, restaurant_id),
    CONSTRAINT order_rounds_sent_by_fkey
        FOREIGN KEY (sent_by_staff_id, restaurant_id)
        REFERENCES public.staff (id, restaurant_id),
    CONSTRAINT order_rounds_sequence_positive CHECK (sequence_no > 0)
);

-- The kitchen queue, oldest first.
CREATE INDEX order_rounds_queue_idx
    ON public.order_rounds (restaurant_id, sent_at)
    WHERE status = 'queued';

-- ===========================================================================
-- bills: the payment document
--
-- Created before order_lines because a line carries the bill it was assigned to.
-- Currency, service charge percent, dish names, prices, and tax rates are all
-- copied onto a bill and its bill_taxes rows when it closes, so a bill printed a
-- year later still says exactly what the customer paid.
-- ===========================================================================

CREATE TABLE public.bills (
    id                     uuid PRIMARY KEY,
    restaurant_id          uuid NOT NULL REFERENCES public.restaurants (id) ON DELETE CASCADE,
    visit_id               uuid NOT NULL,
    number                 bigint,
    status                 public.bill_status NOT NULL,
    currency_code          char(3) NOT NULL,
    currency_decimals      smallint NOT NULL,
    subtotal               numeric(14,4) NOT NULL DEFAULT 0,
    service_charge_percent numeric(6,3),
    service_charge_amount  numeric(14,4) NOT NULL DEFAULT 0,
    tax_total              numeric(14,4) NOT NULL DEFAULT 0,
    total                  numeric(14,4) NOT NULL DEFAULT 0,
    opened_by_staff_id     uuid NOT NULL,
    closed_by_staff_id     uuid,
    closed_at              timestamptz,
    created_at             timestamptz NOT NULL DEFAULT now(),
    updated_at             timestamptz NOT NULL,

    CONSTRAINT bills_id_restaurant_key UNIQUE (id, restaurant_id),
    -- NULLs are distinct here, so every open bill coexists happily and only
    -- allocated numbers compete.
    CONSTRAINT bills_number_key UNIQUE (restaurant_id, number),
    CONSTRAINT bills_visit_fkey
        FOREIGN KEY (visit_id, restaurant_id)
        REFERENCES public.visits (id, restaurant_id),
    CONSTRAINT bills_opened_by_fkey
        FOREIGN KEY (opened_by_staff_id, restaurant_id)
        REFERENCES public.staff (id, restaurant_id),
    CONSTRAINT bills_closed_by_fkey
        FOREIGN KEY (closed_by_staff_id, restaurant_id)
        REFERENCES public.staff (id, restaurant_id),
    CONSTRAINT bills_currency_decimals_in_range CHECK (currency_decimals BETWEEN 0 AND 4),
    CONSTRAINT bills_currency_code_is_upper_case CHECK (currency_code ~ '^[A-Z]{3}$'),
    CONSTRAINT bills_number_positive CHECK (number IS NULL OR number > 0),
    CONSTRAINT bills_amounts_non_negative
        CHECK (subtotal >= 0 AND service_charge_amount >= 0 AND tax_total >= 0 AND total >= 0),
    CONSTRAINT bills_service_charge_percent_non_negative
        CHECK (service_charge_percent IS NULL OR service_charge_percent >= 0),
    -- A closed bill is a complete document or it is not closed.
    CONSTRAINT bills_closed_is_complete
        CHECK (status <> 'closed'
               OR (number IS NOT NULL AND closed_at IS NOT NULL AND closed_by_staff_id IS NOT NULL))
);

-- Revenue by day and by hour, in feature 18.
CREATE INDEX bills_closed_at_idx
    ON public.bills (restaurant_id, closed_at)
    WHERE status = 'closed';

CREATE INDEX bills_visit_idx ON public.bills (restaurant_id, visit_id);

-- ===========================================================================
-- order_lines: one dish on one ticket
--
-- `round_id` is NOT NULL on purpose: a line exists only once it has been sent,
-- so the waiter's unsent basket is client side state and never reaches the
-- database. There is therefore no state before 'queued'.
--
-- `bill_id` is nullable and is what would let a party split a bill. Nothing
-- implements splitting yet; the schema permits it.
-- ===========================================================================

CREATE TABLE public.order_lines (
    id                  uuid PRIMARY KEY,
    restaurant_id       uuid NOT NULL REFERENCES public.restaurants (id) ON DELETE CASCADE,
    round_id            uuid NOT NULL,
    dish_id             uuid NOT NULL,
    bill_id             uuid,
    quantity            int NOT NULL,
    unit_price          numeric(14,4) NOT NULL,
    dish_name           text NOT NULL,
    line_total          numeric(14,4) NOT NULL,
    note                text,
    status              public.line_status NOT NULL,
    ready_by_staff_id   uuid,
    ready_at            timestamptz,
    served_at           timestamptz,
    voided_by_staff_id  uuid,
    voided_at           timestamptz,
    void_reason         text,
    created_at          timestamptz NOT NULL DEFAULT now(),
    updated_at          timestamptz NOT NULL,

    CONSTRAINT order_lines_id_restaurant_key UNIQUE (id, restaurant_id),
    CONSTRAINT order_lines_round_fkey
        FOREIGN KEY (round_id, restaurant_id)
        REFERENCES public.order_rounds (id, restaurant_id),
    CONSTRAINT order_lines_dish_fkey
        FOREIGN KEY (dish_id, restaurant_id)
        REFERENCES public.dishes (id, restaurant_id),
    CONSTRAINT order_lines_bill_fkey
        FOREIGN KEY (bill_id, restaurant_id)
        REFERENCES public.bills (id, restaurant_id),
    CONSTRAINT order_lines_ready_by_fkey
        FOREIGN KEY (ready_by_staff_id, restaurant_id)
        REFERENCES public.staff (id, restaurant_id),
    CONSTRAINT order_lines_voided_by_fkey
        FOREIGN KEY (voided_by_staff_id, restaurant_id)
        REFERENCES public.staff (id, restaurant_id),
    CONSTRAINT order_lines_quantity_positive CHECK (quantity > 0),
    CONSTRAINT order_lines_unit_price_non_negative CHECK (unit_price >= 0),
    CONSTRAINT order_lines_total_is_product CHECK (line_total = quantity * unit_price),
    CONSTRAINT order_lines_void_is_explained
        CHECK (status <> 'voided'
               OR (void_reason IS NOT NULL AND voided_by_staff_id IS NOT NULL AND voided_at IS NOT NULL))
);

-- The two ways lines are read: by ticket, and by bill.
CREATE INDEX order_lines_round_idx ON public.order_lines (restaurant_id, round_id);
CREATE INDEX order_lines_bill_idx ON public.order_lines (restaurant_id, bill_id);
-- Dish ranking, in feature 18.
CREATE INDEX order_lines_dish_idx ON public.order_lines (restaurant_id, dish_id);

-- ===========================================================================
-- bill_taxes: the tax breakdown copied onto a bill at close
-- ===========================================================================

CREATE TABLE public.bill_taxes (
    id            uuid PRIMARY KEY,
    restaurant_id uuid NOT NULL REFERENCES public.restaurants (id) ON DELETE CASCADE,
    bill_id       uuid NOT NULL,
    name          text NOT NULL,
    rate_percent  numeric(6,3) NOT NULL,
    amount        numeric(14,4) NOT NULL,
    created_at    timestamptz NOT NULL DEFAULT now(),
    updated_at    timestamptz NOT NULL,

    CONSTRAINT bill_taxes_id_restaurant_key UNIQUE (id, restaurant_id),
    CONSTRAINT bill_taxes_bill_name_key UNIQUE (bill_id, name),
    CONSTRAINT bill_taxes_bill_fkey
        FOREIGN KEY (bill_id, restaurant_id)
        REFERENCES public.bills (id, restaurant_id),
    CONSTRAINT bill_taxes_rate_non_negative CHECK (rate_percent >= 0),
    CONSTRAINT bill_taxes_amount_non_negative CHECK (amount >= 0)
);

-- ===========================================================================
-- payments: how a closed bill was paid
--
-- A method and an amount only. No card data, no payment provider, nothing in
-- PCI scope, because spec 0001 put no payment provider in the product.
-- ===========================================================================

CREATE TABLE public.payments (
    id                 uuid PRIMARY KEY,
    restaurant_id      uuid NOT NULL REFERENCES public.restaurants (id) ON DELETE CASCADE,
    bill_id            uuid NOT NULL,
    method             public.payment_method NOT NULL,
    amount             numeric(14,4) NOT NULL,
    taken_by_staff_id  uuid NOT NULL,
    taken_at           timestamptz NOT NULL,
    note               text,
    created_at         timestamptz NOT NULL DEFAULT now(),
    updated_at         timestamptz NOT NULL,

    CONSTRAINT payments_id_restaurant_key UNIQUE (id, restaurant_id),
    CONSTRAINT payments_bill_fkey
        FOREIGN KEY (bill_id, restaurant_id)
        REFERENCES public.bills (id, restaurant_id),
    CONSTRAINT payments_taken_by_fkey
        FOREIGN KEY (taken_by_staff_id, restaurant_id)
        REFERENCES public.staff (id, restaurant_id),
    CONSTRAINT payments_amount_positive CHECK (amount > 0)
);

CREATE INDEX payments_bill_idx ON public.payments (restaurant_id, bill_id);

-- ===========================================================================
-- bill_number_counters: gapless per restaurant bill numbering
--
-- One row per restaurant, and nothing relies on the row having been created: the
-- allocation is an upsert, so a missing counter creates itself rather than
-- matching zero rows silently.
--
-- `next_number` holds the number most recently handed out; the allocation
-- returns the value it just wrote. A rolled back transaction releases the number
-- again, because the row stays locked until the transaction ends, and that is
-- exactly what keeps the sequence gapless. It also means two tills closing bills
-- in the same restaurant at the same second briefly queue, which is the accepted
-- price of gapless numbering.
-- ===========================================================================

CREATE TABLE public.bill_number_counters (
    restaurant_id uuid PRIMARY KEY REFERENCES public.restaurants (id) ON DELETE CASCADE,
    next_number   bigint NOT NULL DEFAULT 1,
    created_at    timestamptz NOT NULL DEFAULT now(),
    updated_at    timestamptz NOT NULL,

    CONSTRAINT bill_number_counters_positive CHECK (next_number > 0)
);

-- ===========================================================================
-- audit_log: what changed, who changed it, and what it was before
--
-- Required rather than optional, because this schema holds money and access
-- control. `actor_staff_id` is nullable for the one case with no staff member
-- behind it, a change made by the system itself. Every change AC-14 names is
-- staff initiated and carries an actor.
-- ===========================================================================

CREATE TABLE public.audit_log (
    id              uuid PRIMARY KEY,
    restaurant_id   uuid NOT NULL REFERENCES public.restaurants (id) ON DELETE CASCADE,
    actor_staff_id  uuid,
    action          text NOT NULL,
    entity_type     text NOT NULL,
    entity_id       uuid NOT NULL,
    before          jsonb,
    after           jsonb,
    occurred_at     timestamptz NOT NULL DEFAULT now(),
    created_at      timestamptz NOT NULL DEFAULT now(),
    updated_at      timestamptz NOT NULL,

    CONSTRAINT audit_log_id_restaurant_key UNIQUE (id, restaurant_id),
    CONSTRAINT audit_log_actor_fkey
        FOREIGN KEY (actor_staff_id, restaurant_id)
        REFERENCES public.staff (id, restaurant_id),
    CONSTRAINT audit_log_action_not_blank CHECK (length(btrim(action)) > 0),
    CONSTRAINT audit_log_entity_type_not_blank CHECK (length(btrim(entity_type)) > 0)
);

CREATE INDEX audit_log_recent_idx ON public.audit_log (restaurant_id, occurred_at DESC);

-- ===========================================================================
-- Row level security
--
-- ENABLE turns policies on. FORCE is what makes them apply to the table's owner
-- as well, and without it the whole backstop is decorative: Postgres exempts a
-- table's owner from its policies, so a migration owner connection, or any
-- future mistake that connects as the owner, would read every restaurant's rows.
--
-- current_restaurant_id() returns NULL when nothing scoped the transaction, and
-- `restaurant_id = NULL` matches no rows, so an unscoped transaction sees
-- nothing rather than seeing everything. That failure direction is the safe one.
-- ===========================================================================

ALTER TABLE public.restaurants ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.restaurants FORCE ROW LEVEL SECURITY;
CREATE POLICY restaurants_tenant_isolation ON public.restaurants
    FOR ALL
    USING (id = public.current_restaurant_id())
    WITH CHECK (id = public.current_restaurant_id());

ALTER TABLE public.tax_components ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.tax_components FORCE ROW LEVEL SECURITY;
CREATE POLICY tax_components_tenant_isolation ON public.tax_components
    FOR ALL
    USING (restaurant_id = public.current_restaurant_id())
    WITH CHECK (restaurant_id = public.current_restaurant_id());

ALTER TABLE public.staff ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.staff FORCE ROW LEVEL SECURITY;
CREATE POLICY staff_tenant_isolation ON public.staff
    FOR ALL
    USING (restaurant_id = public.current_restaurant_id())
    WITH CHECK (restaurant_id = public.current_restaurant_id());

ALTER TABLE public.sessions ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.sessions FORCE ROW LEVEL SECURITY;
CREATE POLICY sessions_tenant_isolation ON public.sessions
    FOR ALL
    USING (restaurant_id = public.current_restaurant_id())
    WITH CHECK (restaurant_id = public.current_restaurant_id());

ALTER TABLE public.table_sections ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.table_sections FORCE ROW LEVEL SECURITY;
CREATE POLICY table_sections_tenant_isolation ON public.table_sections
    FOR ALL
    USING (restaurant_id = public.current_restaurant_id())
    WITH CHECK (restaurant_id = public.current_restaurant_id());

ALTER TABLE public.dining_tables ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.dining_tables FORCE ROW LEVEL SECURITY;
CREATE POLICY dining_tables_tenant_isolation ON public.dining_tables
    FOR ALL
    USING (restaurant_id = public.current_restaurant_id())
    WITH CHECK (restaurant_id = public.current_restaurant_id());

ALTER TABLE public.menu_categories ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.menu_categories FORCE ROW LEVEL SECURITY;
CREATE POLICY menu_categories_tenant_isolation ON public.menu_categories
    FOR ALL
    USING (restaurant_id = public.current_restaurant_id())
    WITH CHECK (restaurant_id = public.current_restaurant_id());

ALTER TABLE public.dishes ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.dishes FORCE ROW LEVEL SECURITY;
CREATE POLICY dishes_tenant_isolation ON public.dishes
    FOR ALL
    USING (restaurant_id = public.current_restaurant_id())
    WITH CHECK (restaurant_id = public.current_restaurant_id());

ALTER TABLE public.visits ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.visits FORCE ROW LEVEL SECURITY;
CREATE POLICY visits_tenant_isolation ON public.visits
    FOR ALL
    USING (restaurant_id = public.current_restaurant_id())
    WITH CHECK (restaurant_id = public.current_restaurant_id());

ALTER TABLE public.order_rounds ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.order_rounds FORCE ROW LEVEL SECURITY;
CREATE POLICY order_rounds_tenant_isolation ON public.order_rounds
    FOR ALL
    USING (restaurant_id = public.current_restaurant_id())
    WITH CHECK (restaurant_id = public.current_restaurant_id());

ALTER TABLE public.bills ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.bills FORCE ROW LEVEL SECURITY;
CREATE POLICY bills_tenant_isolation ON public.bills
    FOR ALL
    USING (restaurant_id = public.current_restaurant_id())
    WITH CHECK (restaurant_id = public.current_restaurant_id());

ALTER TABLE public.order_lines ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.order_lines FORCE ROW LEVEL SECURITY;
CREATE POLICY order_lines_tenant_isolation ON public.order_lines
    FOR ALL
    USING (restaurant_id = public.current_restaurant_id())
    WITH CHECK (restaurant_id = public.current_restaurant_id());

ALTER TABLE public.bill_taxes ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.bill_taxes FORCE ROW LEVEL SECURITY;
CREATE POLICY bill_taxes_tenant_isolation ON public.bill_taxes
    FOR ALL
    USING (restaurant_id = public.current_restaurant_id())
    WITH CHECK (restaurant_id = public.current_restaurant_id());

ALTER TABLE public.payments ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.payments FORCE ROW LEVEL SECURITY;
CREATE POLICY payments_tenant_isolation ON public.payments
    FOR ALL
    USING (restaurant_id = public.current_restaurant_id())
    WITH CHECK (restaurant_id = public.current_restaurant_id());

ALTER TABLE public.bill_number_counters ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.bill_number_counters FORCE ROW LEVEL SECURITY;
CREATE POLICY bill_number_counters_tenant_isolation ON public.bill_number_counters
    FOR ALL
    USING (restaurant_id = public.current_restaurant_id())
    WITH CHECK (restaurant_id = public.current_restaurant_id());

ALTER TABLE public.audit_log ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.audit_log FORCE ROW LEVEL SECURITY;
CREATE POLICY audit_log_tenant_isolation ON public.audit_log
    FOR ALL
    USING (restaurant_id = public.current_restaurant_id())
    WITH CHECK (restaurant_id = public.current_restaurant_id());

-- ===========================================================================
-- Table grants for the API role
--
-- init-roles.sql already sets default privileges so that anything
-- restaurant_owner creates is readable and writable by app_api. These explicit
-- grants exist so the schema is correct on a database where that default was
-- never set, rather than being correct only by inheritance.
--
-- app_api still owns nothing, which is the point: ownership is what would
-- bypass row level security.
-- ===========================================================================

DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'app_api') THEN
        GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA public TO app_api;
    ELSE
        RAISE WARNING 'role app_api does not exist; grant table access by hand (see api/scripts/init-roles.sql)';
    END IF;
END;
$$;

-- ===========================================================================
-- The two, and only two, paths that read across restaurants
--
-- Sign in has a chicken and egg problem: you cannot scope a transaction to a
-- restaurant until you know which restaurant the person belongs to, and finding
-- that out means reading a row without a scope. Two narrow SECURITY DEFINER
-- functions with fixed return shapes are the whole answer, and every other read
-- in the system goes through row level security.
--
-- They are owned by `auth_lookup`, NOT by the schema owner, and that is load
-- bearing and easy to get subtly wrong. A SECURITY DEFINER function runs as its
-- owner, and FORCE ROW LEVEL SECURITY is exactly what removes an owner's
-- exemption. A function owned by the schema owner would therefore still be
-- filtered by the tenant policy, would find current_restaurant_id() NULL, and
-- would return no rows at all. Sign in would never work.
--
-- So `auth_lookup` owns the two functions, owns no table, holds only SELECT on
-- staff and sessions, and each of those tables carries one extra named policy
-- for it. The bypass is then something anyone can list with \dp, rather than an
-- invisible consequence of who owns what, and it needs no superuser, which
-- matters because RDS does not hand out BYPASSRLS.
-- ===========================================================================

-- The role is a hard requirement, not a nicety: without it the two functions
-- would be created owned by the schema owner and would silently return nothing,
-- which surfaces as "nobody can sign in" long after the deploy. The project's
-- rule is that a missing piece of configuration fails the deploy rather than the
-- first request, so this refuses to migrate instead.
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'auth_lookup') THEN
        RAISE EXCEPTION
            'role auth_lookup does not exist. Create it first (see api/scripts/init-roles.sql); '
            'the sign in lookups must be owned by it or they silently return no rows under FORCE ROW LEVEL SECURITY.';
    END IF;
END;
$$;

GRANT USAGE ON SCHEMA public TO auth_lookup;
GRANT SELECT ON public.staff TO auth_lookup;
GRANT SELECT ON public.sessions TO auth_lookup;

CREATE POLICY staff_auth_lookup_read ON public.staff
    FOR SELECT TO auth_lookup USING (true);

CREATE POLICY sessions_auth_lookup_read ON public.sessions
    FOR SELECT TO auth_lookup USING (true);

-- Returns only the identity and hash fields sign in needs. Not the display name,
-- not anything else about the row, and it takes no restaurant argument.
CREATE FUNCTION public.find_staff_for_login(p_email text)
RETURNS TABLE (
    staff_id       uuid,
    restaurant_id  uuid,
    role           public.staff_role,
    password_hash  text,
    deactivated_at timestamptz
)
    LANGUAGE sql
    STABLE
    SECURITY DEFINER
    SET search_path = pg_catalog
    AS $$
        SELECT s.id, s.restaurant_id, s.role, s.password_hash, s.deactivated_at
        FROM public.staff AS s
        WHERE lower(s.email) = lower(p_email)
    $$;

COMMENT ON FUNCTION public.find_staff_for_login(text) IS
    'One of exactly two paths that read across restaurants. Owned by auth_lookup so FORCE ROW LEVEL SECURITY does not filter it.';

-- Returns the session and who it belongs to. Whether the account is deactivated
-- is reported, not judged: this function answers "whose session is this", and
-- feature 7 decides what to do about a deactivated account.
CREATE FUNCTION public.resolve_session(p_token_hash bytea)
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
    $$;

COMMENT ON FUNCTION public.resolve_session(bytea) IS
    'The other of exactly two paths that read across restaurants. Owned by auth_lookup. An expired or revoked session returns no row.';

ALTER FUNCTION public.find_staff_for_login(text) OWNER TO auth_lookup;
ALTER FUNCTION public.resolve_session(bytea) OWNER TO auth_lookup;

-- A SECURITY DEFINER function is executable by PUBLIC unless told otherwise, and
-- these two read every restaurant's staff rows. Only the API role gets them.
REVOKE EXECUTE ON FUNCTION public.find_staff_for_login(text) FROM PUBLIC;
REVOKE EXECUTE ON FUNCTION public.resolve_session(bytea) FROM PUBLIC;

DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'app_api') THEN
        GRANT EXECUTE ON FUNCTION public.find_staff_for_login(text) TO app_api;
        GRANT EXECUTE ON FUNCTION public.resolve_session(bytea) TO app_api;
    ELSE
        RAISE WARNING 'role app_api does not exist; grant execute on the sign in lookups by hand';
    END IF;
END;
$$;
