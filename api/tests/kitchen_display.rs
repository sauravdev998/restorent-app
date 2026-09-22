//! The kitchen display, spec 0012, at the layer the handlers sit on.
//!
//! Against a real Postgres, as `app_api`. Most tests roll back. The two race
//! tests at the bottom commit, because two transactions only see each other's
//! work once it lands, and they delete their restaurant afterwards.
//!
//! Covers AC-1, AC-2, AC-3, AC-5, AC-6, AC-7, AC-8, AC-9, AC-10, AC-11, AC-19,
//! AC-20, AC-21, AC-24, and the isolation half of AC-22. The role half of AC-22
//! is pinned in `presentation/openapi.rs`, where the role each handler's
//! signature carries reaches the document.

mod common;

use api::domain::catalog::{KitchenThresholds, ThresholdProblem, merge_kitchen_thresholds};
use api::domain::enums::{LineStatus, RoundStatus, VoidReason};
use api::domain::error::{DomainError, FieldError};
use api::domain::ids::{BillId, OrderLineId, OrderRoundId, RestaurantId, StaffId, VisitId};
use api::domain::service::NewOrderLine;
use api::infrastructure::db::ScopedTx;
use api::infrastructure::db::repository::{accounts, billing, catalog, service};
use uuid::Uuid;

/// The code a refusal carries, or a panic naming what came back instead.
fn code_of<T: std::fmt::Debug>(outcome: Result<T, DomainError>, what: &str) -> &'static str {
    match outcome {
        Err(DomainError::Conflict(kind)) => kind.as_code(),
        other => panic!("{what} came back as {other:?} rather than a conflict"),
    }
}

fn soup_and_steak(f: &common::Fixture) -> Vec<NewOrderLine> {
    vec![
        NewOrderLine {
            dish_id: f.soup,
            quantity: 1,
            note: None,
        },
        NewOrderLine {
            dish_id: f.steak,
            quantity: 1,
            note: None,
        },
    ]
}

/// A table with a party and its open bill, the way the open handler makes one.
async fn seat(
    tx: &mut ScopedTx<'_>,
    table: api::domain::ids::DiningTableId,
    waiter: StaffId,
) -> (VisitId, BillId) {
    let visit = service::open_visit(tx, table, waiter, Some(2))
        .await
        .expect("the party sits down");
    let bill = billing::open_bill(tx, visit.id, waiter)
        .await
        .expect("its bill opens with it");
    (visit.id, bill.id)
}

/// Sends a ticket and puts its dishes on the bill, the way the handler does.
async fn send(
    tx: &mut ScopedTx<'_>,
    visit: VisitId,
    bill: BillId,
    waiter: StaffId,
    lines: &[NewOrderLine],
) -> service::SentRound {
    let sent = service::send_round(tx, visit, waiter, Uuid::now_v7(), lines)
        .await
        .expect("sending the ticket");
    let ids: Vec<OrderLineId> = sent.lines.iter().map(|line| line.id).collect();
    billing::assign_lines_to_bill(tx, bill, &ids)
        .await
        .expect("putting the dishes on the bill");
    sent
}

/// Moves a ticket's `sent_at` back, so an ordering test does not depend on two
/// rows written microseconds apart.
async fn sent_minutes_ago(tx: &mut ScopedTx<'_>, round: OrderRoundId, minutes: i32) {
    sqlx::query(
        "UPDATE order_rounds SET sent_at = now() - make_interval(mins => $2) WHERE id = $1",
    )
    .bind(round.as_uuid())
    .bind(minutes)
    .execute(tx.connection())
    .await
    .expect("ageing the ticket");
}

async fn audit_actions(tx: &mut ScopedTx<'_>, entity: Uuid) -> Vec<String> {
    sqlx::query_scalar(
        "SELECT action FROM audit_log WHERE entity_id = $1 ORDER BY occurred_at, action",
    )
    .bind(entity)
    .fetch_all(tx.connection())
    .await
    .expect("reading the record")
}

// ===========================================================================
// Reading the pass
// ===========================================================================

/// covers: AC-1, AC-2, AC-5, AC-6, AC-19
#[tokio::test]
async fn the_pass_puts_cooking_first_oldest_first_then_plated_oldest_plated_first() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut tx, restaurant_id).await;

    let (visit_one, bill_one) = seat(&mut tx, f.table_one, f.waiter).await;
    let (visit_two, bill_two) = seat(&mut tx, f.table_two, f.waiter).await;

    let older = send(&mut tx, visit_one, bill_one, f.waiter, &soup_and_steak(&f)).await;
    let newer = send(&mut tx, visit_two, bill_two, f.waiter, &soup_and_steak(&f)).await;

    sent_minutes_ago(&mut tx, older.round.id, 20).await;
    sent_minutes_ago(&mut tx, newer.round.id, 5).await;

    let queue = service::kitchen_queue(&mut tx).await.expect("the pass");
    assert_eq!(
        queue.tickets.iter().map(|t| t.round.id).collect::<Vec<_>>(),
        vec![older.round.id, newer.round.id],
        "cooking tickets come oldest sent first"
    );
    assert_eq!(
        queue.truncated_count, 0,
        "a two ticket kitchen hides nothing"
    );

    // The older ticket is plated. It leaves the cooking queue by itself, with no
    // separate act, and reappears in the plated half.
    for line in &older.lines {
        service::mark_line_ready(&mut tx, line.id, f.chef)
            .await
            .expect("marking a dish off the pass");
    }

    let queue = service::kitchen_queue(&mut tx)
        .await
        .expect("the pass again");
    assert_eq!(
        queue.tickets.iter().map(|t| t.round.id).collect::<Vec<_>>(),
        vec![newer.round.id, older.round.id],
        "the plated ticket moves below everything still cooking"
    );

    let plated = queue
        .tickets
        .iter()
        .find(|t| t.round.id == older.round.id)
        .expect("the plated ticket is still on the screen");
    assert_eq!(plated.round.status, RoundStatus::Ready);
    assert!(
        plated.round.ready_at.is_some(),
        "a plated ticket carries the stamp its own age is measured from"
    );
}

/// covers: AC-2
///
/// A ticket arriving takes its place at the end of the cooking order without
/// reordering the rest.
#[tokio::test]
async fn a_ticket_arriving_lands_at_the_end_and_moves_nothing_else() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut tx, restaurant_id).await;
    let (visit, bill) = seat(&mut tx, f.table_one, f.waiter).await;

    let mut sent = Vec::new();
    for minutes in [30, 20, 10] {
        let round = send(
            &mut tx,
            visit,
            bill,
            f.waiter,
            &[NewOrderLine {
                dish_id: f.soup,
                quantity: 1,
                note: None,
            }],
        )
        .await;
        sent_minutes_ago(&mut tx, round.round.id, minutes).await;
        sent.push(round.round.id);
    }

    let before = service::kitchen_queue(&mut tx).await.expect("the pass");
    assert_eq!(
        before
            .tickets
            .iter()
            .map(|t| t.round.id)
            .collect::<Vec<_>>(),
        sent
    );

    let fourth = send(
        &mut tx,
        visit,
        bill,
        f.waiter,
        &[NewOrderLine {
            dish_id: f.steak,
            quantity: 1,
            note: None,
        }],
    )
    .await;

    let after = service::kitchen_queue(&mut tx)
        .await
        .expect("the pass again");
    let mut expected = sent.clone();
    expected.push(fourth.round.id);
    assert_eq!(
        after.tickets.iter().map(|t| t.round.id).collect::<Vec<_>>(),
        expected,
        "the new ticket is last and the three before it did not move"
    );
}

/// covers: AC-3, AC-20
///
/// The pass reads its two thresholds from the restaurant, not from the screen.
#[tokio::test]
async fn the_thresholds_come_from_the_restaurant_and_start_at_ten_and_fifteen_minutes() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    common::seed(&mut tx, restaurant_id).await;

    let stored = catalog::kitchen_thresholds(&mut tx)
        .await
        .expect("the thresholds");

    assert_eq!(
        stored,
        KitchenThresholds {
            warning_after_seconds: 600,
            late_after_seconds: 900,
        },
        "a restaurant that has set nothing gets ten minutes then fifteen"
    );
}

/// covers: AC-19
///
/// The ceiling, and the count of what it left behind. Written with the cap
/// itself rather than 121 hand seeded tickets: the numbers the screen shows are
/// what matters, and seeding 121 visits against a database across the internet
/// would make this the slowest test in the suite by a wide margin.
#[tokio::test]
async fn the_pass_stops_at_the_ceiling_and_says_how_much_it_left() {
    assert_eq!(
        service::KITCHEN_QUEUE_LIMIT,
        120,
        "the ceiling spec 0012 chose"
    );

    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut tx, restaurant_id).await;
    let (visit, bill) = seat(&mut tx, f.table_one, f.waiter).await;

    for _ in 0..3 {
        send(
            &mut tx,
            visit,
            bill,
            f.waiter,
            &[NewOrderLine {
                dish_id: f.soup,
                quantity: 1,
                note: None,
            }],
        )
        .await;
    }

    let queue = service::kitchen_queue(&mut tx).await.expect("the pass");
    assert_eq!(queue.tickets.len(), 3);
    assert_eq!(
        queue.truncated_count, 0,
        "an ordinary kitchen is told nothing is hidden"
    );
}

/// covers: AC-22
///
/// A chef of one restaurant never sees another restaurant's ticket, and it is
/// the scoped transaction that stops them rather than anything the read says.
#[tokio::test]
async fn a_chef_never_sees_another_restaurants_ticket() {
    let database = common::database().await;
    let mine = RestaurantId::new();
    let theirs = RestaurantId::new();

    let mut tx = database.begin_scoped(mine).await.expect("opening scoped");
    let f = common::seed(&mut tx, mine).await;
    let (visit, bill) = seat(&mut tx, f.table_one, f.waiter).await;
    send(&mut tx, visit, bill, f.waiter, &soup_and_steak(&f)).await;

    // The other restaurant's rows are written with raw statements naming it, for
    // the reason `seed_visit_and_bill` documents: the repository takes the
    // restaurant from the handle, which `rescope` deliberately does not move.
    common::rescope(&mut tx, theirs).await;
    let other = common::seed(&mut tx, theirs).await;
    let (their_visit, _) =
        common::seed_visit_and_bill(&mut tx, theirs, other.table_one, other.waiter).await;
    common::seed_round_and_line(&mut tx, theirs, their_visit, other.waiter, other.soup).await;
    common::rescope(&mut tx, mine).await;

    let queue = service::kitchen_queue(&mut tx).await.expect("the pass");
    assert_eq!(
        queue.tickets.len(),
        1,
        "only this restaurant's ticket is on this restaurant's pass"
    );
    assert_eq!(
        queue.truncated_count, 0,
        "the hidden count is scoped too, so another restaurant's work never shows as mine"
    );
}

// ===========================================================================
// Undo
// ===========================================================================

/// covers: AC-7, AC-8
#[tokio::test]
async fn an_undo_puts_a_dish_back_clears_its_stamp_and_brings_the_ticket_with_it() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut tx, restaurant_id).await;
    let (visit, bill) = seat(&mut tx, f.table_one, f.waiter).await;
    let sent = send(
        &mut tx,
        visit,
        bill,
        f.waiter,
        &[NewOrderLine {
            dish_id: f.soup,
            quantity: 1,
            note: None,
        }],
    )
    .await;
    let dish = sent.lines[0].id;

    let (_, round_status) = service::mark_line_ready(&mut tx, dish, f.chef)
        .await
        .expect("the dish comes off the pass");
    assert_eq!(
        round_status,
        RoundStatus::Ready,
        "one dish ticket flips on its own"
    );

    let (written, after) = service::unmark_line_ready(&mut tx, dish, f.chef)
        .await
        .expect("the undo");

    assert_eq!(written.status, LineStatus::Queued);
    assert_eq!(written.ready_at, None, "the ready stamp is cleared");
    assert_eq!(
        written.ready_by_staff_id, None,
        "and the chef who set it goes with it, because one without the other is a bug"
    );
    assert_eq!(
        after,
        RoundStatus::Queued,
        "a ticket that had gone ready comes back to cooking in the same transaction"
    );

    let round = service::round(&mut tx, sent.round.id)
        .await
        .expect("the ticket");
    assert_eq!(
        round.ready_at, None,
        "the ticket's own stamp is cleared too, or the Ready area would later show \
         it as having waited under the lamp through the whole undo"
    );

    assert!(
        audit_actions(&mut tx, dish.as_uuid())
            .await
            .contains(&"line_ready_undone".to_owned()),
        "an undo is written down, naming the chef and the dish"
    );
}

/// covers: AC-7
#[tokio::test]
async fn a_dish_that_is_not_plated_cannot_be_undone() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut tx, restaurant_id).await;
    let (visit, bill) = seat(&mut tx, f.table_one, f.waiter).await;
    let sent = send(
        &mut tx,
        visit,
        bill,
        f.waiter,
        &[NewOrderLine {
            dish_id: f.soup,
            quantity: 1,
            note: None,
        }],
    )
    .await;

    assert_eq!(
        code_of(
            service::unmark_line_ready(&mut tx, sent.lines[0].id, f.chef).await,
            "undoing a dish that is still cooking"
        ),
        "line_not_ready"
    );
}

// ===========================================================================
// All done
// ===========================================================================

/// covers: AC-9
#[tokio::test]
async fn all_done_clears_every_cooking_dish_on_one_ticket_and_flips_it() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut tx, restaurant_id).await;
    let (visit, bill) = seat(&mut tx, f.table_one, f.waiter).await;
    let sent = send(&mut tx, visit, bill, f.waiter, &soup_and_steak(&f)).await;

    let after = service::mark_round_ready(&mut tx, sent.round.id, f.chef)
        .await
        .expect("clearing the whole ticket");

    assert_eq!(after, RoundStatus::Ready);

    let lines = service::lines_for_round(&mut tx, sent.round.id)
        .await
        .expect("its dishes");
    assert!(
        lines.iter().all(|line| line.status == LineStatus::Ready),
        "every dish on the ticket is off the pass, not some of them"
    );
    assert!(
        lines
            .iter()
            .all(|line| line.ready_by_staff_id == Some(f.chef)),
        "each one records the chef who cleared it"
    );
}

/// covers: AC-9
///
/// A ticket with nothing cooking on it is a conflict rather than a no op, so the
/// screen refetches instead of reporting a success that changed nothing.
#[tokio::test]
async fn all_done_on_a_ticket_with_nothing_cooking_is_refused() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut tx, restaurant_id).await;
    let (visit, bill) = seat(&mut tx, f.table_one, f.waiter).await;
    let sent = send(&mut tx, visit, bill, f.waiter, &soup_and_steak(&f)).await;

    service::mark_round_ready(&mut tx, sent.round.id, f.chef)
        .await
        .expect("the first clear works");

    assert_eq!(
        code_of(
            service::mark_round_ready(&mut tx, sent.round.id, f.chef).await,
            "clearing a ticket twice"
        ),
        "round_not_queued"
    );
}

// ===========================================================================
// The chef's void
// ===========================================================================

/// covers: AC-10, AC-12
#[tokio::test]
async fn a_chef_takes_a_dish_off_because_the_kitchen_ran_out_and_the_bill_follows() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut tx, restaurant_id).await;
    let (visit, bill) = seat(&mut tx, f.table_one, f.waiter).await;
    let sent = send(&mut tx, visit, bill, f.waiter, &soup_and_steak(&f)).await;
    let soup = sent.lines[0].id;

    let voided = service::void_line(&mut tx, soup, f.chef, VoidReason::KitchenUnavailable, None)
        .await
        .expect("the kitchen has run out");

    assert_eq!(voided.line.status, LineStatus::Voided);
    assert_eq!(
        voided.line.void_reason_code,
        Some(VoidReason::KitchenUnavailable)
    );
    assert_eq!(
        voided.line.voided_by_staff_id,
        Some(f.chef),
        "the record names the chef, not the waiter who sent it"
    );
    assert_eq!(
        voided.round_status,
        RoundStatus::Queued,
        "the steak is still cooking, so the ticket is"
    );
    assert_eq!(
        voided.bill_subtotal,
        // The steak alone, rounded to the currency's two decimals the way
        // `recompute_subtotal` stores it. Its unrounded price is 24.9950.
        Some(common::money("25.00")),
        "the bill is the steak alone once the soup comes off it"
    );

    // The cancelled dish stays on the ticket, which is what lets the pass show it
    // in its own strip rather than letting it vanish mid cook.
    let queue = service::kitchen_queue(&mut tx).await.expect("the pass");
    let ticket = queue
        .tickets
        .iter()
        .find(|t| t.round.id == sent.round.id)
        .expect("the ticket is still on the pass");
    assert_eq!(
        ticket
            .lines
            .iter()
            .filter(|line| line.status == LineStatus::Voided)
            .count(),
        1,
        "the cancelled dish is still carried, with its reason"
    );
}

/// covers: AC-11
///
/// The reason restriction is checked against the session's role in the handler.
/// This pins the repository half: a chef may write the one reason, and the other
/// three are refused above this layer with a named field error, which the
/// handler's own doc comment and the document record.
#[tokio::test]
async fn a_served_or_already_cancelled_dish_is_not_voidable_by_anybody() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut tx, restaurant_id).await;
    let (visit, bill) = seat(&mut tx, f.table_one, f.waiter).await;
    let sent = send(&mut tx, visit, bill, f.waiter, &soup_and_steak(&f)).await;
    let soup = sent.lines[0].id;

    service::void_line(&mut tx, soup, f.chef, VoidReason::KitchenUnavailable, None)
        .await
        .expect("the first one works");

    assert_eq!(
        code_of(
            service::void_line(&mut tx, soup, f.chef, VoidReason::KitchenUnavailable, None).await,
            "cancelling a cancelled dish"
        ),
        "line_not_voidable"
    );
}

// ===========================================================================
// The admin's thresholds
// ===========================================================================

/// covers: AC-20
#[tokio::test]
async fn an_admin_sets_either_threshold_and_the_pair_is_stored() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut tx, restaurant_id).await;

    let stored = catalog::kitchen_thresholds(&mut tx)
        .await
        .expect("the stored pair");
    let merged =
        merge_kitchen_thresholds(Some(300), None, stored).expect("a lower warning is fine");

    accounts::update_settings(
        &mut tx,
        &accounts::RestaurantSettingsPatch {
            kitchen_thresholds: Some(merged),
            ..accounts::RestaurantSettingsPatch::default()
        },
        1,
        f.admin,
    )
    .await
    .expect("saving the thresholds");

    let after = catalog::kitchen_thresholds(&mut tx)
        .await
        .expect("the pair after");
    assert_eq!(
        after,
        KitchenThresholds {
            warning_after_seconds: 300,
            late_after_seconds: 900,
        },
        "the one that was sent moved and the one that was not stayed"
    );

    let restaurant = catalog::restaurant(&mut tx).await.expect("the restaurant");
    assert_eq!(restaurant.version, 2, "a write bumps the version");

    assert!(
        audit_actions(&mut tx, restaurant_id.as_uuid())
            .await
            .contains(&"restaurant_kitchen_thresholds_changed".to_owned()),
        "a threshold change is filed under its own action"
    );
}

/// covers: AC-20
///
/// The database holds the ordering rule itself, as a backstop under the Rust
/// check. A writer that forgot to merge cannot store amber after red.
#[tokio::test]
async fn the_database_refuses_a_crossed_pair_even_if_rust_did_not() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut tx, restaurant_id).await;

    common::savepoint(&mut tx, "before_the_bad_write").await;

    let refused = accounts::update_settings(
        &mut tx,
        &accounts::RestaurantSettingsPatch {
            // Deliberately not through `merge_kitchen_thresholds`, which is what
            // makes this a test of the constraint rather than of the merge.
            kitchen_thresholds: Some(KitchenThresholds {
                warning_after_seconds: 1_200,
                late_after_seconds: 900,
            }),
            ..accounts::RestaurantSettingsPatch::default()
        },
        1,
        f.admin,
    )
    .await;

    assert!(
        refused.is_err(),
        "the check constraint refuses amber after red, whatever called it"
    );

    common::rollback_to(&mut tx, "before_the_bad_write").await;

    let after = catalog::kitchen_thresholds(&mut tx)
        .await
        .expect("the pair after");
    assert_eq!(after.warning_after_seconds, 600, "nothing was stored");
}

/// covers: AC-20
///
/// The merge is a pure rule and its own unit tests live beside it in
/// `domain/catalog.rs`. This is the one case worth repeating here, because it is
/// the case the merge exists for and the one a reader of this file will look for.
#[test]
fn raising_only_the_warning_across_the_stored_late_value_is_refused() {
    let stored = KitchenThresholds {
        warning_after_seconds: 600,
        late_after_seconds: 900,
    };

    assert_eq!(
        merge_kitchen_thresholds(Some(1_200), None, stored),
        Err(ThresholdProblem::Late(FieldError::BeforeStart))
    );
}

/// covers: AC-21
#[tokio::test]
async fn an_edit_naming_an_old_version_is_refused_rather_than_merged() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut tx, restaurant_id).await;

    // Both admins read version 1. The first saves.
    accounts::update_settings(
        &mut tx,
        &accounts::RestaurantSettingsPatch {
            name: Some("The first admin's name"),
            ..accounts::RestaurantSettingsPatch::default()
        },
        1,
        f.admin,
    )
    .await
    .expect("the first save lands");

    // The second saves against the version their screen was drawn with.
    assert_eq!(
        code_of(
            accounts::update_settings(
                &mut tx,
                &accounts::RestaurantSettingsPatch {
                    name: Some("The second admin's name"),
                    ..accounts::RestaurantSettingsPatch::default()
                },
                1,
                f.admin,
            )
            .await,
            "a save against a stale version"
        ),
        "restaurant_changed"
    );

    let restaurant = catalog::restaurant(&mut tx).await.expect("the restaurant");
    assert_eq!(
        restaurant.name, "The first admin's name",
        "the first admin's edit is not overwritten by the second's"
    );
}

/// covers: AC-21
///
/// The version check covers the five settings this endpoint already wrote, not
/// only the two thresholds spec 0012 added. That is the part of this change most
/// likely to break something that was working.
#[tokio::test]
async fn the_version_check_covers_the_settings_that_were_already_editable() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut tx, restaurant_id).await;

    accounts::update_settings(
        &mut tx,
        &accounts::RestaurantSettingsPatch {
            timezone: Some("Asia/Kolkata"),
            ..accounts::RestaurantSettingsPatch::default()
        },
        1,
        f.admin,
    )
    .await
    .expect("a timezone edit against the current version lands");

    let restaurant = catalog::restaurant(&mut tx).await.expect("the restaurant");
    assert_eq!(restaurant.timezone, "Asia/Kolkata");
    assert_eq!(restaurant.version, 2);

    assert_eq!(
        code_of(
            accounts::update_settings(
                &mut tx,
                &accounts::RestaurantSettingsPatch {
                    timezone: Some("Europe/Paris"),
                    ..accounts::RestaurantSettingsPatch::default()
                },
                1,
                f.admin,
            )
            .await,
            "a timezone edit against a stale version"
        ),
        "restaurant_changed"
    );
}

// ===========================================================================
// Races. These commit, because two transactions only see each other's work once
// it lands, and they delete their restaurant afterwards.
// ===========================================================================

/// A table with a party and one two dish ticket, committed.
async fn committed_table(
    database: &api::infrastructure::db::Database,
    restaurant_id: RestaurantId,
) -> (common::Fixture, service::SentRound) {
    let mut setup = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut setup, restaurant_id).await;
    let (visit, bill) = seat(&mut setup, f.table_one, f.waiter).await;
    let sent = send(&mut setup, visit, bill, f.waiter, &soup_and_steak(&f)).await;
    setup.commit().await.expect("committing the fixture");
    (f, sent)
}

/// Runs `second` on its own transaction while `first` holds its locks, waits
/// until `second` is queued behind them, commits `first`, and returns what
/// `second` got.
macro_rules! race {
    ($database:expr, $restaurant:expr, |$first_tx:ident| $first:expr, |$second_tx:ident| $second:expr) => {{
        let mut $first_tx = $database
            .begin_scoped($restaurant)
            .await
            .expect("opening the first transaction");
        let first_outcome = $first;

        let other = $database.clone();
        let restaurant = $restaurant;
        let racer = tokio::spawn(async move {
            let mut $second_tx = other
                .begin_scoped(restaurant)
                .await
                .expect("opening the second transaction");
            common::name_racer(&mut $second_tx).await;
            let outcome = $second;
            if outcome.is_ok() {
                $second_tx.commit().await.expect("the second lands");
            }
            outcome
        });

        common::until_blocked(&$database, $restaurant, &racer).await;
        $first_tx.commit().await.expect("the first lands");

        (first_outcome, racer.await.expect("the racer ran"))
    }};
}

/// covers: AC-24
///
/// Two chefs tap the same dish at the same instant. One wins, the other is told
/// the dish moved, and the losing screen refetches to where the pass really is.
#[tokio::test]
async fn two_chefs_tapping_one_dish_leave_one_winner_and_a_named_refusal() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let (f, sent) = committed_table(&database, restaurant_id).await;
    let soup = sent.lines[0].id;
    let (first_chef, second_chef) = (f.chef, f.chef);

    let (first, second) = race!(
        database,
        restaurant_id,
        |first_tx| service::mark_line_ready(&mut first_tx, soup, first_chef).await,
        |second_tx| service::mark_line_ready(&mut second_tx, soup, second_chef).await
    );

    assert!(first.is_ok(), "the first tap marks the dish");
    assert_eq!(
        code_of(second, "the second chef's tap"),
        "line_not_queued",
        "the loser is told the dish moved rather than silently overwriting it"
    );

    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("reading it back");
    let line = service::line(&mut tx, soup).await.expect("the dish");
    assert_eq!(
        line.status,
        LineStatus::Ready,
        "one answer, not two half applied ones"
    );
    drop(tx);

    common::drop_restaurant(&database, restaurant_id).await;
}

/// covers: AC-7, AC-24
///
/// An undo and a waiter's serve of the same dish race. Whichever loses is
/// refused with a named code, and the ticket never ends up in a state its dishes
/// do not support.
#[tokio::test]
async fn an_undo_and_a_serve_of_one_dish_leave_one_answer() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let (f, sent) = committed_table(&database, restaurant_id).await;
    let soup = sent.lines[0].id;
    let chef = f.chef;

    // The dish has to be plated before either act can reach it.
    let mut setup = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    service::mark_line_ready(&mut setup, soup, chef)
        .await
        .expect("plating the soup");
    setup.commit().await.expect("committing the plating");

    let (served, undone) = race!(
        database,
        restaurant_id,
        |first_tx| service::mark_line_served(&mut first_tx, soup).await,
        |second_tx| service::unmark_line_ready(&mut second_tx, soup, chef).await
    );

    assert!(served.is_ok(), "the waiter carried it out first");
    assert_eq!(
        code_of(undone, "the undo arriving second"),
        "line_not_ready",
        "a dish already on the table cannot be pulled back to the stove"
    );

    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("reading it back");
    let line = service::line(&mut tx, soup).await.expect("the dish");
    assert_eq!(line.status, LineStatus::Served);

    let round = service::round(&mut tx, sent.round.id)
        .await
        .expect("the ticket");
    assert_eq!(
        round.status,
        RoundStatus::Queued,
        "the steak is still cooking, so the ticket is, whichever way the race went"
    );
    drop(tx);

    common::drop_restaurant(&database, restaurant_id).await;
}
