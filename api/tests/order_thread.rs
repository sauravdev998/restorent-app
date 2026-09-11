//! The thin order thread, at the layer the handlers sit on.
//!
//! These run the four reads and the two composed writes that spec 0007 adds,
//! against a real Postgres, as `app_api`, inside a transaction that rolls back.
//! What they are proving is not that `send_round` works, which spec 0003's
//! tests already cover, but the things this slice put on top of it: that the
//! floor answers what a waiter's screen needs, that the kitchen queue holds the
//! right tickets in the right order, that a conflict now names itself, and that
//! none of it reaches another restaurant's rows.
//!
//! The conflict assertions are the ones worth reading twice. Before this slice
//! every refusal was the word `conflict` and a sentence in English; the point of
//! each `as_code` assertion below is that a waiter's screen can now say what
//! actually happened, in the language that waiter reads.
//!
//! Covers AC-1, AC-2, AC-3, AC-4, AC-6, AC-9, AC-10, AC-12, AC-13, AC-14.

mod common;

use api::domain::enums::{LineStatus, RoundStatus, VisitStatus};
use api::domain::error::DomainError;
use api::domain::ids::{RestaurantId, VisitId};
use api::domain::service::NewOrderLine;
use api::infrastructure::db::repository::{billing, catalog, service};

/// The code a refusal carries, or a panic naming what came back instead.
fn code_of<T>(outcome: Result<T, DomainError>, what: &str) -> &'static str {
    match outcome {
        Err(DomainError::Conflict(kind)) => kind.as_code(),
        Err(other) => panic!("{what} was refused as {other:?} rather than a conflict"),
        Ok(_) => panic!("{what} was allowed when it should have been refused"),
    }
}

/// AC-1: the floor names every live table, and an occupied one carries the
/// visit, who opened it, and when.
#[tokio::test]
async fn the_floor_shows_every_table_and_who_is_sitting_at_the_occupied_ones() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut tx, restaurant_id).await;

    let before = service::floor(&mut tx).await.expect("reading the floor");

    assert_eq!(
        before.len(),
        2,
        "the fixture's two tables are not both here"
    );
    assert!(
        before.iter().all(|table| table.occupancy.is_none()),
        "a table read as occupied before anybody sat down"
    );

    let visit = service::open_visit(&mut tx, f.table_one, f.waiter, Some(4))
        .await
        .expect("seating a party");

    let after = service::floor(&mut tx)
        .await
        .expect("reading the floor again");

    let taken = after
        .iter()
        .find(|table| table.table.id == f.table_one)
        .expect("the table that was opened is still on the floor");

    let occupancy = taken
        .occupancy
        .as_ref()
        .expect("the opened table reads as free, so no waiter can see it is taken");

    assert_eq!(occupancy.visit_id, visit.id);
    assert_eq!(occupancy.guest_count, Some(4));
    assert_eq!(
        occupancy.opened_by, "Wes Waiter",
        "the floor does not name who opened the table, which is what a shift \
         handover reads"
    );
    assert!(
        !occupancy.food_ready,
        "a table with nothing ordered on it says food is ready"
    );

    let still_free = after
        .iter()
        .find(|table| table.table.id == f.table_two)
        .expect("the other table is still on the floor");
    assert!(
        still_free.occupancy.is_none(),
        "opening one table marked another one occupied"
    );
}

/// AC-1: the floor is ordered by section, then by table position, so a waiter
/// reads it in the order they walk it.
#[tokio::test]
async fn the_floor_comes_back_in_the_order_the_room_is_walked() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut tx, restaurant_id).await;

    let floor = service::floor(&mut tx).await.expect("reading the floor");
    let labels: Vec<&str> = floor
        .iter()
        .map(|table| table.table.label.as_str())
        .collect();

    assert_eq!(
        labels,
        vec!["T1", "T2"],
        "the floor came back in some other order than section then position"
    );
    assert_eq!(floor[0].table.id, f.table_one);
}

/// AC-1: the floor says when a table has food waiting, which is what puts it in
/// front of a waiter who is not watching for the alert.
#[tokio::test]
async fn a_table_with_food_on_the_pass_says_so_on_the_floor() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut tx, restaurant_id).await;

    let visit = service::open_visit(&mut tx, f.table_one, f.waiter, None)
        .await
        .expect("seating a party");

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

    let queued = service::floor(&mut tx).await.expect("reading the floor");
    assert!(
        !queued[0]
            .occupancy
            .as_ref()
            .expect("the table is occupied")
            .food_ready,
        "a ticket still on the pass reads as ready to collect"
    );

    service::mark_line_ready(&mut tx, lines[0].id, f.chef)
        .await
        .expect("the dish comes off the pass");

    let ready = service::floor(&mut tx)
        .await
        .expect("reading the floor again");
    assert!(
        ready[0]
            .occupancy
            .as_ref()
            .expect("the table is occupied")
            .food_ready,
        "food came off the pass and the floor did not say so"
    );
}

/// AC-3: an unavailable dish stays on the menu, marked, and an archived one
/// does not. A waiter who can see the kitchen has run out can tell the customer.
#[tokio::test]
async fn the_menu_keeps_an_unavailable_dish_and_drops_an_archived_one() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut tx, restaurant_id).await;

    catalog::set_dish_availability(&mut tx, f.soup, false, f.chef)
        .await
        .expect("switching the soup off");

    let menu = catalog::live_dishes(&mut tx)
        .await
        .expect("reading the menu");

    let soup = menu
        .iter()
        .find(|dish| dish.id == f.soup)
        .expect("an unavailable dish vanished from the menu, so no waiter can tell a customer");
    assert!(!soup.is_available);

    catalog::archive_dish(&mut tx, f.steak, f.admin)
        .await
        .expect("archiving the steak");

    let after = catalog::live_dishes(&mut tx)
        .await
        .expect("reading the menu again");
    assert!(
        !after.iter().any(|dish| dish.id == f.steak),
        "an archived dish is still on the menu"
    );
}

/// AC-4: sending puts every line on the visit's open bill in the same
/// transaction, so the running subtotal is true from the moment the ticket is
/// sent rather than at the end of the meal.
#[tokio::test]
async fn sending_a_ticket_puts_its_dishes_on_the_bill_at_once() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut tx, restaurant_id).await;

    let visit = service::open_visit(&mut tx, f.table_one, f.waiter, None)
        .await
        .expect("seating a party");
    let bill = billing::open_bill(&mut tx, visit.id, f.waiter)
        .await
        .expect("opening the bill");

    let found = service::open_bill_of(&mut tx, visit.id)
        .await
        .expect("looking for the visit's open bill");
    assert_eq!(
        found,
        Some(bill.id),
        "the handler could not find the bill it just opened"
    );

    let (_, lines) = service::send_round(
        &mut tx,
        visit.id,
        f.waiter,
        &[
            NewOrderLine {
                dish_id: f.soup,
                quantity: 2,
                note: Some("no cream".to_owned()),
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

    let line_ids: Vec<_> = lines.iter().map(|line| line.id).collect();
    let updated = billing::assign_lines_to_bill(&mut tx, bill.id, &line_ids)
        .await
        .expect("putting the dishes on the bill");

    // Two soups at 9.5000 and one steak at 24.9950, rounded to the euro's two
    // places by the repository, never by anything in the presentation layer.
    assert_eq!(
        updated.subtotal,
        common::money("44.00"),
        "the running subtotal is not what the dishes come to"
    );
    assert_eq!(
        lines[0].note.as_deref(),
        Some("no cream"),
        "the guest's note did not survive the send"
    );
    // Re read, because the lines the send returned were built before they were
    // put on the bill. What matters is where they are once the transaction the
    // handler runs both writes in has finished.
    let assigned = service::lines_for_round(&mut tx, lines[0].round_id)
        .await
        .expect("re reading the ticket's dishes");

    assert!(
        assigned.iter().all(|line| line.bill_id == Some(bill.id)),
        "a dish reached the kitchen on no bill at all"
    );
}

/// AC-6: the kitchen queue holds exactly the tickets with work left on them,
/// oldest first, each carrying the table it is going to.
#[tokio::test]
async fn the_kitchen_queue_is_the_work_left_oldest_first() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut tx, restaurant_id).await;

    let visit = service::open_visit(&mut tx, f.table_one, f.waiter, None)
        .await
        .expect("seating a party");

    let (first, first_lines) = service::send_round(
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
    .expect("sending the starters");

    let (second, _) = service::send_round(
        &mut tx,
        visit.id,
        f.waiter,
        &[NewOrderLine {
            dish_id: f.steak,
            quantity: 1,
            note: None,
        }],
    )
    .await
    .expect("sending the mains");

    let queue = service::kitchen_queue(&mut tx)
        .await
        .expect("reading the pass");

    assert_eq!(queue.len(), 2);
    assert_eq!(
        queue[0].round.id, first.id,
        "the pass is not in the order the tickets arrived"
    );
    assert_eq!(queue[1].round.id, second.id);
    assert_eq!(
        queue[0].table_label, "T1",
        "a ticket reached the pass without saying where the food is going"
    );
    assert_eq!(queue[0].lines.len(), 1);

    // Carry the first one out. It has left the pass and must not be on a screen
    // a chef is cooking from.
    service::mark_line_ready(&mut tx, first_lines[0].id, f.chef)
        .await
        .expect("the dish comes off the pass");
    service::mark_line_served(&mut tx, first_lines[0].id)
        .await
        .expect("carrying it out");

    let left = service::kitchen_queue(&mut tx)
        .await
        .expect("reading the pass again");

    assert_eq!(
        left.len(),
        1,
        "a served ticket is still on the kitchen screen"
    );
    assert_eq!(left[0].round.id, second.id);
}

/// AC-7: the last dish makes the whole ticket ready, and nothing set that.
#[tokio::test]
async fn the_last_dish_off_the_pass_makes_the_whole_ticket_ready() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut tx, restaurant_id).await;

    let visit = service::open_visit(&mut tx, f.table_one, f.waiter, None)
        .await
        .expect("seating a party");

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

    let (_, after_first) = service::mark_line_ready(&mut tx, lines[0].id, f.chef)
        .await
        .expect("the first dish comes off");
    assert_eq!(
        after_first,
        RoundStatus::Queued,
        "one dish of two made the whole ticket ready, so a waiter would collect \
         half an order"
    );

    let (_, after_last) = service::mark_line_ready(&mut tx, lines[1].id, f.chef)
        .await
        .expect("the last dish comes off");
    assert_eq!(after_last, RoundStatus::Ready);
}

/// AC-10, AC-11: the visit reads back as the whole meal, and closing writes the
/// number and every figure while freeing the table.
#[tokio::test]
async fn a_visit_reads_back_as_the_whole_meal_and_closes_with_a_number() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut tx, restaurant_id).await;

    let visit = service::open_visit(&mut tx, f.table_one, f.waiter, Some(2))
        .await
        .expect("seating a party");
    let bill = billing::open_bill(&mut tx, visit.id, f.waiter)
        .await
        .expect("opening the bill");

    let (round, lines) = service::send_round(
        &mut tx,
        visit.id,
        f.waiter,
        &[NewOrderLine {
            dish_id: f.soup,
            quantity: 2,
            note: None,
        }],
    )
    .await
    .expect("sending the ticket");

    let line_ids: Vec<_> = lines.iter().map(|line| line.id).collect();
    billing::assign_lines_to_bill(&mut tx, bill.id, &line_ids)
        .await
        .expect("putting the dishes on the bill");

    let read = service::rounds_for_visit(&mut tx, visit.id)
        .await
        .expect("reading the visit's tickets");
    assert_eq!(read.len(), 1);
    assert_eq!(read[0].0.id, round.id);
    assert_eq!(read[0].1.len(), 1);

    assert_eq!(
        service::table_label(&mut tx, visit.table_id)
            .await
            .expect("reading the table's label"),
        "T1"
    );

    // Closing now is refused, because the food is still on the pass. This is
    // the refusal a waiter reads most often, and it names itself.
    assert_eq!(
        code_of(
            billing::close_bill(&mut tx, bill.id, f.waiter).await,
            "closing with a dish still out"
        ),
        "bill_has_unserved_lines"
    );

    common::savepoint(&mut tx, "before_close").await;
    common::rollback_to(&mut tx, "before_close").await;

    service::mark_line_ready(&mut tx, lines[0].id, f.chef)
        .await
        .expect("the dish comes off the pass");
    service::mark_line_served(&mut tx, lines[0].id)
        .await
        .expect("carrying it out");

    let closed = billing::close_bill(&mut tx, bill.id, f.waiter)
        .await
        .expect("closing the bill");

    assert!(closed.number.is_some(), "a closed bill has no number");
    assert_eq!(closed.subtotal, common::money("19.00"));
    // Twelve and a half percent of nineteen euros, rounded to the euro.
    assert_eq!(closed.service_charge_amount, common::money("2.38"));
    // Twenty percent value added tax, from the fixture's one component.
    assert_eq!(closed.tax_total, common::money("3.80"));
    assert_eq!(
        closed.total,
        common::money("25.18"),
        "the total is not the sum of the figures printed above it, so the \
         arithmetic on the receipt does not come out"
    );

    let taxes = billing::bill_taxes(&mut tx, bill.id)
        .await
        .expect("reading the tax breakdown");
    assert_eq!(taxes.len(), 1);
    assert_eq!(taxes[0].name, "VAT");

    service::close_visit(&mut tx, visit.id)
        .await
        .expect("the party leaves");

    let floor = service::floor(&mut tx).await.expect("reading the floor");
    assert!(
        floor
            .iter()
            .find(|table| table.table.id == f.table_one)
            .expect("the table is still there")
            .occupancy
            .is_none(),
        "the party left and the table still reads as occupied"
    );

    // And it can take the next party straight away.
    service::open_visit(&mut tx, f.table_one, f.waiter, None)
        .await
        .expect("the next party sits down");
}

/// AC-13: every refusal the service half of the thread produces names itself,
/// so the sentence a waiter or a chef reads is chosen on the web side and
/// translated rather than rendered from the API's English.
#[tokio::test]
async fn every_service_refusal_names_what_happened() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut tx, restaurant_id).await;

    let visit = service::open_visit(&mut tx, f.table_one, f.waiter, None)
        .await
        .expect("seating a party");

    // Two waiters, one table.
    common::savepoint(&mut tx, "occupied").await;
    assert_eq!(
        code_of(
            service::open_visit(&mut tx, f.table_one, f.waiter, None).await,
            "opening an occupied table"
        ),
        "table_occupied"
    );
    common::rollback_to(&mut tx, "occupied").await;

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

    // A dish carried out before it came off the pass.
    common::savepoint(&mut tx, "not_ready").await;
    assert_eq!(
        code_of(
            service::mark_line_served(&mut tx, lines[0].id).await,
            "carrying out a dish that is still cooking"
        ),
        "line_not_ready"
    );
    common::rollback_to(&mut tx, "not_ready").await;

    // A second chef reaching for the same dish. The two line codes are the
    // whole reason the conflict carries the status it expected: this one and
    // the one above come from the same helper and must not read alike.
    service::mark_line_ready(&mut tx, lines[0].id, f.chef)
        .await
        .expect("the dish comes off the pass");

    common::savepoint(&mut tx, "twice").await;
    assert_eq!(
        code_of(
            service::mark_line_ready(&mut tx, lines[0].id, f.chef).await,
            "marking the same dish twice"
        ),
        "line_not_queued"
    );
    common::rollback_to(&mut tx, "twice").await;

    // A party that has already left. The bill has to exist and hold the dish
    // first, because a visit refuses to close while a dish on it is on no bill,
    // which is exactly what the send handler makes impossible.
    service::mark_line_served(&mut tx, lines[0].id)
        .await
        .expect("carrying it out");

    let bill = billing::open_bill(&mut tx, visit.id, f.waiter)
        .await
        .expect("opening the bill");
    billing::assign_lines_to_bill(&mut tx, bill.id, &[lines[0].id])
        .await
        .expect("putting the dish on the bill");
    billing::close_bill(&mut tx, bill.id, f.waiter)
        .await
        .expect("closing the bill");

    service::close_visit(&mut tx, visit.id)
        .await
        .expect("the party leaves");

    assert_eq!(
        code_of(
            service::send_round(
                &mut tx,
                visit.id,
                f.waiter,
                &[NewOrderLine {
                    dish_id: f.soup,
                    quantity: 1,
                    note: None,
                }],
            )
            .await,
            "ordering after the party left"
        ),
        "visit_not_open"
    );
}

/// AC-12, AC-13: the same, for the refusals that end a meal. These are the ones
/// a waiter meets while a customer is standing at the till, so the sentence has
/// to say which of the three it was.
#[tokio::test]
async fn every_closing_refusal_names_what_happened() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut tx, restaurant_id).await;

    let visit = service::open_visit(&mut tx, f.table_one, f.waiter, None)
        .await
        .expect("seating a party");
    let bill = billing::open_bill(&mut tx, visit.id, f.waiter)
        .await
        .expect("opening the bill");

    // Nothing ordered at all.
    common::savepoint(&mut tx, "empty").await;
    assert_eq!(
        code_of(
            billing::close_bill(&mut tx, bill.id, f.waiter).await,
            "closing an empty bill"
        ),
        "bill_has_no_lines"
    );
    common::rollback_to(&mut tx, "empty").await;

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

    billing::assign_lines_to_bill(&mut tx, bill.id, &[lines[0].id])
        .await
        .expect("putting the dish on the bill");

    // The one a waiter meets most: food still on the pass.
    common::savepoint(&mut tx, "still_out").await;
    assert_eq!(
        code_of(
            billing::close_bill(&mut tx, bill.id, f.waiter).await,
            "closing with a dish still out"
        ),
        "bill_has_unserved_lines"
    );
    common::rollback_to(&mut tx, "still_out").await;

    service::mark_line_ready(&mut tx, lines[0].id, f.chef)
        .await
        .expect("the dish comes off the pass");
    service::mark_line_served(&mut tx, lines[0].id)
        .await
        .expect("carrying it out");
    billing::close_bill(&mut tx, bill.id, f.waiter)
        .await
        .expect("closing the bill");

    // Two tills closing the same bill.
    assert_eq!(
        code_of(
            billing::close_bill(&mut tx, bill.id, f.waiter).await,
            "closing a bill twice"
        ),
        "bill_already_closed"
    );
}

/// AC-14: none of this slice's reads crosses a restaurant boundary. A waiter of
/// one restaurant sees an empty floor and an empty pass in another's rows, and
/// gets `NotFound` rather than `Forbidden` for a visit that is not theirs, so
/// the two cannot be told apart.
#[tokio::test]
async fn none_of_the_new_reads_can_see_another_restaurants_service() {
    let database = common::database().await;
    let alpha = RestaurantId::new();
    let beta = RestaurantId::new();

    let mut tx = database.begin_scoped(alpha).await.expect("opening scoped");
    common::seed(&mut tx, alpha).await;

    common::rescope(&mut tx, beta).await;
    let beta_fixture = common::seed(&mut tx, beta).await;

    // Written with raw statements naming beta explicitly, not through the
    // repository. `rescope` moves the policy's idea of which restaurant this
    // transaction is, and deliberately does not move the handle's, so a
    // repository call here would name alpha while the policy checked beta.
    let (beta_visit, _) =
        common::seed_visit_and_bill(&mut tx, beta, beta_fixture.table_one, beta_fixture.waiter)
            .await;

    common::seed_round_and_line(
        &mut tx,
        beta,
        beta_visit,
        beta_fixture.waiter,
        beta_fixture.soup,
    )
    .await;

    let beta_visit = VisitId::from_uuid(beta_visit);

    common::rescope(&mut tx, alpha).await;

    let floor = service::floor(&mut tx)
        .await
        .expect("alpha reads its floor");
    assert_eq!(
        floor.len(),
        2,
        "alpha's floor shows a table count that is not its own two"
    );
    assert!(
        floor.iter().all(|table| table.occupancy.is_none()),
        "alpha sees a party sitting at one of beta's tables"
    );

    let pass = service::kitchen_queue(&mut tx)
        .await
        .expect("alpha reads its pass");
    assert!(
        pass.is_empty(),
        "alpha's kitchen screen is showing beta's tickets"
    );

    assert!(
        matches!(
            service::visit(&mut tx, beta_visit).await,
            Err(DomainError::NotFound)
        ),
        "reading another restaurant's visit answered something other than \
         not found, so a caller can tell the two apart"
    );

    assert!(
        matches!(
            service::rounds_for_visit(&mut tx, beta_visit).await,
            Ok(rounds) if rounds.is_empty()
        ),
        "alpha read the tickets on beta's visit"
    );

    assert!(
        matches!(service::open_bill_of(&mut tx, beta_visit).await, Ok(None)),
        "alpha found a bill on beta's visit"
    );
}

/// AC-9: carrying a whole ticket out moves every dish on it that was waiting,
/// and the ticket leaves the pass.
#[tokio::test]
async fn carrying_a_ticket_out_moves_every_dish_that_was_waiting() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut tx, restaurant_id).await;

    let visit = service::open_visit(&mut tx, f.table_one, f.waiter, None)
        .await
        .expect("seating a party");

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

    for line in &lines {
        service::mark_line_ready(&mut tx, line.id, f.chef)
            .await
            .expect("the dish comes off the pass");
    }

    // What the serve handler does: every line that is waiting, one at a time.
    for line in service::lines_for_round(&mut tx, round.id)
        .await
        .expect("reading the ticket's dishes")
    {
        if line.status == LineStatus::Ready {
            service::mark_line_served(&mut tx, line.id)
                .await
                .expect("carrying the dish out");
        }
    }

    let after = service::round(&mut tx, round.id)
        .await
        .expect("reading the ticket");
    assert_eq!(after.status, RoundStatus::Served);

    assert!(
        service::kitchen_queue(&mut tx)
            .await
            .expect("reading the pass")
            .is_empty(),
        "a ticket that reached the table is still on the kitchen screen"
    );

    let visit_after = service::visit(&mut tx, visit.id)
        .await
        .expect("reading the visit");
    assert_eq!(
        visit_after.status,
        VisitStatus::Open,
        "carrying the food out closed the visit"
    );
}

/// AC-8: a document read sees one moment in time, so a ticket and its dishes
/// can never disagree.
///
/// This is a regression test for a bug that reached a real screen. Under
/// Postgres's default `READ COMMITTED`, every statement takes its own fresh
/// snapshot, so a document built out of a dozen statements can carry half of
/// somebody else's commit. What a waiter saw was a ticket reading "cooking"
/// with every dish on it reading "ready", which is a state that never existed
/// in the database and which meant the ready alert never fired: the screen was
/// waiting for a round status it had already been shown the wrong value of.
///
/// The window is milliseconds against a database on the same network and much
/// wider against one across the internet, which is exactly the shape of bug
/// that passes in development and appears on a busy Friday.
#[tokio::test]
async fn a_document_read_never_shows_a_ticket_disagreeing_with_its_dishes() {
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
        &[NewOrderLine {
            dish_id: f.soup,
            quantity: 1,
            note: None,
        }],
    )
    .await
    .expect("sending the ticket");

    // Committed, because the reader below is on its own connection and a
    // transaction that never commits is invisible to everybody else.
    setup.commit().await.expect("committing the fixture");

    // The reader opens its snapshot and takes its first statement, the way the
    // visit handler does.
    let mut reader = database
        .begin_scoped_snapshot(restaurant_id)
        .await
        .expect("opening the reader's snapshot");

    let seen_round = service::round(&mut reader, round.id)
        .await
        .expect("the reader sees the ticket");
    assert_eq!(seen_round.status, RoundStatus::Queued);

    // A chef marks the dish and commits, in the middle of the reader's
    // transaction. This is the race, run in a fixed order rather than hoped
    // about.
    let mut chef = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening the chef's transaction");
    service::mark_line_ready(&mut chef, lines[0].id, f.chef)
        .await
        .expect("the dish comes off the pass");
    chef.commit().await.expect("committing the chef's tap");

    // The reader's next statement. Under READ COMMITTED it would see the dish
    // as ready while the ticket it already read says queued.
    let seen_lines = service::lines_for_round(&mut reader, round.id)
        .await
        .expect("the reader sees the dishes");

    assert_eq!(
        seen_lines[0].status,
        LineStatus::Queued,
        "the read saw a dish that changed after its snapshot opened, so a \
         document can carry half of somebody else's commit"
    );

    reader.rollback().await.expect("ending the read");

    // And a read that starts afterwards sees the whole of it, so nothing is
    // being hidden, only held still for the length of one answer.
    let mut after = database
        .begin_scoped_snapshot(restaurant_id)
        .await
        .expect("opening a later snapshot");

    assert_eq!(
        service::round(&mut after, round.id)
            .await
            .expect("reading the ticket")
            .status,
        RoundStatus::Ready
    );
    assert_eq!(
        service::lines_for_round(&mut after, round.id)
            .await
            .expect("reading the dishes")[0]
            .status,
        LineStatus::Ready
    );

    after.rollback().await.expect("ending the later read");
    common::drop_restaurant(&database, restaurant_id).await;
}
