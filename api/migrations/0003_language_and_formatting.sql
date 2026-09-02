-- ===========================================================================
-- 0003: language and formatting settings
--
-- Three additive columns. No new table, no new relationship, and no change to
-- an existing column, so migration 0002 and the tests feature 4 wrote against
-- it are untouched.
--
-- Two separate ideas live here and they are deliberately not one column:
--
--   * `default_language` is what somebody reads.
--   * `formatting_locale` is how a number, a date, and a money amount are
--     written.
--
-- An owner in India reading the interface in English still wants rupees grouped
-- the Indian way on their own revenue figures, and a bill has to look the same
-- to every member of staff whatever language each of them reads. Deriving one
-- from the other makes both of those impossible.
-- ===========================================================================

ALTER TABLE public.restaurants
    ADD COLUMN default_language  text NOT NULL DEFAULT 'en',
    ADD COLUMN formatting_locale text NOT NULL DEFAULT 'en-US';

ALTER TABLE public.staff
    ADD COLUMN language text;

-- Length bounds only, and no check constraint listing the codes.
--
-- The set of allowed languages is owned by the application: it lives in
-- locales/catalogue.json, which Rust compiles in and validates every write
-- against. Pinning the list into the schema instead would mean a migration
-- every time a language is added, which is exactly what spec 0005 AC-4 forbids.
-- Same reasoning, and the same shape, as `restaurants.timezone` in 0002.
--
-- 8 covers a BCP 47 language subtag with a script or a region ('sr-Latn' is 7).
-- 35 is the longest a well formed locale identifier gets in practice.
ALTER TABLE public.restaurants
    ADD CONSTRAINT restaurants_default_language_length
        CHECK (char_length(default_language) BETWEEN 2 AND 8),
    ADD CONSTRAINT restaurants_formatting_locale_length
        CHECK (char_length(formatting_locale) BETWEEN 2 AND 35);

ALTER TABLE public.staff
    ADD CONSTRAINT staff_language_length
        CHECK (language IS NULL OR char_length(language) BETWEEN 2 AND 8);

COMMENT ON COLUMN public.restaurants.default_language IS
    'The kitchen surface language, the language printed documents use, and the fallback for any staff member with no personal setting. A code from locales/catalogue.json, validated in Rust on write.';

COMMENT ON COLUMN public.restaurants.formatting_locale IS
    'Drives money, number, date, and time formatting. Deliberately independent of the interface language. A code from locales/catalogue.json, validated in Rust on write.';

COMMENT ON COLUMN public.staff.language IS
    'Personal interface language override. NULL means use the restaurant default, so a staff member created by feature 10 needs no value. Never consulted on the kitchen surface, which is a shared appliance.';

-- Tenant scoping is inherited, not restated. `staff` already carries the
-- composite (restaurant_id, id) key and the row level security policy migration
-- 0002 put on it, so a language setting can never be read or written across a
-- restaurant boundary. Adding a column to a table changes neither.
