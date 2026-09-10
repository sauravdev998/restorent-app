//! What happens when two people act at the same moment.
//!
//! These are the only tests in the suite that commit. Two transactions have to
//! be able to see each other's work for any of this to mean anything, and a
//! transaction that never commits is invisible to everybody else. Each one
//! deletes its restaurant at the end, which cascades.
//!
//! The shape is the same throughout: the first transaction takes the lock and
//! holds it, a second transaction is started on another connection and blocks on
//! that lock, the first commits, and then the second is allowed to finish and
//! say what happened to it. That is a real race run in a fixed order, rather
//! than two tasks started at once and hoped about.
//!
//! Covers AC-1, AC-6, AC-8, and AC-13.

mod common;

use api::domain::enums::{LineStatus, RoundStatus};
use api::domain::error::DomainError;
use api::domain::ids::{DishId, RestaurantId, StaffId, VisitId};
use api::domain::service::NewOrderLine;
use api::infrastructure::db::repository::{billing, service};
use api::infrastructure::db::{Database, ScopedTx};

/// Sends one dish and carries it out, so it is ready to go on a bill.
async fn served_line(
    tx: &mut ScopedTx<'_>,
    visit_id: VisitId,
    waiter: StaffId,
    chef: StaffId,
    dish_id: DishId,
) -> api::domain::ids::OrderLineId {
    let (_, lines) = service::send_round(
        tx,
        visit_id,
        waiter,
        &[NewOrderLine {
            dish_id,
            quantity: 1,
            note: None,
        }],
    )
    .await
    .expect("sending the ticket");

    let line_id = lines[0].id;
    service::mark_line_ready(tx, line_id, chef)
        .await
        .expect("the dish comes off the pass");
    service::mark_line_served(tx, line_id)
        .await
        .expect("carrying the dish out");

    line_id
}

/// AC-1: two waiters seating a party at the same table leaves exactly one open
/// visit and one refusal.
#[tokio::test]
async fn two_waiters_racing_for_a_table_leave_exactly_one_party_at_it() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();

    let mut setup = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut setup, restaurant_id).await;
    setup.commit().await.expect("committing the fixture");

    let table = f.table_one;
    let waiter = f.waiter;

    // The first waiter gets there and holds the index lock.
    let mut first = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening the first transaction");
    service::open_visit(&mut first, table, waiter, Some(2))
        .await
        .expect("the first waiter seats a party");

    // The second waiter is now blocked behind that lock.
    let other = database.clone();
    let second = tokio::spawn(async move {
        let mut tx = other
            .begin_scoped(restaurant_id)
            .await
            .expect("opening the second transaction");
        let outcome = service::open_visit(&mut tx, table, waiter, Some(4)).await;
        drop(tx);
        outcome
    });

    first.commit().await.expect("the first waiter's work lands");

    let outcome = second.await.expect("the second waiter's task ran");
    assert!(
        matches!(outcome, Err(DomainError::Conflict(_))),
        "both waiters seated a party at the same table: {outcome:?}"
    );

    let mut check = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let open_visits: i64 = sqlx::query_scalar("SELECT count(*) FROM visits WHERE status = 'open'")
        .fetch_one(check.connection())
        .await
        .expect("counting open visits");
    assert_eq!(
        open_visits, 1,
        "the table ended up with {open_visits} parties"
    );
    drop(check);

    common::drop_restaurant(&database, restaurant_id).await;
}

/// AC-6: two bills closing at once get two different consecutive numbers, and an
/// abandoned close consumes none.
#[tokio::test]
async fn two_bills_closing_at_once_get_consecutive_numbers_and_a_rollback_returns_one() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();

    let mut setup = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut setup, restaurant_id).await;

    // Two parties, two bills, both ready to close.
    let first_visit = service::open_visit(&mut setup, f.table_one, f.waiter, Some(2))
        .await
        .expect("the first party sits down");
    let first_line = served_line(&mut setup, first_visit.id, f.waiter, f.chef, f.soup).await;
    let first_bill = billing::open_bill(&mut setup, first_visit.id, f.waiter)
        .await
        .expect("opening the first bill");
    billing::assign_lines_to_bill(&mut setup, first_bill.id, &[first_line])
        .await
        .expect("assigning the first dish");

    let second_visit = service::open_visit(&mut setup, f.table_two, f.waiter, Some(2))
        .await
        .expect("the second party sits down");
    let second_line = served_line(&mut setup, second_visit.id, f.waiter, f.chef, f.steak).await;
    let second_bill = billing::open_bill(&mut setup, second_visit.id, f.waiter)
        .await
        .expect("opening the second bill");
    billing::assign_lines_to_bill(&mut setup, second_bill.id, &[second_line])
        .await
        .expect("assigning the second dish");

    setup.commit().await.expect("committing the fixture");

    // A close that is abandoned. It takes a number and gives it straight back.
    let mut abandoned = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    billing::close_bill(&mut abandoned, first_bill.id, f.waiter)
        .await
        .expect("closing the first bill");
    abandoned.rollback().await.expect("abandoning the close");

    // Now the real race. The first till holds the counter row.
    let mut till_one = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening the first till");
    let one = billing::close_bill(&mut till_one, first_bill.id, f.waiter)
        .await
        .expect("the first till closes its bill");

    let other = database.clone();
    let waiter = f.waiter;
    let second_bill_id = second_bill.id;
    let till_two = tokio::spawn(async move {
        let mut tx = other
            .begin_scoped(restaurant_id)
            .await
            .expect("opening the second till");
        let closed = billing::close_bill(&mut tx, second_bill_id, waiter).await;
        if closed.is_ok() {
            tx.commit().await.expect("the second till commits");
        }
        closed
    });

    till_one.commit().await.expect("the first till commits");

    let two = till_two
        .await
        .expect("the second till's task ran")
        .expect("the second till closed its bill");

    assert_eq!(
        one.number,
        Some(1),
        "the abandoned close consumed a number: the first real bill got {:?}",
        one.number
    );
    assert_eq!(
        two.number,
        Some(2),
        "two bills closing at once did not get consecutive numbers"
    );
    assert_ne!(one.number, two.number, "two bills got the same number");

    common::drop_restaurant(&database, restaurant_id).await;
}

/// AC-8: when two people act on the same dish, one wins and the other's
/// conditional update changes zero rows.
#[tokio::test]
async fn the_loser_of_a_race_on_one_dish_changes_nothing() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();

    let mut setup = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut setup, restaurant_id).await;
    let visit = service::open_visit(&mut setup, f.table_one, f.waiter, Some(2))
        .await
        .expect("the party sits down");
    let (_, lines) = service::send_round(
        &mut setup,
        visit.id,
        f.waiter,
        &[NewOrderLine {
            dish_id: f.soup,
            quantity: 1,
            note: None,
        }],
    )
    .await
    .expect("sending the ticket");
    let line_id = lines[0].id;
    setup.commit().await.expect("committing the fixture");

    // The waiter cancels the dish and holds its row lock.
    let mut waiter_side = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening the waiter's transaction");
    service::void_line(
        &mut waiter_side,
        line_id,
        f.waiter,
        "guest changed their mind",
    )
    .await
    .expect("the waiter cancels the dish");

    // The chef is marking the same dish ready at the same moment, and blocks.
    let other = database.clone();
    let chef = f.chef;
    let chef_side = tokio::spawn(async move {
        let mut tx = other
            .begin_scoped(restaurant_id)
            .await
            .expect("opening the chef's transaction");
        let outcome = service::mark_line_ready(&mut tx, line_id, chef).await;
        drop(tx);
        outcome
    });

    waiter_side.commit().await.expect("the waiter's work lands");

    let outcome = chef_side.await.expect("the chef's task ran");
    assert!(
        matches!(outcome, Err(DomainError::Conflict(_))),
        "the chef overwrote a cancellation instead of being told it happened: {outcome:?}"
    );

    // And the dish really is cancelled, not ready.
    let mut check = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let line = service::line(&mut check, line_id)
        .await
        .expect("re reading the dish");
    assert_eq!(
        line.status,
        LineStatus::Voided,
        "the losing write changed the dish anyway"
    );
    assert_eq!(line.ready_by_staff_id, None);
    drop(check);

    common::drop_restaurant(&database, restaurant_id).await;
}

/// AC-8 and AC-13: a bill closing against a concurrent reassignment ends up with
/// figures that match exactly the dishes it actually has.
#[tokio::test]
async fn a_close_racing_a_reassignment_still_matches_the_dishes_it_ended_up_with() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();

    let mut setup = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut setup, restaurant_id).await;
    let visit = service::open_visit(&mut setup, f.table_one, f.waiter, Some(4))
        .await
        .expect("the party sits down");

    let soup_line = served_line(&mut setup, visit.id, f.waiter, f.chef, f.soup).await;
    let steak_line = served_line(&mut setup, visit.id, f.waiter, f.chef, f.steak).await;

    let bill = billing::open_bill(&mut setup, visit.id, f.waiter)
        .await
        .expect("opening the bill");
    billing::assign_lines_to_bill(&mut setup, bill.id, &[soup_line])
        .await
        .expect("putting the soup on the bill");
    setup.commit().await.expect("committing the fixture");

    // One waiter adds the steak to the bill, taking its row lock.
    let mut reassigning = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening the reassigning transaction");
    billing::assign_lines_to_bill(&mut reassigning, bill.id, &[steak_line])
        .await
        .expect("adding the steak to the bill");

    // Another is closing the same bill, and blocks on that lock.
    let other = database.clone();
    let waiter = f.waiter;
    let bill_id = bill.id;
    let closing = tokio::spawn(async move {
        let mut tx = other
            .begin_scoped(restaurant_id)
            .await
            .expect("opening the closing transaction");
        let closed = billing::close_bill(&mut tx, bill_id, waiter).await;
        if closed.is_ok() {
            tx.commit().await.expect("the close commits");
        }
        closed
    });

    reassigning.commit().await.expect("the reassignment lands");

    let closed = closing
        .await
        .expect("the closing task ran")
        .expect("the bill closed");

    // Both dishes, because the reassignment got in first: 9.5000 + 24.9950.
    assert_eq!(
        closed.subtotal,
        common::money("34.50"),
        "the closed bill's subtotal does not match the dishes it ended up with"
    );

    // And the stored figures agree with the dishes actually on it.
    let mut check = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let assigned: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM order_lines WHERE bill_id = $1 AND status <> 'voided'",
    )
    .bind(bill_id.as_uuid())
    .fetch_one(check.connection())
    .await
    .expect("counting the dishes on the bill");
    assert_eq!(assigned, 2, "the bill closed with {assigned} dishes on it");

    let taxes = billing::bill_taxes(&mut check, bill_id)
        .await
        .expect("reading the tax breakdown");
    let taxes_sum: rust_decimal::Decimal = taxes.iter().map(|tax| tax.amount).sum();
    assert_eq!(
        closed.total,
        closed.subtotal + closed.service_charge_amount + taxes_sum,
        "the closed bill's total does not add up"
    );
    drop(check);

    common::drop_restaurant(&database, restaurant_id).await;
}

/// A table archived by an admin cannot take a party, even one seated a moment
/// later by a waiter who was already looking at it.
#[tokio::test]
async fn a_table_archived_underneath_a_waiter_stops_taking_parties() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();

    let mut setup = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut setup, restaurant_id).await;
    setup.commit().await.expect("committing the fixture");

    let mut admin_side = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening the admin's transaction");
    api::infrastructure::db::repository::catalog::archive_dining_table(
        &mut admin_side,
        f.table_one,
    )
    .await
    .expect("the admin archives the table");
    admin_side.commit().await.expect("the archive lands");

    let mut waiter_side = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening the waiter's transaction");
    let refused: Result<_, DomainError> =
        service::open_visit(&mut waiter_side, f.table_one, f.waiter, Some(2)).await;
    assert!(
        matches!(refused, Err(DomainError::Invalid(_))),
        "a party was seated at an archived table: {refused:?}"
    );
    drop(waiter_side);

    common::drop_restaurant(&database, restaurant_id).await;
}

/// Nothing here should ever leave a stray `Database` clone holding a
/// transaction open, so this proves the pool is still usable at the end.
#[tokio::test]
async fn the_pool_is_still_usable_after_the_races() {
    let database: Database = common::database().await;
    assert!(
        database.is_reachable().await,
        "the database stopped answering"
    );
}

/// AC-7, AC-8: two chefs marking two different dishes on the same ticket at the
/// same moment still leave the ticket ready.
///
/// A regression test for a bug that reached a real kitchen screen. Both
/// transactions recomputed the ticket from its lines under their own snapshot,
/// each saw the other's dish still queued, and both wrote "queued" back. The
/// ticket then sat for ever reading "cooking" with every dish on it reading
/// "ready", the waiter was never told the food was up, and nothing anywhere
/// reported an error.
///
/// This is not an exotic race. It is two chefs working one ticket, which is
/// what a kitchen with two chefs does all evening.
///
/// The fix is the ticket's own row lock, taken before its dishes are read, so
/// the second recompute waits and then sees the first one's committed line.
#[tokio::test]
async fn two_dishes_on_one_ticket_marked_at_once_still_leave_it_ready() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();

    let mut setup = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut setup, restaurant_id).await;

    let visit = service::open_visit(&mut setup, f.table_one, f.waiter, None)
        .await
        .expect("seating a party");

    let (round, lines) = service::send_round(
        &mut setup,
        visit.id,
        f.waiter,
        &[
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
        ],
    )
    .await
    .expect("sending the ticket");

    setup.commit().await.expect("committing the fixture");

    let (first_line, second_line) = (lines[0].id, lines[1].id);

    // The first chef marks their dish and holds the transaction open, which is
    // what a request still in flight looks like to everybody else.
    let mut first = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening the first chef's transaction");
    service::mark_line_ready(&mut first, first_line, f.chef)
        .await
        .expect("the first dish comes off the pass");

    // The second chef marks the other dish while that is still open. Their
    // recompute blocks on the ticket's lock rather than reading around it.
    let second = tokio::spawn({
        let database = database.clone();
        let chef = f.chef;
        async move {
            let mut tx = database
                .begin_scoped(restaurant_id)
                .await
                .expect("opening the second chef's transaction");
            let outcome = service::mark_line_ready(&mut tx, second_line, chef).await;
            tx.commit().await.expect("committing the second chef's tap");
            outcome
        }
    });

    // Long enough for the second transaction to have reached the lock and
    // stopped there. Nothing is being timed; this is what makes the overlap
    // real rather than hoped for.
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    first
        .commit()
        .await
        .expect("committing the first chef's tap");

    second
        .await
        .expect("the second chef's task finished")
        .expect("the second dish comes off the pass");

    let mut reader = database
        .begin_scoped_snapshot(restaurant_id)
        .await
        .expect("opening a reader");

    let after = service::round(&mut reader, round.id)
        .await
        .expect("reading the ticket");

    assert_eq!(
        after.status,
        RoundStatus::Ready,
        "both dishes are off the pass and the ticket still reads {:?}, so the \
         waiter is never told the food is up",
        after.status
    );
    assert!(
        after.ready_at.is_some(),
        "the ticket reads ready with no time on it"
    );

    reader.rollback().await.expect("ending the read");
    common::drop_restaurant(&database, restaurant_id).await;
}
