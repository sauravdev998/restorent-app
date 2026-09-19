-- ===========================================================================
-- 0009: waiter service flow, per spec 0011
--
-- No new table. Four small things the waiter's working screen needs:
--
--   * every open table has a responsible waiter, who hears the ready chime.
--     Filled from whoever opened it, then made required
--   * every round may carry the key the phone made for that send, so a send
--     retried on bad Wi Fi finds the first round instead of making a second
--     kitchen ticket
--   * a cancelled dish carries one of four reason codes, and its free text
--     becomes optional except for "other"
--   * a dish note and a void text get a length ceiling the database holds
--     itself, beside the one the API checks
--
-- Nothing is deployed yet, so this runs as one step. The existing grants in
-- 0002 cover every added column, and no policy changes.
-- ===========================================================================

-- ---------------------------------------------------------------------------
-- visits: the responsible waiter
-- ---------------------------------------------------------------------------

ALTER TABLE public.visits
    ADD COLUMN responsible_staff_id uuid;

UPDATE public.visits
SET responsible_staff_id = opened_by_staff_id;

ALTER TABLE public.visits
    ALTER COLUMN responsible_staff_id SET NOT NULL,
    ADD CONSTRAINT visits_responsible_fkey
        FOREIGN KEY (responsible_staff_id, restaurant_id)
        REFERENCES public.staff (id, restaurant_id);

-- The Mine filter, and the "whose tables are these" join Orders and Floor make.
CREATE INDEX visits_open_responsible_idx
    ON public.visits (restaurant_id, responsible_staff_id)
    WHERE status = 'open';

-- ---------------------------------------------------------------------------
-- order_rounds: the send key
-- ---------------------------------------------------------------------------

ALTER TABLE public.order_rounds
    ADD COLUMN client_key uuid;

-- The final guard behind the lookup `send_round` makes under the visit lock.
-- Named so `conflict_on` recognises it and answers `client_key_reused`.
CREATE UNIQUE INDEX order_rounds_one_per_client_key
    ON public.order_rounds (restaurant_id, client_key)
    WHERE client_key IS NOT NULL;

-- ---------------------------------------------------------------------------
-- order_lines: void reasons and length ceilings
-- ---------------------------------------------------------------------------

CREATE TYPE public.void_reason AS ENUM (
    'guest_changed_mind',
    'entered_by_mistake',
    'kitchen_unavailable',
    'other'
);

ALTER TABLE public.order_lines
    ADD COLUMN void_reason_code public.void_reason;

-- A line voided before this migration had free text only. It keeps its text
-- and is filed under "other", which is the one code that requires text.
UPDATE public.order_lines
SET void_reason_code = 'other'
WHERE status = 'voided';

-- Both were unlimited before. Only development data exists, so an over long
-- value is cut rather than refused.
UPDATE public.order_lines
SET void_reason = left(void_reason, 200)
WHERE char_length(void_reason) > 200;

UPDATE public.order_lines
SET note = left(note, 140)
WHERE char_length(note) > 140;

-- A blank note is stored as none, so a stored one is never empty.
UPDATE public.order_lines SET note = NULL WHERE note IS NOT NULL AND btrim(note) = '';

ALTER TABLE public.order_lines
    DROP CONSTRAINT order_lines_void_is_explained,
    ADD CONSTRAINT order_lines_void_is_explained
        CHECK (status <> 'voided'
               OR (void_reason_code IS NOT NULL
                   AND voided_by_staff_id IS NOT NULL
                   AND voided_at IS NOT NULL
                   AND (void_reason_code <> 'other' OR void_reason IS NOT NULL))),
    ADD CONSTRAINT order_lines_note_length
        CHECK (note IS NULL OR char_length(note) BETWEEN 1 AND 140),
    ADD CONSTRAINT order_lines_void_reason_length
        CHECK (void_reason IS NULL OR char_length(void_reason) BETWEEN 1 AND 200);

COMMENT ON COLUMN public.visits.responsible_staff_id IS
    'The waiter who hears the ready chime for this table. Whoever opened it, until somebody takes it over.';

COMMENT ON COLUMN public.order_rounds.client_key IS
    'The key the phone made for this send. A second send with the same key returns this round instead of creating another.';

COMMENT ON COLUMN public.order_lines.void_reason_code IS
    'Why the dish was cancelled. Required on a voided line; text in void_reason is required only for other.';
