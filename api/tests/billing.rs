//! Bills: what the customer pays, and what the restaurant keeps a record of.
//!
//! Covers AC-5, AC-6, AC-9, AC-13, AC-14, and AC-15.

mod common;

use api::domain::enums::{BillStatus, Diet, PaymentMethod, StaffRole};
use api::domain::error::DomainError;
use api::domain::ids::{BillId, DishId, RestaurantId};
use api::domain::service::NewOrderLine;
use api::infrastructure::db::ScopedTx;
use api::infrastructure::db::repository::{billing, catalog, service};
use rust_decimal::Decimal;

/// Runs a whole meal on one table and leaves the bill open, ready to close.
///
/// Returns the bill and the visit, so a test can carry on from either.
async fn meal_ready_to_close(
    tx: &mut ScopedTx<'_>,
    f: &common::Fixture,
    table: api::domain::ids::DiningTableId,
    dishes: &[DishId],
) -> (BillId, api::domain::ids::VisitId) {
    let visit = service::open_visit(tx, table, f.waiter, Some(2))
        .await
        .expect("the party sits down");

    let lines: Vec<NewOrderLine> = dishes
        .iter()
        .map(|dish_id| NewOrderLine {
            dish_id: *dish_id,
            quantity: 1,
            note: None,
        })
        .collect();

    let (_, sent) = service::send_round(tx, visit.id, f.waiter, &lines)
        .await
        .expect("sending the ticket");

    for line in &sent {
        service::mark_line_ready(tx, line.id, f.chef)
            .await
            .expect("the dish comes off the pass");
        service::mark_line_served(tx, line.id)
            .await
            .expect("carrying the dish out");
    }

    let bill = billing::open_bill(tx, visit.id, f.waiter)
        .await
        .expect("opening the bill");

    let line_ids: Vec<_> = sent.iter().map(|line| line.id).collect();
    billing::assign_lines_to_bill(tx, bill.id, &line_ids)
        .await
        .expect("assigning the dishes");

    (bill.id, visit.id)
}

/// AC-9: every figure on a closed bill is exact, rounded to the restaurant's own
/// currency, and the total has no residue.
///
/// The numbers are chosen so the arithmetic has something to round: soup at
/// 9.5000 plus steak at 24.9950 is 34.4950, which is not a whole number of
/// cents.
#[tokio::test]
async fn a_closed_bills_figures_are_exact_and_add_up() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut tx, restaurant_id).await;

    let (bill_id, visit_id) =
        meal_ready_to_close(&mut tx, &f, f.table_one, &[f.soup, f.steak]).await;

    let closed = billing::close_bill(&mut tx, bill_id, f.waiter)
        .await
        .expect("closing the bill");

    assert_eq!(closed.status, BillStatus::Closed);
    assert_eq!(closed.number, Some(1), "the first bill is not number one");
    assert!(closed.closed_at.is_some());
    assert_eq!(closed.closed_by_staff_id, Some(f.waiter));

    // 9.5000 + 24.9950 = 34.4950, rounded half away from zero to 34.50.
    assert_eq!(closed.subtotal, common::money("34.50"));
    // 12.5% of 34.50 is 4.3125, rounded to 4.31.
    assert_eq!(closed.service_charge_percent, Some(common::money("12.5")));
    assert_eq!(closed.service_charge_amount, common::money("4.31"));
    // 20% of 34.50 is 6.90 exactly.
    assert_eq!(closed.tax_total, common::money("6.90"));
    assert_eq!(closed.total, common::money("45.71"));

    // The identity AC-9 asks for, checked rather than assumed.
    let taxes = billing::bill_taxes(&mut tx, bill_id)
        .await
        .expect("reading the tax breakdown");
    let taxes_sum: Decimal = taxes.iter().map(|tax| tax.amount).sum();

    assert_eq!(taxes.len(), 1);
    assert_eq!(taxes[0].name, "VAT");
    assert_eq!(taxes[0].rate_percent, common::money("20.000"));
    assert_eq!(taxes_sum, closed.tax_total);
    assert_eq!(
        closed.total,
        closed.subtotal + closed.service_charge_amount + taxes_sum,
        "the total does not equal subtotal plus service charge plus taxes"
    );

    assert_eq!(closed.currency.code(), "EUR");
    assert_eq!(closed.currency.decimals(), 2);

    // Paying it and freeing the table.
    let payment = billing::record_payment(
        &mut tx,
        bill_id,
        PaymentMethod::Card,
        closed.total,
        f.waiter,
    )
    .await
    .expect("recording the payment");
    assert_eq!(payment.amount, common::money("45.71"));

    service::close_visit(&mut tx, visit_id)
        .await
        .expect("the party leaves");
}

/// AC-9 again, for a restaurant that does not charge to two decimal places.
///
/// The rounding is per restaurant, not a hardcoded two places, and this is the
/// test that would catch somebody replacing it with one.
#[tokio::test]
async fn a_bill_rounds_to_its_own_restaurants_currency() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");

    // Yen has no decimal places at all.
    let f = common::seed_with(
        &mut tx,
        restaurant_id,
        "JPY",
        0,
        "Asia/Tokyo",
        Some(common::money("12.5")),
    )
    .await;

    let (bill_id, _) = meal_ready_to_close(&mut tx, &f, f.table_one, &[f.soup, f.steak]).await;

    let closed = billing::close_bill(&mut tx, bill_id, f.waiter)
        .await
        .expect("closing the bill");

    assert_eq!(closed.currency.code(), "JPY");
    assert_eq!(closed.currency.decimals(), 0);

    // 34.4950 rounds to 34, then 12.5% of 34 is 4.25 which rounds to 4, and 20%
    // of 34 is 6.80 which rounds to 7.
    assert_eq!(closed.subtotal, common::money("34"));
    assert_eq!(closed.service_charge_amount, common::money("4"));
    assert_eq!(closed.tax_total, common::money("7"));
    assert_eq!(closed.total, common::money("45"));

    // Whole numbers, with nothing hiding behind the decimal point.
    assert_eq!(closed.total.fract(), Decimal::ZERO);
}

/// A restaurant that charges no service charge gets zero, never a null.
#[tokio::test]
async fn no_service_charge_yields_zero_rather_than_nothing() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed_with(&mut tx, restaurant_id, "EUR", 2, "Europe/Berlin", None).await;

    let (bill_id, _) = meal_ready_to_close(&mut tx, &f, f.table_one, &[f.soup]).await;

    let closed = billing::close_bill(&mut tx, bill_id, f.waiter)
        .await
        .expect("closing the bill");

    assert_eq!(closed.service_charge_percent, None);
    assert_eq!(
        closed.service_charge_amount,
        Decimal::ZERO,
        "no service charge produced something other than zero"
    );
    assert_eq!(closed.subtotal, common::money("9.50"));
    assert_eq!(closed.tax_total, common::money("1.90"));
    assert_eq!(closed.total, common::money("11.40"));
}

/// AC-5 and AC-10: a closed bill is unchanged by anything that happens
/// afterwards.
///
/// The menu, the tax rules, and the service charge all move underneath it, and
/// every figure and every name on the bill stays exactly as it was.
#[tokio::test]
async fn a_closed_bill_is_untouched_by_later_edits() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut tx, restaurant_id).await;

    let (bill_id, _) = meal_ready_to_close(&mut tx, &f, f.table_one, &[f.soup, f.steak]).await;
    let before = billing::close_bill(&mut tx, bill_id, f.waiter)
        .await
        .expect("closing the bill");
    let taxes_before = billing::bill_taxes(&mut tx, bill_id)
        .await
        .expect("reading the tax breakdown");
    let lines_before = sqlx::query_scalar::<_, String>(
        "SELECT dish_name FROM order_lines WHERE bill_id = $1 ORDER BY dish_name",
    )
    .bind(bill_id.as_uuid())
    .fetch_all(tx.connection())
    .await
    .expect("reading the dish names on the bill");

    // Now change everything the bill was drawn from.
    catalog::update_dish(
        &mut tx,
        f.soup,
        &catalog::DishEdit {
            category_id: f.category,
            name: "Consomme".to_owned(),
            description: Some("renamed and repriced".to_owned()),
            price: common::money("99.0000"),
            diet: Diet::NonVeg,
            version: 1,
        },
        f.admin,
    )
    .await
    .expect("repricing and renaming the soup");
    catalog::set_dish_availability(&mut tx, f.soup, false, f.admin)
        .await
        .expect("switching the soup off");
    catalog::archive_dish(&mut tx, f.soup, f.admin)
        .await
        .expect("archiving the soup");
    catalog::update_tax_component(&mut tx, f.vat, "Sales tax", common::money("5.000"), f.admin)
        .await
        .expect("editing the tax component");
    catalog::set_service_charge(&mut tx, Some(common::money("30.0")), f.admin)
        .await
        .expect("editing the service charge");

    // And read the bill back.
    let after = billing::bill(&mut tx, bill_id)
        .await
        .expect("re reading the bill");
    let taxes_after = billing::bill_taxes(&mut tx, bill_id)
        .await
        .expect("re reading the tax breakdown");
    let lines_after = sqlx::query_scalar::<_, String>(
        "SELECT dish_name FROM order_lines WHERE bill_id = $1 ORDER BY dish_name",
    )
    .bind(bill_id.as_uuid())
    .fetch_all(tx.connection())
    .await
    .expect("re reading the dish names on the bill");

    assert_eq!(after, before, "a closed bill's figures moved");
    assert_eq!(
        taxes_after, taxes_before,
        "a closed bill's tax breakdown moved"
    );
    assert_eq!(
        lines_after, lines_before,
        "a closed bill's dish names moved: {lines_after:?}"
    );
    assert!(
        lines_after.iter().any(|name| name == "Soup"),
        "the bill lost the name the dish had when it was ordered"
    );
    assert_eq!(taxes_after[0].name, "VAT");
    assert_eq!(taxes_after[0].rate_percent, common::money("20.000"));
}

/// AC-6: a bill with nothing on it cannot close, and therefore consumes no
/// number.
#[tokio::test]
async fn an_empty_bill_cannot_close_and_consumes_no_number() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut tx, restaurant_id).await;

    let visit = service::open_visit(&mut tx, f.table_one, f.waiter, Some(2))
        .await
        .expect("the party sits down");
    let empty = billing::open_bill(&mut tx, visit.id, f.waiter)
        .await
        .expect("opening an empty bill");

    let refused = billing::close_bill(&mut tx, empty.id, f.waiter).await;
    assert!(
        matches!(refused, Err(DomainError::Conflict(_))),
        "an empty bill closed: {refused:?}"
    );

    // The next real bill, at another table, still gets number one, which proves
    // the refusal consumed nothing.
    let (bill_id, _) = meal_ready_to_close(&mut tx, &f, f.table_two, &[f.soup]).await;
    let closed = billing::close_bill(&mut tx, bill_id, f.waiter)
        .await
        .expect("closing a real bill");

    assert_eq!(
        closed.number,
        Some(1),
        "the refused empty bill consumed a number"
    );
}

/// A bill with a dish still on the pass cannot close.
#[tokio::test]
async fn a_bill_cannot_close_while_a_dish_is_still_out() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut tx, restaurant_id).await;

    let visit = service::open_visit(&mut tx, f.table_one, f.waiter, Some(2))
        .await
        .expect("the party sits down");
    let (_, lines) = service::send_round(
        &mut tx,
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

    let bill = billing::open_bill(&mut tx, visit.id, f.waiter)
        .await
        .expect("opening the bill");
    billing::assign_lines_to_bill(&mut tx, bill.id, &[lines[0].id])
        .await
        .expect("assigning the dish");

    let refused = billing::close_bill(&mut tx, bill.id, f.waiter).await;
    assert!(
        matches!(refused, Err(DomainError::Conflict(_))),
        "a bill closed with a dish still on the pass: {refused:?}"
    );

    // Cancelling the dish with a reason is the explicit way out, and then the
    // bill has nothing countable on it, so it still cannot close.
    service::void_line(&mut tx, lines[0].id, f.waiter, "kitchen ran out")
        .await
        .expect("cancelling the dish");

    let still_refused = billing::close_bill(&mut tx, bill.id, f.waiter).await;
    assert!(
        matches!(still_refused, Err(DomainError::Conflict(_))),
        "a bill with only a cancelled dish closed: {still_refused:?}"
    );
}

/// AC-13: a bill's figures match exactly the dishes assigned to it, and moving
/// one between two bills recomputes both.
#[tokio::test]
async fn moving_a_dish_between_bills_recomputes_both_of_them() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut tx, restaurant_id).await;

    let visit = service::open_visit(&mut tx, f.table_one, f.waiter, Some(2))
        .await
        .expect("the party sits down");
    let (_, lines) = service::send_round(
        &mut tx,
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

    let first = billing::open_bill(&mut tx, visit.id, f.waiter)
        .await
        .expect("opening the first bill");
    let second = billing::open_bill(&mut tx, visit.id, f.waiter)
        .await
        .expect("opening the second bill");

    // Everything on the first bill.
    let both: Vec<_> = lines.iter().map(|line| line.id).collect();
    let first_state = billing::assign_lines_to_bill(&mut tx, first.id, &both)
        .await
        .expect("assigning both dishes");
    assert_eq!(first_state.subtotal, common::money("34.50"));

    // Then the steak moves to the second bill, and both ends move.
    let second_state = billing::assign_lines_to_bill(&mut tx, second.id, &[lines[1].id])
        .await
        .expect("moving the steak");
    let first_state = billing::bill(&mut tx, first.id)
        .await
        .expect("re reading the first bill");

    assert_eq!(
        second_state.subtotal,
        common::money("25.00"),
        "the steak's new bill did not pick it up"
    );
    assert_eq!(
        first_state.subtotal,
        common::money("9.50"),
        "the steak's old bill kept charging for it"
    );

    // A cancelled dish counts for nothing on either.
    service::void_line(&mut tx, lines[0].id, f.waiter, "sent back")
        .await
        .expect("cancelling the soup");
    let recomputed = billing::assign_lines_to_bill(&mut tx, first.id, &[lines[0].id])
        .await
        .expect("reassigning the cancelled soup");
    assert_eq!(
        recomputed.subtotal,
        Decimal::ZERO,
        "a cancelled dish was still being charged for"
    );
}

/// A dish cannot be moved off a bill that has already closed.
#[tokio::test]
async fn a_dish_cannot_be_moved_off_a_closed_bill() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut tx, restaurant_id).await;

    let (closed_bill, visit_id) = meal_ready_to_close(&mut tx, &f, f.table_one, &[f.soup]).await;
    billing::close_bill(&mut tx, closed_bill, f.waiter)
        .await
        .expect("closing the bill");

    let line_id: uuid::Uuid = sqlx::query_scalar("SELECT id FROM order_lines WHERE bill_id = $1")
        .bind(closed_bill.as_uuid())
        .fetch_one(tx.connection())
        .await
        .expect("finding the line on the closed bill");

    let another = billing::open_bill(&mut tx, visit_id, f.waiter)
        .await
        .expect("opening another bill");

    let refused = billing::assign_lines_to_bill(
        &mut tx,
        another.id,
        &[api::domain::ids::OrderLineId::from_uuid(line_id)],
    )
    .await;

    assert!(
        matches!(refused, Err(DomainError::Conflict(_))),
        "a dish was moved off a closed bill: {refused:?}"
    );
}

/// A bill has to be closed before it can be paid.
#[tokio::test]
async fn a_bill_has_to_be_closed_before_it_is_paid() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut tx, restaurant_id).await;

    let (bill_id, _) = meal_ready_to_close(&mut tx, &f, f.table_one, &[f.soup]).await;

    let too_early = billing::record_payment(
        &mut tx,
        bill_id,
        PaymentMethod::Cash,
        common::money("11.40"),
        f.waiter,
    )
    .await;
    assert!(
        matches!(too_early, Err(DomainError::Conflict(_))),
        "an open bill was paid: {too_early:?}"
    );

    billing::close_bill(&mut tx, bill_id, f.waiter)
        .await
        .expect("closing the bill");

    let nothing = billing::record_payment(
        &mut tx,
        bill_id,
        PaymentMethod::Cash,
        Decimal::ZERO,
        f.waiter,
    )
    .await;
    assert!(
        matches!(nothing, Err(DomainError::Invalid(_))),
        "a payment of nothing was recorded: {nothing:?}"
    );

    billing::record_payment(
        &mut tx,
        bill_id,
        PaymentMethod::Cash,
        common::money("11.40"),
        f.waiter,
    )
    .await
    .expect("recording the payment");
}

/// AC-15: the local day a bill belongs to comes from the restaurant's own
/// timezone, not the server's.
///
/// Two restaurants twenty five hours apart close a bill at the same instant. If
/// the day came from the server, both would report the same date. Because it
/// comes from each restaurant's own timezone, they never can.
#[tokio::test]
async fn the_local_day_of_a_bill_comes_from_its_restaurants_timezone() {
    let database = common::database().await;

    let far_east = RestaurantId::new();
    let mut tx = database
        .begin_scoped(far_east)
        .await
        .expect("opening scoped");
    // Kiritimati is UTC+14.
    let f = common::seed_with(
        &mut tx,
        far_east,
        "EUR",
        2,
        "Pacific/Kiritimati",
        Some(common::money("12.5")),
    )
    .await;
    let (bill_id, _) = meal_ready_to_close(&mut tx, &f, f.table_one, &[f.soup]).await;
    billing::close_bill(&mut tx, bill_id, f.waiter)
        .await
        .expect("closing the bill");
    let eastern_day = billing::bill_local_day(&mut tx, bill_id)
        .await
        .expect("reading the local day")
        .expect("a closed bill has a local day");
    drop(tx);

    let far_west = RestaurantId::new();
    let mut tx = database
        .begin_scoped(far_west)
        .await
        .expect("opening scoped");
    // Niue is UTC-11, so the two are twenty five hours apart and their local
    // dates can never agree.
    let f = common::seed_with(
        &mut tx,
        far_west,
        "EUR",
        2,
        "Pacific/Niue",
        Some(common::money("12.5")),
    )
    .await;
    let (bill_id, _) = meal_ready_to_close(&mut tx, &f, f.table_one, &[f.soup]).await;
    billing::close_bill(&mut tx, bill_id, f.waiter)
        .await
        .expect("closing the bill");
    let western_day = billing::bill_local_day(&mut tx, bill_id)
        .await
        .expect("reading the local day")
        .expect("a closed bill has a local day");
    drop(tx);

    assert_ne!(
        eastern_day, western_day,
        "two restaurants twenty five hours apart reported the same local day, \
         which means the day is coming from the server rather than from each \
         restaurant's own timezone"
    );
}

/// An open bill has no local day, because it has not closed.
#[tokio::test]
async fn an_open_bill_has_no_local_day_yet() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut tx, restaurant_id).await;

    let (bill_id, _) = meal_ready_to_close(&mut tx, &f, f.table_one, &[f.soup]).await;

    assert_eq!(
        billing::bill_local_day(&mut tx, bill_id)
            .await
            .expect("reading the local day"),
        None,
        "an open bill reported a local day"
    );
}

/// Cancels one dish and closes a bill covering the ticket, so the audit log has
/// one void and one bill close in it.
async fn void_one_dish_and_close_the_bill(tx: &mut ScopedTx<'_>, f: &common::Fixture) {
    let visit = service::open_visit(tx, f.table_one, f.waiter, Some(2))
        .await
        .expect("the party sits down");
    let (_, lines) = service::send_round(
        tx,
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

    service::void_line(tx, lines[0].id, f.waiter, "sent back")
        .await
        .expect("cancelling the soup");

    service::mark_line_ready(tx, lines[1].id, f.chef)
        .await
        .expect("the steak comes off the pass");
    service::mark_line_served(tx, lines[1].id)
        .await
        .expect("carrying the steak out");

    let bill = billing::open_bill(tx, visit.id, f.waiter)
        .await
        .expect("opening the bill");
    let line_ids: Vec<_> = lines.iter().map(|line| line.id).collect();
    billing::assign_lines_to_bill(tx, bill.id, &line_ids)
        .await
        .expect("assigning the dishes");
    billing::close_bill(tx, bill.id, f.waiter)
        .await
        .expect("closing the bill");
}

/// AC-14: every consequential change writes exactly one audit row, with who did
/// it and what the value was before.
#[tokio::test]
async fn every_consequential_change_is_written_down() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut tx, restaurant_id).await;

    // A void, and a bill close.
    void_one_dish_and_close_the_bill(&mut tx, &f).await;

    // A price edit, a tax edit, a service charge edit, a role change, and a
    // deactivation.
    catalog::update_dish(
        &mut tx,
        f.steak,
        &catalog::DishEdit {
            category_id: f.category,
            name: "Steak".to_owned(),
            description: None,
            price: common::money("27.5000"),
            diet: Diet::NonVeg,
            version: 1,
        },
        f.admin,
    )
    .await
    .expect("repricing the steak");
    catalog::update_tax_component(&mut tx, f.vat, "VAT", common::money("21.000"), f.admin)
        .await
        .expect("editing the tax component");
    catalog::set_service_charge(&mut tx, Some(common::money("15.0")), f.admin)
        .await
        .expect("editing the service charge");
    catalog::change_staff_role(&mut tx, f.waiter, StaffRole::Admin, f.admin)
        .await
        .expect("promoting the waiter");
    catalog::deactivate_staff(&mut tx, f.chef, f.admin)
        .await
        .expect("deactivating the chef");

    let recorded: Vec<(String, Option<uuid::Uuid>)> =
        sqlx::query_as("SELECT action, actor_staff_id FROM audit_log ORDER BY occurred_at, action")
            .fetch_all(tx.connection())
            .await
            .expect("reading the audit log");

    let actions: Vec<&str> = recorded.iter().map(|(action, _)| action.as_str()).collect();

    for expected in [
        "line_voided",
        "bill_closed",
        "dish_edited",
        "tax_component_edited",
        "service_charge_edited",
        "staff_role_changed",
        "staff_deactivated",
    ] {
        assert_eq!(
            actions.iter().filter(|action| **action == expected).count(),
            1,
            "expected exactly one {expected} row, got {actions:?}"
        );
    }

    assert!(
        recorded.iter().all(|(_, actor)| actor.is_some()),
        "an audit row was written with nobody responsible for it"
    );

    // And the before and after are both really there for one of them.
    let (before, after): (Option<serde_json::Value>, Option<serde_json::Value>) =
        sqlx::query_as("SELECT before, after FROM audit_log WHERE action = 'staff_role_changed'")
            .fetch_one(tx.connection())
            .await
            .expect("reading the role change row");

    assert_eq!(
        before.and_then(|value| value.get("role").cloned()),
        Some(serde_json::json!("waiter")),
        "the role change did not record what the role was before"
    );
    assert_eq!(
        after.and_then(|value| value.get("role").cloned()),
        Some(serde_json::json!("admin")),
        "the role change did not record what the role became"
    );
}
