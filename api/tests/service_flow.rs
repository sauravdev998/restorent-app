//! Seating a party, sending tickets, and moving dishes along.
//!
//! Covers AC-1, AC-7, AC-8, and AC-10.

mod common;

use api::domain::enums::{LineStatus, RoundStatus, VisitStatus};
use api::domain::error::DomainError;
use api::domain::ids::RestaurantId;
use api::domain::service::NewOrderLine;
use api::infrastructure::db::repository::{billing, catalog, service};

/// One dish, the usual way to order it.
fn one(dish_id: api::domain::ids::DishId) -> Vec<NewOrderLine> {
    vec![NewOrderLine {
        dish_id,
        quantity: 1,
        note: None,
    }]
}

/// AC-1: a table holds at most one open visit, and the database is what refuses
/// the second.
#[tokio::test]
async fn a_table_holds_at_most_one_open_visit() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut tx, restaurant_id).await;

    service::open_visit(&mut tx, f.table_one, f.waiter, Some(2))
        .await
        .expect("the first party sits down");

    // The refusal comes from a unique violation, which aborts the transaction it
    // happens in. That costs nothing in production, where the operation is the
    // whole request and the transaction is being rolled back anyway, but a test
    // that carries on afterwards needs a savepoint to rewind to.
    common::savepoint(&mut tx, "second_party").await;
    let second = service::open_visit(&mut tx, f.table_one, f.waiter, Some(4)).await;

    assert!(
        matches!(second, Err(DomainError::Conflict(_))),
        "a second party was seated at an occupied table: {second:?}"
    );
    common::rollback_to(&mut tx, "second_party").await;

    // A different table is fine, which confirms the refusal was about occupancy
    // rather than about opening visits at all.
    service::open_visit(&mut tx, f.table_two, f.waiter, Some(4))
        .await
        .expect("a second party sits at a different table");
}

/// A party that sits down and leaves without ordering frees the table, and the
/// table can then take a new party.
#[tokio::test]
async fn a_visit_with_no_bills_closes_and_frees_its_table() {
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

    let closed = service::close_visit(&mut tx, visit.id)
        .await
        .expect("a party with no bills leaves freely");

    assert_eq!(closed.status, VisitStatus::Closed);
    assert!(
        closed.closed_at.is_some(),
        "a closed visit has no closed_at"
    );

    service::open_visit(&mut tx, f.table_one, f.waiter, Some(3))
        .await
        .expect("the table did not free up");
}

/// AC-7: marking one dish ready leaves every other dish alone, and the ticket
/// flips only when the last one that counts lands.
#[tokio::test]
async fn status_is_per_dish_and_the_ticket_follows_its_dishes() {
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

    let (round, lines) = service::send_round(
        &mut tx,
        visit.id,
        f.waiter,
        &[
            NewOrderLine {
                dish_id: f.soup,
                quantity: 1,
                note: Some("no salt".to_owned()),
            },
            NewOrderLine {
                dish_id: f.steak,
                quantity: 2,
                note: None,
            },
        ],
    )
    .await
    .expect("sending the ticket");

    assert_eq!(round.sequence_no, 1, "the first ticket is not number one");
    assert_eq!(round.status, RoundStatus::Queued);
    assert_eq!(lines.len(), 2);

    let soup = lines[0].clone();
    let steak = lines[1].clone();

    // The note reaches the kitchen unchanged, and the price and name were copied.
    assert_eq!(soup.note.as_deref(), Some("no salt"));
    assert_eq!(soup.dish_name, "Soup");
    assert_eq!(soup.unit_price, common::money("9.5000"));
    assert_eq!(steak.line_total, common::money("49.9900"));

    // One dish ready leaves the other alone and the ticket still queued.
    let (marked, round_status) = service::mark_line_ready(&mut tx, soup.id, f.chef)
        .await
        .expect("the chef marks the soup ready");

    assert_eq!(marked.status, LineStatus::Ready);
    assert_eq!(marked.ready_by_staff_id, Some(f.chef));
    assert_eq!(
        round_status,
        RoundStatus::Queued,
        "the ticket went ready while a dish was still on the pass"
    );

    let untouched = service::line(&mut tx, steak.id)
        .await
        .expect("re reading the steak");
    assert_eq!(
        untouched.status,
        LineStatus::Queued,
        "marking the soup ready moved the steak too"
    );

    // The last dish flips the ticket.
    let (_, round_status) = service::mark_line_ready(&mut tx, steak.id, f.chef)
        .await
        .expect("the chef marks the steak ready");
    assert_eq!(round_status, RoundStatus::Ready);

    let reread = service::round(&mut tx, round.id)
        .await
        .expect("re reading the ticket");
    assert_eq!(reread.status, RoundStatus::Ready);
    assert!(
        reread.ready_at.is_some(),
        "a ready ticket has no ready_at stamped on it"
    );

    // And serving both flips it again.
    service::mark_line_served(&mut tx, soup.id)
        .await
        .expect("carrying the soup out");
    let (_, round_status) = service::mark_line_served(&mut tx, steak.id)
        .await
        .expect("carrying the steak out");
    assert_eq!(round_status, RoundStatus::Served);
}

/// AC-7, the case the ordering of the rule exists for: a ticket whose dishes
/// were every one cancelled is cancelled, never ready.
#[tokio::test]
async fn a_ticket_whose_dishes_are_all_cancelled_never_reads_as_ready() {
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
    let (round, lines) = service::send_round(
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

    service::void_line(&mut tx, lines[0].id, f.waiter, "guest changed their mind")
        .await
        .expect("cancelling the first dish");
    let (voided, round_status) =
        service::void_line(&mut tx, lines[1].id, f.waiter, "kitchen ran out")
            .await
            .expect("cancelling the second dish");

    assert_eq!(voided.status, LineStatus::Voided);
    assert_eq!(voided.void_reason.as_deref(), Some("kitchen ran out"));
    assert_eq!(voided.voided_by_staff_id, Some(f.waiter));
    assert_eq!(
        round_status,
        RoundStatus::Voided,
        "a ticket with every dish cancelled read as {round_status:?} rather than cancelled"
    );

    let reread = service::round(&mut tx, round.id)
        .await
        .expect("re reading the ticket");
    assert_eq!(reread.status, RoundStatus::Voided);
}

/// A cancelled dish does not hold a ticket back from being served.
#[tokio::test]
async fn a_cancelled_dish_does_not_hold_the_rest_of_the_ticket_back() {
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

    service::void_line(&mut tx, lines[1].id, f.waiter, "kitchen ran out")
        .await
        .expect("cancelling the steak");

    service::mark_line_ready(&mut tx, lines[0].id, f.chef)
        .await
        .expect("the soup comes off the pass");
    let (_, round_status) = service::mark_line_served(&mut tx, lines[0].id)
        .await
        .expect("carrying the soup out");

    assert_eq!(
        round_status,
        RoundStatus::Served,
        "a ticket with one served dish and one cancelled one read as {round_status:?}"
    );
}

/// AC-8: a state change from an unexpected state changes nothing and says so.
#[tokio::test]
async fn a_state_change_from_an_unexpected_state_is_refused() {
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
    let (_, lines) = service::send_round(&mut tx, visit.id, f.waiter, &one(f.soup))
        .await
        .expect("sending the ticket");
    let line = lines[0].clone();

    // Serving something that is still on the pass.
    let too_early = service::mark_line_served(&mut tx, line.id).await;
    assert!(
        matches!(too_early, Err(DomainError::Conflict(_))),
        "a queued dish was marked served: {too_early:?}"
    );

    service::mark_line_ready(&mut tx, line.id, f.chef)
        .await
        .expect("the chef marks it ready");

    // The chef marking it ready a second time, which is what the loser of a race
    // does.
    let again = service::mark_line_ready(&mut tx, line.id, f.chef).await;
    assert!(
        matches!(again, Err(DomainError::Conflict(_))),
        "the same dish was marked ready twice: {again:?}"
    );

    service::mark_line_served(&mut tx, line.id)
        .await
        .expect("carrying it out");

    // Cancelling a dish that is already on the table.
    let too_late = service::void_line(&mut tx, line.id, f.waiter, "changed mind").await;
    assert!(
        matches!(too_late, Err(DomainError::Conflict(_))),
        "a served dish was cancelled: {too_late:?}"
    );

    // And a cancellation with no reason is refused before anything is written.
    let (_, more) = service::send_round(&mut tx, visit.id, f.waiter, &one(f.steak))
        .await
        .expect("sending another ticket");
    let unexplained = service::void_line(&mut tx, more[0].id, f.waiter, "   ").await;
    assert!(
        matches!(unexplained, Err(DomainError::Invalid(_))),
        "a dish was cancelled without a reason: {unexplained:?}"
    );
}

/// AC-10: archiving hides a thing from working queries without breaking
/// anything that already referred to it.
#[tokio::test]
async fn archiving_hides_without_breaking_what_referred_to_it() {
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
    let (_, lines) = service::send_round(&mut tx, visit.id, f.waiter, &one(f.soup))
        .await
        .expect("sending the ticket");
    let line = lines[0].clone();

    assert_eq!(
        catalog::live_dishes(&mut tx).await.expect("reading").len(),
        2
    );
    assert_eq!(
        catalog::live_dining_tables(&mut tx)
            .await
            .expect("reading")
            .len(),
        2
    );
    assert_eq!(
        catalog::active_staff(&mut tx).await.expect("reading").len(),
        3
    );

    catalog::archive_dish(&mut tx, f.soup, f.admin)
        .await
        .expect("archiving the soup");
    catalog::archive_dining_table(&mut tx, f.table_two)
        .await
        .expect("archiving a table");
    catalog::archive_menu_category(&mut tx, f.category)
        .await
        .expect("archiving the category");
    catalog::deactivate_staff(&mut tx, f.chef, f.admin)
        .await
        .expect("deactivating the chef");

    // Gone from every working query.
    let dishes = catalog::live_dishes(&mut tx).await.expect("reading dishes");
    assert_eq!(dishes.len(), 1, "the archived dish is still on the menu");
    assert!(dishes.iter().all(|dish| dish.id != f.soup));

    assert_eq!(
        catalog::live_dining_tables(&mut tx)
            .await
            .expect("reading tables")
            .len(),
        1,
        "the archived table is still on the floor plan"
    );
    assert!(
        catalog::live_menu_categories(&mut tx)
            .await
            .expect("reading categories")
            .is_empty(),
        "the archived category is still on the menu"
    );

    let staff = catalog::active_staff(&mut tx).await.expect("reading staff");
    assert_eq!(staff.len(), 2, "the deactivated chef is still listed");
    assert!(staff.iter().all(|member| member.id != f.chef));

    // And the line that referred to the dish still resolves, with the name and
    // price it copied when it was ordered.
    let reread = service::line(&mut tx, line.id)
        .await
        .expect("the order line stopped resolving after its dish was archived");
    assert_eq!(reread.dish_name, "Soup");
    assert_eq!(reread.unit_price, common::money("9.5000"));

    // An archived dish cannot be ordered again.
    let refused = service::send_round(&mut tx, visit.id, f.waiter, &one(f.soup)).await;
    assert!(
        matches!(refused, Err(DomainError::Invalid(_))),
        "an archived dish was ordered: {refused:?}"
    );

    // Nor can an archived table take a party.
    let refused = service::open_visit(&mut tx, f.table_two, f.waiter, Some(2)).await;
    assert!(
        matches!(refused, Err(DomainError::Invalid(_))),
        "an archived table took a party: {refused:?}"
    );
}

/// A dish the kitchen has run out of cannot be ordered, and a dish already sent
/// is unaffected.
#[tokio::test]
async fn an_unavailable_dish_cannot_be_ordered_but_one_already_sent_is_untouched() {
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
    let (_, lines) = service::send_round(&mut tx, visit.id, f.waiter, &one(f.soup))
        .await
        .expect("sending the ticket");

    catalog::update_dish(
        &mut tx,
        f.soup,
        &catalog::DishEdit {
            name: "Soup".to_owned(),
            description: None,
            price: common::money("9.5000"),
            is_available: false,
        },
        f.admin,
    )
    .await
    .expect("the kitchen runs out of soup");

    let refused = service::send_round(&mut tx, visit.id, f.waiter, &one(f.soup)).await;
    assert!(
        matches!(refused, Err(DomainError::Invalid(_))),
        "a dish the kitchen ran out of was ordered anyway: {refused:?}"
    );

    let already_sent = service::line(&mut tx, lines[0].id)
        .await
        .expect("the line already sent stopped resolving");
    assert_eq!(
        already_sent.status,
        LineStatus::Queued,
        "marking a dish unavailable disturbed one already on a ticket"
    );
}

/// A party cannot leave while a bill is open or a dish is on nobody's bill.
#[tokio::test]
async fn a_party_cannot_leave_while_money_is_still_owed() {
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
    let (_, lines) = service::send_round(&mut tx, visit.id, f.waiter, &one(f.soup))
        .await
        .expect("sending the ticket");

    // A dish on nobody's bill.
    let unassigned = service::close_visit(&mut tx, visit.id).await;
    assert!(
        matches!(unassigned, Err(DomainError::Conflict(_))),
        "a party left with a dish on nobody's bill: {unassigned:?}"
    );

    let bill = billing::open_bill(&mut tx, visit.id, f.waiter)
        .await
        .expect("opening a bill");
    billing::assign_lines_to_bill(&mut tx, bill.id, &[lines[0].id])
        .await
        .expect("assigning the dish");

    // Now the dish is assigned but the bill is still open.
    let still_open = service::close_visit(&mut tx, visit.id).await;
    assert!(
        matches!(still_open, Err(DomainError::Conflict(_))),
        "a party left with an open bill: {still_open:?}"
    );

    service::mark_line_ready(&mut tx, lines[0].id, f.chef)
        .await
        .expect("the soup comes off the pass");
    service::mark_line_served(&mut tx, lines[0].id)
        .await
        .expect("carrying it out");
    billing::close_bill(&mut tx, bill.id, f.waiter)
        .await
        .expect("closing the bill");

    service::close_visit(&mut tx, visit.id)
        .await
        .expect("the party still could not leave");
}

/// Moving a party to another table, and being refused an occupied one.
#[tokio::test]
async fn a_party_can_move_tables_but_not_onto_an_occupied_one() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut tx, restaurant_id).await;

    let first = service::open_visit(&mut tx, f.table_one, f.waiter, Some(2))
        .await
        .expect("the first party sits down");

    let moved = service::move_visit(&mut tx, first.id, f.table_two)
        .await
        .expect("moving the party");
    assert_eq!(moved.table_id, f.table_two);

    // Table one is free again, so somebody else takes it.
    let second = service::open_visit(&mut tx, f.table_one, f.waiter, Some(4))
        .await
        .expect("a second party takes the freed table");

    // And now neither can move onto the other.
    let blocked = service::move_visit(&mut tx, second.id, f.table_two).await;
    assert!(
        matches!(blocked, Err(DomainError::Conflict(_))),
        "a party moved onto an occupied table: {blocked:?}"
    );
}

/// Each ticket in a meal gets the next number, and a ticket needs a dish on it.
#[tokio::test]
async fn tickets_are_numbered_in_order_within_a_visit() {
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

    let (starters, _) = service::send_round(&mut tx, visit.id, f.waiter, &one(f.soup))
        .await
        .expect("sending the starters");
    let (mains, _) = service::send_round(&mut tx, visit.id, f.waiter, &one(f.steak))
        .await
        .expect("sending the mains");

    assert_eq!(starters.sequence_no, 1);
    assert_eq!(mains.sequence_no, 2);

    let empty = service::send_round(&mut tx, visit.id, f.waiter, &[]).await;
    assert!(
        matches!(empty, Err(DomainError::Invalid(_))),
        "an empty ticket reached the kitchen: {empty:?}"
    );
}
