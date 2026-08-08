# Verify: core data model · spec 0003 · updated 9 August 2026

_Steps derived from spec 0003 acceptance criteria and its Value sourcing table. `/check verify` runs these; `/test` locks the durable ones._

Everything here runs against a real Postgres, connected as `app_api`. Connecting as
the schema owner would make every isolation step pass while proving nothing, because
Postgres exempts a table's owner from its own policies.

Set up first:

```bash
pnpm db:up && pnpm migrate
```

## Commands

- [x] `pnpm migrate` on a database where the `auth_lookup` role does not exist → refuses with a message naming `api/scripts/init-roles.sql`, rather than migrating and leaving sign in silently broken → AC-12
- [x] `cargo test --manifest-path api/Cargo.toml --test isolation` → 6 pass → AC-2, AC-3, AC-4, AC-11, AC-12
- [x] `cargo test --manifest-path api/Cargo.toml --test service_flow` → 11 pass → AC-1, AC-7, AC-8, AC-10
- [x] `cargo test --manifest-path api/Cargo.toml --test billing` → 12 pass → AC-5, AC-6, AC-9, AC-13, AC-14, AC-15
- [x] `cargo test --manifest-path api/Cargo.toml --test concurrency` → 6 pass → AC-1, AC-6, AC-8, AC-13
- [x] `pnpm check` → green → all
- [x] `pnpm sqlx:check` → no diff, so the committed `.sqlx` cache matches the SQL in the tree → AC-9

## Schema shape, checked against the live database

Run each as `restaurant_owner`, against the migrated database.

- [x] `SELECT count(*) FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname='public' AND c.relkind='r' AND c.relname<>'_sqlx_migrations'` → `16` → AC-1
- [x] `SELECT count(*) FROM pg_type t JOIN pg_namespace n ON n.oid=t.typnamespace WHERE n.nspname='public' AND t.typtype='e'` → `6` → AC-7
- [x] Every tenant table has both `ENABLE` and `FORCE`: `SELECT count(*) FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname='public' AND c.relkind='r' AND c.relname<>'_sqlx_migrations' AND NOT (c.relrowsecurity AND c.relforcerowsecurity)` → `0` → AC-4
- [x] The two sign in functions are owned by `auth_lookup`, not the schema owner: `SELECT proname, pg_get_userbyid(proowner) FROM pg_proc p JOIN pg_namespace n ON n.oid=p.pronamespace WHERE n.nspname='public' AND proname IN ('find_staff_for_login','resolve_session')` → both `auth_lookup` → AC-12
- [x] No money column is anything but `numeric(14,4)`: `SELECT count(*) FROM information_schema.columns WHERE table_schema='public' AND column_name IN ('price','unit_price','line_total','subtotal','service_charge_amount','tax_total','total','amount') AND (data_type<>'numeric' OR numeric_precision<>14 OR numeric_scale<>4)` → `0` → AC-9
- [x] No timestamp column is a bare `timestamp`: `SELECT count(*) FROM information_schema.columns WHERE table_schema='public' AND data_type='timestamp without time zone'` → `0` → AC-15
- [x] Exactly two unscoped read paths exist: `SELECT count(*) FROM pg_proc p JOIN pg_namespace n ON n.oid=p.pronamespace WHERE n.nspname='public' AND p.prosecdef` → `2` → AC-12

## Value sourcing

One step per row of the spec's Value sourcing table. Each varies the input that
breaks if the value is read from the wrong place.

- [x] **Which restaurant a request is scoped to** ← `resolve_session`. Resolve a token, scope a transaction to the id it returns, and confirm the transaction reads only that restaurant's rows → AC-3, AC-12
- [x] **A line's unit price** ← `dishes.price` copied at send time. Send a round, then reprice the dish, then re read the line: `unit_price` is unchanged → AC-5
- [x] **A line's dish name** ← `dishes.name` copied at send time. Send a round, rename the dish, then archive it, then re read the line: `dish_name` is still the old name and the line still resolves → AC-5, AC-10
- [x] **A round's sequence number** ← `max(sequence_no) + 1` under the visit's row lock. Send three rounds on one visit → `1`, `2`, `3`; send two concurrently → two different numbers, never a duplicate → AC-8
- [x] **A round's status after a line write** ← the total function over its lines. Vary the mix: all voided → `voided` (never `ready`); one voided plus one served → `served`; one queued plus one ready → `queued` → AC-7
- [x] **A bill number** ← the `bill_number_counters` upsert. Close two bills → `1`, `2`. Close one in a transaction that rolls back, then close for real → still the next number, no gap. Close an empty bill → refused, consumes nothing → AC-6
- [x] **A bill's starting figures** ← zero, not null. Open a bill and read `subtotal`, `service_charge_amount`, `tax_total`, `total` → all `0`, none null → AC-9
- [ ] **Which figures move before close** ← `subtotal` only. Assign lines to an open bill → `subtotal` moves, the other three stay `0` → AC-9
  - Half observed on 9 August 2026. That the subtotal moves is proved by
    `billing::moving_a_dish_between_bills_recomputes_both_of_them`. That the other
    three stay at zero until close is asserted by nothing, so it stays unticked.
    One for `/test` to lock.
- [x] **A bill's subtotal** ← sum of `line_total` over assigned non voided lines. Void one assigned line → the subtotal drops by exactly that line. Move a line to another bill → both bills recompute → AC-13
- [x] **Each tax name and rate on a bill** ← `tax_components` copied at close. Close a bill, then rename the component and change its rate → the bill's `bill_taxes` row is unchanged → AC-5
- [x] **Each tax amount** ← subtotal times the copied rate, rounded once → AC-9
- [x] **The service charge percent and amount** ← `restaurants.service_charge_percent`. With it set to `12.5` on a subtotal of `34.50` → `4.31`. With it null → percent null and amount `0`, never a null amount → AC-9
- [x] **How rounding is performed** ← half away from zero, not half to even. `0.125` at two decimals → `0.13`, and `0.135` → `0.14`. Both rounding up is the tell; half to even would send the first to `0.12` → AC-9
- [x] **The currency a bill is in** ← `restaurants.currency_code` and `currency_decimals`. Run the same meal in a `EUR`/2 restaurant and a `JPY`/0 one: `34.4950` becomes `34.50` in one and `34` in the other → AC-9
- [x] **A bill's total** ← subtotal plus rounded service charge plus the sum of rounded taxes. `34.50 + 4.31 + 6.90 = 45.71`, with no residue → AC-9
- [x] **The local day a bill belongs to** ← `restaurants.timezone`, never the server's. Close a bill in a `Pacific/Kiritimati` restaurant and one in a `Pacific/Niue` restaurant at the same instant: the two local days differ, at every hour of the day. Equal days mean the value is coming from the server → AC-15
- [x] **The live event payload** ← `notify_entity_change` inside the same scoped transaction. `LISTEN entity_changed`, then send a round → a payload naming the right `restaurant_id` and the entity string `order_round` → AC-7
- [x] **An audit row's actor** ← the staff member acting. Perform a void, a bill close, a dish price edit, a tax edit, a service charge edit, a role change, and a deactivation → exactly seven rows, each with an actor and both a before and an after → AC-14

## Acceptance criteria coverage

- AC-1 one open visit per table · `service_flow::a_table_holds_at_most_one_open_visit`, `concurrency::two_waiters_racing_for_a_table_leave_exactly_one_party_at_it`
- AC-2 no cross restaurant reference · `isolation::a_correctly_scoped_transaction_still_cannot_point_at_another_restaurant`
- AC-3 a scope reads only its own rows · `isolation::a_scoped_transaction_reads_none_of_another_restaurants_rows`
- AC-4 no scope reads nothing · `isolation::an_unscoped_transaction_reads_nothing_at_all`
- AC-5 a closed bill never changes · `billing::a_closed_bill_is_untouched_by_later_edits`
- AC-6 gapless per restaurant numbering · `billing::an_empty_bill_cannot_close_and_consumes_no_number`, `concurrency::two_bills_closing_at_once_get_consecutive_numbers_and_a_rollback_returns_one`
- AC-7 status is per dish · `service_flow::status_is_per_dish_and_the_ticket_follows_its_dishes`, `service_flow::a_ticket_whose_dishes_are_all_cancelled_never_reads_as_ready`, plus the domain unit tests in `domain::service`
- AC-8 the loser of a race changes nothing · `service_flow::a_state_change_from_an_unexpected_state_is_refused`, `concurrency::the_loser_of_a_race_on_one_dish_changes_nothing`
- AC-9 money is exact · `billing::a_closed_bills_figures_are_exact_and_add_up`, `billing::a_bill_rounds_to_its_own_restaurants_currency`, `billing::no_service_charge_yields_zero_rather_than_nothing`, plus `domain::money`
- AC-10 archiving hides without breaking · `service_flow::archiving_hides_without_breaking_what_referred_to_it`
- AC-11 deletion is complete or absent · `isolation::deleting_a_restaurant_is_complete_and_deactivating_one_removes_nothing`
- AC-12 exactly two unscoped read paths · `isolation::the_two_sign_in_lookups_read_across_restaurants_and_nothing_else_does`, `isolation::an_expired_or_revoked_session_resolves_to_nothing`
- AC-13 a bill matches its lines · `billing::moving_a_dish_between_bills_recomputes_both_of_them`, `billing::a_dish_cannot_be_moved_off_a_closed_bill`, `concurrency::a_close_racing_a_reassignment_still_matches_the_dishes_it_ended_up_with`
- AC-14 consequential changes are recorded · `billing::every_consequential_change_is_written_down`
- AC-15 time is unambiguous · `billing::the_local_day_of_a_bill_comes_from_its_restaurants_timezone`, `billing::an_open_bill_has_no_local_day_yet`, plus the schema shape check above

## Not covered here, and why

- **Timezone validation on write.** The spec puts it in Rust on write, and this
  feature ships no path that writes a restaurant: registration is feature 7's.
  There is nothing yet to attach the check to. Feature 7 owns it.
- **Role permissions.** `staff.role` is carried, not enforced. Which role may do
  what is feature 7's decision.
