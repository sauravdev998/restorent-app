//! The waiter service flow, spec 0011, at the layer the handlers sit on.
//!
//! Against a real Postgres, as `app_api`. Most tests roll back. The race tests
//! at the bottom commit, because two transactions only see each other's work
//! once it lands, and they delete their restaurant afterwards.
//!
//! Covers AC-4, AC-5, AC-6, AC-8, AC-12, AC-13, AC-14, AC-15, AC-16, AC-17,
//! and the isolation half of AC-18.

mod common;

use api::domain::enums::{BillStatus, LineStatus, RoundStatus, VisitStatus, VoidReason};
use api::domain::error::DomainError;
use api::domain::ids::{DiningTableId, OrderLineId, RestaurantId, StaffId, VisitId};
use api::domain::service::{NewOrderLine, OrderLine};
use api::infrastructure::db::ScopedTx;
use api::infrastructure::db::repository::{billing, service};
use uuid::Uuid;

/// The code a refusal carries, or a panic naming what came back instead.
fn code_of<T: std::fmt::Debug>(outcome: Result<T, DomainError>, what: &str) -> &'static str {
    match outcome {
        Err(DomainError::Conflict(kind)) => kind.as_code(),
        other => panic!("{what} came back as {other:?} rather than a conflict"),
    }
}

fn one(dish: api::domain::ids::DishId) -> Vec<NewOrderLine> {
    vec![NewOrderLine {
        dish_id: dish,
        quantity: 1,
        note: None,
    }]
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
    table: DiningTableId,
    waiter: StaffId,
) -> (VisitId, api::domain::ids::BillId) {
    let visit = service::open_visit(tx, table, waiter, Some(2))
        .await
        .expect("the party sits down");
    let bill = billing::open_bill(tx, visit.id, waiter)
        .await
        .expect("opening the bill");
    (visit.id, bill.id)
}

/// A send the way the handler does it: the ticket, then its lines onto the bill.
async fn send(
    tx: &mut ScopedTx<'_>,
    visit: VisitId,
    bill: api::domain::ids::BillId,
    waiter: StaffId,
    key: Uuid,
    lines: &[NewOrderLine],
) -> service::SentRound {
    let sent = service::send_round(tx, visit, waiter, key, lines)
        .await
        .expect("sending the ticket");
    if !sent.replayed {
        let ids: Vec<OrderLineId> = sent.lines.iter().map(|line| line.id).collect();
        billing::assign_lines_to_bill(tx, bill, &ids)
            .await
            .expect("putting the dishes on the bill");
    }
    sent
}

async fn audit_actions(tx: &mut ScopedTx<'_>, entity: Uuid) -> Vec<String> {
    sqlx::query_scalar(
        "SELECT action FROM audit_log WHERE entity_id = $1 ORDER BY occurred_at, action",
    )
    .bind(entity)
    .fetch_all(tx.connection())
    .await
    .expect("reading the audit log")
}

// ===========================================================================
// Rounds and the send key
// ===========================================================================

/// AC-5, AC-8: the same key twice makes one ticket, and the second answer is
/// that same ticket marked as a replay. A new key makes a second round with the
/// next number.
#[tokio::test]
async fn a_repeated_send_key_makes_one_ticket_and_a_new_key_makes_the_next_round() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut tx, restaurant_id).await;
    let (visit, bill) = seat(&mut tx, f.table_one, f.waiter).await;

    let key = Uuid::now_v7();
    let first = send(&mut tx, visit, bill, f.waiter, key, &one(f.soup)).await;
    let again = send(&mut tx, visit, bill, f.waiter, key, &one(f.soup)).await;

    assert!(!first.replayed);
    assert!(
        again.replayed,
        "the second send with one key was not a replay"
    );
    assert_eq!(again.round.id, first.round.id);
    assert_eq!(
        again.lines.iter().map(|line| line.id).collect::<Vec<_>>(),
        first.lines.iter().map(|line| line.id).collect::<Vec<_>>()
    );

    let second_round = send(
        &mut tx,
        visit,
        bill,
        f.waiter,
        Uuid::now_v7(),
        &one(f.steak),
    )
    .await;
    assert_eq!(second_round.round.sequence_no, 2);

    let rounds = service::rounds_for_visit(&mut tx, visit)
        .await
        .expect("reading the rounds");
    assert_eq!(rounds.len(), 2, "a replayed send made a ticket of its own");

    let subtotal = billing::bill(&mut tx, bill)
        .await
        .expect("reading the bill")
        .subtotal;
    assert_eq!(
        subtotal,
        common::money("34.50"),
        "a replay counted its dishes on the bill a second time"
    );
}

/// AC-8: a key already used on another visit is refused.
#[tokio::test]
async fn a_send_key_from_another_table_is_refused() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut tx, restaurant_id).await;
    let (one_visit, one_bill) = seat(&mut tx, f.table_one, f.waiter).await;
    let (two_visit, _) = seat(&mut tx, f.table_two, f.waiter).await;

    let key = Uuid::now_v7();
    send(&mut tx, one_visit, one_bill, f.waiter, key, &one(f.soup)).await;

    let reused = service::send_round(&mut tx, two_visit, f.waiter, key, &one(f.soup)).await;
    assert_eq!(code_of(reused, "a reused key"), "client_key_reused");
}

/// AC-7, AC-8: a send whose answer was lost still gets its ticket back after
/// the party has left, rather than `visit_not_open`.
#[tokio::test]
async fn a_replay_after_the_table_closed_still_returns_the_ticket() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut tx, restaurant_id).await;
    let (visit, bill) = seat(&mut tx, f.table_one, f.waiter).await;

    let key = Uuid::now_v7();
    let sent = send(&mut tx, visit, bill, f.waiter, key, &one(f.soup)).await;
    let line = sent.lines[0].id;
    service::mark_line_ready(&mut tx, line, f.chef)
        .await
        .expect("ready");
    service::mark_line_served(&mut tx, line)
        .await
        .expect("served");
    billing::end_visit(&mut tx, visit, f.waiter)
        .await
        .expect("closing the table");

    let replay = service::send_round(&mut tx, visit, f.waiter, key, &one(f.soup))
        .await
        .expect("the replay is answered");
    assert!(replay.replayed);
    assert_eq!(replay.round.id, sent.round.id);

    let fresh = service::send_round(&mut tx, visit, f.waiter, Uuid::now_v7(), &one(f.soup)).await;
    assert_eq!(
        code_of(fresh, "a new send to a closed table"),
        "visit_not_open"
    );
}

// ===========================================================================
// Notes
// ===========================================================================

/// AC-6: 140 Devanagari characters are stored and read back unchanged, on the
/// waiter's read and on the kitchen ticket, and the database refuses 141.
#[tokio::test]
async fn a_long_hindi_note_reaches_the_kitchen_whole_and_the_database_holds_the_ceiling() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut tx, restaurant_id).await;
    let (visit, bill) = seat(&mut tx, f.table_one, f.waiter).await;

    let note = "प्याज़ नहीं".chars().cycle().take(140).collect::<String>();
    let sent = send(
        &mut tx,
        visit,
        bill,
        f.waiter,
        Uuid::now_v7(),
        &[NewOrderLine {
            dish_id: f.soup,
            quantity: 1,
            note: Some(note.clone()),
        }],
    )
    .await;
    assert_eq!(sent.lines[0].note.as_deref(), Some(note.as_str()));

    let pass = service::kitchen_queue(&mut tx)
        .await
        .expect("reading the pass");
    assert_eq!(pass[0].lines[0].note.as_deref(), Some(note.as_str()));

    common::savepoint(&mut tx, "long_note").await;
    let too_long = sqlx::query("UPDATE order_lines SET note = $2 WHERE id = $1")
        .bind(sent.lines[0].id.as_uuid())
        .bind(format!("{note}x"))
        .execute(tx.connection())
        .await;
    assert!(
        too_long.is_err(),
        "the database stored a note of 141 characters"
    );
    common::rollback_to(&mut tx, "long_note").await;
}

// ===========================================================================
// Serving
// ===========================================================================

/// AC-12: one ready dish is served on its own, and serving one that is still
/// cooking is refused.
#[tokio::test]
async fn one_dish_is_served_on_its_own_while_the_rest_cooks() {
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
        Uuid::now_v7(),
        &soup_and_steak(&f),
    )
    .await;
    let (soup, steak) = (sent.lines[0].id, sent.lines[1].id);

    let early = service::mark_line_served(&mut tx, steak).await;
    assert_eq!(code_of(early, "serving a cooking dish"), "line_not_ready");

    service::mark_line_ready(&mut tx, soup, f.chef)
        .await
        .expect("the soup is ready");
    let (served, round_status) = service::mark_line_served(&mut tx, soup)
        .await
        .expect("serving the soup");

    assert_eq!(served.status, LineStatus::Served);
    assert_eq!(
        round_status,
        RoundStatus::Queued,
        "the steak is still cooking"
    );
    let steak_now = service::line(&mut tx, steak).await.expect("the steak");
    assert_eq!(steak_now.status, LineStatus::Queued);
}

/// AC-12: serve all ready serves what is ready even while another dish cooks,
/// and is refused only when nothing on the ticket is ready.
#[tokio::test]
async fn serve_all_ready_serves_what_is_ready_and_refuses_when_nothing_is() {
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
        Uuid::now_v7(),
        &soup_and_steak(&f),
    )
    .await;
    let (soup, steak) = (sent.lines[0].id, sent.lines[1].id);

    let nothing = service::serve_ready_lines(&mut tx, sent.round.id).await;
    assert_eq!(
        code_of(nothing, "serve all ready with nothing ready"),
        "nothing_ready"
    );

    service::mark_line_ready(&mut tx, soup, f.chef)
        .await
        .expect("the soup is ready");
    service::serve_ready_lines(&mut tx, sent.round.id)
        .await
        .expect("serving what is ready");

    assert_eq!(
        service::line(&mut tx, soup).await.expect("soup").status,
        LineStatus::Served
    );
    assert_eq!(
        service::line(&mut tx, steak).await.expect("steak").status,
        LineStatus::Queued
    );

    service::mark_line_ready(&mut tx, steak, f.chef)
        .await
        .expect("the steak is ready");
    service::serve_ready_lines(&mut tx, sent.round.id)
        .await
        .expect("serving the steak");

    let round = service::round(&mut tx, sent.round.id)
        .await
        .expect("the round");
    assert_eq!(round.status, RoundStatus::Served);
    assert!(
        service::kitchen_queue(&mut tx)
            .await
            .expect("the pass")
            .is_empty(),
        "a fully served ticket stayed in the kitchen queue"
    );
}

// ===========================================================================
// Voids
// ===========================================================================

/// AC-13: a void takes the dish off the bill's subtotal and writes one audit
/// row carrying the reason code.
#[tokio::test]
async fn a_void_lowers_the_subtotal_and_is_audited_with_its_code() {
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
        Uuid::now_v7(),
        &soup_and_steak(&f),
    )
    .await;

    let voided = service::void_line(
        &mut tx,
        sent.lines[1].id,
        f.waiter,
        VoidReason::EnteredByMistake,
        None,
    )
    .await
    .expect("cancelling the steak");

    assert_eq!(voided.line.status, LineStatus::Voided);
    assert_eq!(
        voided.line.void_reason_code,
        Some(VoidReason::EnteredByMistake)
    );
    assert_eq!(voided.bill_subtotal, Some(common::money("9.50")));
    assert_eq!(
        billing::bill(&mut tx, bill)
            .await
            .expect("the bill")
            .subtotal,
        common::money("9.50")
    );

    let codes: Vec<Option<String>> = sqlx::query_scalar(
        "SELECT after ->> 'void_reason_code' FROM audit_log
          WHERE entity_id = $1 AND action = 'line_voided'",
    )
    .bind(sent.lines[1].id.as_uuid())
    .fetch_all(tx.connection())
    .await
    .expect("reading the audit row");
    assert_eq!(codes, vec![Some("entered_by_mistake".to_owned())]);
}

/// AC-13, AC-14: voiding the last cooking dish turns a ticket with a ready dish
/// ready; voiding every dish cancels it and takes it off the pass; a served
/// dish cannot be voided.
#[tokio::test]
async fn voids_move_the_ticket_the_way_its_dishes_say() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut tx, restaurant_id).await;
    let (visit, bill) = seat(&mut tx, f.table_one, f.waiter).await;

    let first = send(
        &mut tx,
        visit,
        bill,
        f.waiter,
        Uuid::now_v7(),
        &soup_and_steak(&f),
    )
    .await;
    service::mark_line_ready(&mut tx, first.lines[0].id, f.chef)
        .await
        .expect("the soup is ready");
    let last_cooking = service::void_line(
        &mut tx,
        first.lines[1].id,
        f.waiter,
        VoidReason::KitchenUnavailable,
        None,
    )
    .await
    .expect("cancelling the steak");
    assert_eq!(
        last_cooking.round_status,
        RoundStatus::Ready,
        "cancelling the last cooking dish did not make the ticket ready"
    );

    let second = send(
        &mut tx,
        visit,
        bill,
        f.waiter,
        Uuid::now_v7(),
        &one(f.steak),
    )
    .await;
    let all_gone = service::void_line(
        &mut tx,
        second.lines[0].id,
        f.waiter,
        VoidReason::Other,
        Some("spilled on the way"),
    )
    .await
    .expect("cancelling the only dish");
    assert_eq!(all_gone.round_status, RoundStatus::Voided);
    let pass = service::kitchen_queue(&mut tx).await.expect("the pass");
    assert!(
        pass.iter().all(|ticket| ticket.round.id != second.round.id),
        "a ticket with every dish cancelled stayed on the pass"
    );

    service::mark_line_served(&mut tx, first.lines[0].id)
        .await
        .expect("serving the soup");
    let too_late = service::void_line(
        &mut tx,
        first.lines[0].id,
        f.waiter,
        VoidReason::GuestChangedMind,
        None,
    )
    .await;
    assert_eq!(
        code_of(too_late, "voiding a served dish"),
        "line_not_voidable"
    );

    let twice = service::void_line(
        &mut tx,
        first.lines[1].id,
        f.waiter,
        VoidReason::GuestChangedMind,
        None,
    )
    .await;
    assert_eq!(code_of(twice, "voiding a voided dish"), "line_not_voidable");

    let nobody = service::void_line(
        &mut tx,
        OrderLineId::new(),
        f.waiter,
        VoidReason::GuestChangedMind,
        None,
    )
    .await;
    assert!(matches!(nobody, Err(DomainError::NotFound)));
}

/// AC-14: the kitchen still shows a cancelled dish on a ticket that has others
/// left, so the chef sees it was cancelled rather than watching it vanish.
#[tokio::test]
async fn the_pass_keeps_a_cancelled_dish_on_its_ticket() {
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
        Uuid::now_v7(),
        &soup_and_steak(&f),
    )
    .await;

    service::void_line(
        &mut tx,
        sent.lines[0].id,
        f.waiter,
        VoidReason::GuestChangedMind,
        None,
    )
    .await
    .expect("cancelling the soup");

    let pass = service::kitchen_queue(&mut tx).await.expect("the pass");
    let statuses: Vec<LineStatus> = pass[0].lines.iter().map(|line| line.status).collect();
    assert_eq!(statuses, vec![LineStatus::Voided, LineStatus::Queued]);
}

// ===========================================================================
// Ownership, moves, and the Orders read
// ===========================================================================

/// AC-4: whoever opens a table is responsible for it, a colleague can take it
/// over naming who they expect to take it from, and a stale expectation is
/// refused.
#[tokio::test]
async fn a_table_is_taken_over_only_from_the_waiter_the_screen_showed() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut tx, restaurant_id).await;
    let colleague = common::seed_staff(
        &mut tx,
        restaurant_id.as_uuid(),
        &format!(
            "colleague-{}@example.test",
            restaurant_id.as_uuid().simple()
        ),
        "Cara Colleague",
        "waiter",
    )
    .await;
    let (visit, _) = seat(&mut tx, f.table_one, f.waiter).await;

    let opened = service::visit(&mut tx, visit).await.expect("the visit");
    assert_eq!(opened.responsible_staff_id, f.waiter);

    let taken = service::take_over_visit(&mut tx, visit, f.waiter, colleague)
        .await
        .expect("the colleague takes the table");
    assert_eq!(taken.responsible_staff_id, colleague);
    assert_eq!(
        audit_actions(&mut tx, visit.as_uuid()).await,
        vec!["visit_taken_over".to_owned()]
    );

    let stale = service::take_over_visit(&mut tx, visit, f.waiter, f.waiter).await;
    assert_eq!(code_of(stale, "a stale take over"), "table_taken_over");

    let orders = service::open_orders(&mut tx)
        .await
        .expect("the Orders read");
    assert_eq!(orders[0].responsible_staff_id, colleague);
    assert_eq!(orders[0].responsible_name, "Cara Colleague");

    let floor = service::floor(&mut tx).await.expect("the floor");
    let occupancy = floor[0].occupancy.as_ref().expect("table one is occupied");
    assert_eq!(occupancy.responsible_name, "Cara Colleague");
    assert_eq!(occupancy.opened_by, "Wes Waiter");

    let missing = service::take_over_visit(&mut tx, VisitId::new(), f.waiter, colleague).await;
    assert!(matches!(missing, Err(DomainError::NotFound)));
}

/// AC-15: a party moves with its tickets and bill, the kitchen ticket reads the
/// new label, one audit row is written, and an occupied, archived, or unknown
/// table is refused.
#[tokio::test]
async fn a_party_moves_with_everything_and_the_pass_follows() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut tx, restaurant_id).await;
    let (visit, bill) = seat(&mut tx, f.table_one, f.waiter).await;
    let sent = send(&mut tx, visit, bill, f.waiter, Uuid::now_v7(), &one(f.soup)).await;

    let moved = service::move_visit(&mut tx, visit, f.table_two, f.waiter)
        .await
        .expect("moving the party");
    assert_eq!(moved.table_id, f.table_two);
    assert_eq!(moved.responsible_staff_id, f.waiter);

    let pass = service::kitchen_queue(&mut tx).await.expect("the pass");
    assert_eq!(pass[0].round.id, sent.round.id);
    assert_eq!(pass[0].table_label, "T2");
    assert_eq!(
        service::open_bill_of(&mut tx, visit)
            .await
            .expect("the bill"),
        Some(bill)
    );
    assert_eq!(
        audit_actions(&mut tx, visit.as_uuid()).await,
        vec!["visit_moved".to_owned()]
    );

    let (other, _) = seat(&mut tx, f.table_one, f.waiter).await;
    let onto_itself = service::move_visit(&mut tx, other, f.table_one, f.waiter).await;
    assert_eq!(
        code_of(onto_itself, "a move onto its own table"),
        "table_occupied"
    );

    // The unique index refuses this one, which aborts the statement, so it is
    // wrapped to let the test carry on.
    common::savepoint(&mut tx, "occupied").await;
    let occupied = service::move_visit(&mut tx, other, f.table_two, f.waiter).await;
    assert_eq!(code_of(occupied, "a move onto a party"), "table_occupied");
    common::rollback_to(&mut tx, "occupied").await;

    let unknown = service::move_visit(&mut tx, other, DiningTableId::new(), f.waiter).await;
    assert!(matches!(unknown, Err(DomainError::NotFound)));

    let archived = common::seed_table_labelled(&mut tx, &f, "Old").await;
    sqlx::query("UPDATE dining_tables SET archived_at = now() WHERE id = $1")
        .bind(archived.as_uuid())
        .execute(tx.connection())
        .await
        .expect("archiving the table");
    let onto_archived = service::move_visit(&mut tx, other, archived, f.waiter).await;
    assert!(
        matches!(onto_archived, Err(DomainError::NotFound)),
        "a move onto an archived table answered {onto_archived:?}"
    );
}

/// AC-2, AC-5: the Orders read lists every open visit with every round and
/// dish, and nothing from a closed one.
#[tokio::test]
async fn the_orders_read_holds_every_open_table_and_every_round_on_it() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut tx, restaurant_id).await;
    let (first, first_bill) = seat(&mut tx, f.table_one, f.waiter).await;
    let (second, _) = seat(&mut tx, f.table_two, f.waiter).await;

    send(
        &mut tx,
        first,
        first_bill,
        f.waiter,
        Uuid::now_v7(),
        &soup_and_steak(&f),
    )
    .await;
    let later = send(
        &mut tx,
        first,
        first_bill,
        f.waiter,
        Uuid::now_v7(),
        &one(f.soup),
    )
    .await;
    service::mark_line_ready(&mut tx, later.lines[0].id, f.chef)
        .await
        .expect("ready");

    let orders = service::open_orders(&mut tx)
        .await
        .expect("the Orders read");
    assert_eq!(
        orders
            .iter()
            .map(|order| order.visit_id)
            .collect::<Vec<_>>(),
        vec![first, second]
    );

    let rounds = &orders[0].rounds;
    assert_eq!(rounds.len(), 2);
    assert_eq!(rounds[0].0.sequence_no, 1);
    assert_eq!(rounds[0].1.len(), 2);
    assert_eq!(rounds[1].0.sequence_no, 2);
    let ready: Vec<&OrderLine> = rounds[1].1.iter().collect();
    assert_eq!(ready[0].status, LineStatus::Ready);
    assert!(ready[0].ready_at.is_some());
    assert_eq!(orders[0].table_label, "T1");
    assert!(orders[1].rounds.is_empty());
}

// ===========================================================================
// The empty close
// ===========================================================================

/// AC-16: a table whose only dish was cancelled closes with its bill voided,
/// no number used, and the next real bill takes number one.
#[tokio::test]
async fn an_empty_bill_is_voided_and_uses_no_number() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut tx, restaurant_id).await;

    let (visit, bill) = seat(&mut tx, f.table_one, f.waiter).await;
    let sent = send(&mut tx, visit, bill, f.waiter, Uuid::now_v7(), &one(f.soup)).await;
    service::void_line(
        &mut tx,
        sent.lines[0].id,
        f.waiter,
        VoidReason::GuestChangedMind,
        None,
    )
    .await
    .expect("cancelling the only dish");

    let settled = billing::end_visit(&mut tx, visit, f.waiter)
        .await
        .expect("closing an empty table");
    assert_eq!(settled.status, BillStatus::Voided);
    assert_eq!(settled.number, None);
    assert_eq!(settled.total, common::money("0"));
    assert_eq!(
        service::visit(&mut tx, visit)
            .await
            .expect("the visit")
            .status,
        VisitStatus::Closed
    );
    assert_eq!(
        audit_actions(&mut tx, bill.as_uuid()).await,
        vec!["bill_voided".to_owned()]
    );

    let again = billing::end_visit(&mut tx, visit, f.waiter).await;
    assert_eq!(code_of(again, "closing it twice"), "bill_already_closed");

    // A party that ordered nothing closes the same way.
    let (nothing, _) = seat(&mut tx, f.table_one, f.waiter).await;
    let empty = billing::end_visit(&mut tx, nothing, f.waiter)
        .await
        .expect("closing a table that ordered nothing");
    assert_eq!(empty.status, BillStatus::Voided);

    let (real, real_bill) = seat(&mut tx, f.table_one, f.waiter).await;
    let dish = send(
        &mut tx,
        real,
        real_bill,
        f.waiter,
        Uuid::now_v7(),
        &one(f.soup),
    )
    .await;
    service::mark_line_ready(&mut tx, dish.lines[0].id, f.chef)
        .await
        .expect("ready");
    service::mark_line_served(&mut tx, dish.lines[0].id)
        .await
        .expect("served");
    let closed = billing::end_visit(&mut tx, real, f.waiter)
        .await
        .expect("closing a real bill");
    assert_eq!(closed.status, BillStatus::Closed);
    assert_eq!(
        closed.number,
        Some(1),
        "a voided empty bill used up a number"
    );
}

// ===========================================================================
// Isolation
// ===========================================================================

/// AC-18: another restaurant's visits are absent from the Orders read, and its
/// visit, dish, and table ids are not found on every new write.
#[tokio::test]
async fn another_restaurants_ids_are_not_found_on_every_new_write() {
    let database = common::database().await;
    let alpha = RestaurantId::new();
    let beta = RestaurantId::new();

    let mut tx = database.begin_scoped(alpha).await.expect("opening scoped");
    let a = common::seed(&mut tx, alpha).await;

    common::rescope(&mut tx, beta).await;
    let b = common::seed(&mut tx, beta).await;
    let (beta_visit, _) = common::seed_visit_and_bill(&mut tx, beta, b.table_one, b.waiter).await;
    let (beta_round, beta_line) =
        common::seed_round_and_line(&mut tx, beta, beta_visit, b.waiter, b.soup).await;

    common::rescope(&mut tx, alpha).await;
    let (alpha_visit, _) = seat(&mut tx, a.table_one, a.waiter).await;

    let orders = service::open_orders(&mut tx).await.expect("alpha's Orders");
    assert_eq!(
        orders
            .iter()
            .map(|order| order.visit_id)
            .collect::<Vec<_>>(),
        vec![alpha_visit],
        "alpha's Orders list shows beta's table"
    );

    let beta_visit = VisitId::from_uuid(beta_visit);
    let beta_line = OrderLineId::from_uuid(beta_line);

    assert!(matches!(
        service::take_over_visit(&mut tx, beta_visit, b.waiter, a.waiter).await,
        Err(DomainError::NotFound)
    ));
    assert!(matches!(
        service::move_visit(&mut tx, beta_visit, a.table_two, a.waiter).await,
        Err(DomainError::NotFound)
    ));
    assert!(matches!(
        service::move_visit(&mut tx, alpha_visit, b.table_two, a.waiter).await,
        Err(DomainError::NotFound)
    ));
    assert!(matches!(
        service::mark_line_served(&mut tx, beta_line).await,
        Err(DomainError::NotFound)
    ));
    assert!(matches!(
        service::void_line(&mut tx, beta_line, a.waiter, VoidReason::Other, Some("x")).await,
        Err(DomainError::NotFound)
    ));
    assert!(matches!(
        service::serve_ready_lines(
            &mut tx,
            api::domain::ids::OrderRoundId::from_uuid(beta_round)
        )
        .await,
        Err(DomainError::NotFound)
    ));
    assert!(matches!(
        billing::end_visit(&mut tx, beta_visit, a.waiter).await,
        Err(DomainError::NotFound)
    ));
}

// ===========================================================================
// Races
// ===========================================================================

/// Commits one restaurant with a party at table one and a two dish ticket on
/// it, for the race tests.
async fn committed_table(
    database: &api::infrastructure::db::Database,
    restaurant_id: RestaurantId,
) -> (common::Fixture, VisitId, service::SentRound) {
    let mut setup = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut setup, restaurant_id).await;
    let (visit, bill) = seat(&mut setup, f.table_one, f.waiter).await;
    let sent = send(
        &mut setup,
        visit,
        bill,
        f.waiter,
        Uuid::now_v7(),
        &soup_and_steak(&f),
    )
    .await;
    setup.commit().await.expect("committing the fixture");
    (f, visit, sent)
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

async fn line_status(
    database: &api::infrastructure::db::Database,
    restaurant_id: RestaurantId,
    line: OrderLineId,
) -> LineStatus {
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    service::line(&mut tx, line).await.expect("the line").status
}

/// AC-17: mark ready arriving after a void loses with `line_not_queued`; a void
/// arriving after mark ready is not a conflict, and the dish ends voided.
#[tokio::test]
async fn ready_and_void_on_one_dish_leave_one_answer_whichever_comes_first() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let (f, _, sent) = committed_table(&database, restaurant_id).await;
    let (soup, steak) = (sent.lines[0].id, sent.lines[1].id);
    let (waiter, chef) = (f.waiter, f.chef);

    let (voided, ready) = race!(
        database,
        restaurant_id,
        |tx| service::void_line(&mut tx, soup, waiter, VoidReason::GuestChangedMind, None)
            .await
            .expect("the void"),
        |racer| service::mark_line_ready(&mut racer, soup, chef).await
    );
    assert_eq!(voided.line.status, LineStatus::Voided);
    assert_eq!(code_of(ready, "ready after a void"), "line_not_queued");

    let (_, void_after_ready) = race!(
        database,
        restaurant_id,
        |tx| service::mark_line_ready(&mut tx, steak, chef)
            .await
            .expect("the ready"),
        |racer| service::void_line(
            &mut racer,
            steak,
            waiter,
            VoidReason::KitchenUnavailable,
            None
        )
        .await
    );
    assert!(
        void_after_ready.is_ok(),
        "voiding a ready dish was refused: {void_after_ready:?}"
    );
    assert_eq!(
        line_status(&database, restaurant_id, steak).await,
        LineStatus::Voided
    );

    common::drop_restaurant(&database, restaurant_id).await;
}

/// AC-17: a void arriving after a serve loses with `line_not_voidable`, and two
/// serves or two voids of one dish leave one winner.
#[tokio::test]
async fn serves_and_voids_on_one_dish_leave_one_winner() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let (f, _, sent) = committed_table(&database, restaurant_id).await;
    let (soup, steak) = (sent.lines[0].id, sent.lines[1].id);
    let (waiter, chef) = (f.waiter, f.chef);

    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    service::mark_line_ready(&mut tx, soup, chef)
        .await
        .expect("the soup is ready");
    tx.commit().await.expect("committing");

    let (_, void_after_serve) = race!(
        database,
        restaurant_id,
        |tx| service::mark_line_served(&mut tx, soup)
            .await
            .expect("the serve"),
        |racer| service::void_line(&mut racer, soup, waiter, VoidReason::GuestChangedMind, None)
            .await
    );
    assert_eq!(
        code_of(void_after_serve, "a void after a serve"),
        "line_not_voidable"
    );

    let (_, second_serve) = race!(
        database,
        restaurant_id,
        |tx| service::void_line(&mut tx, steak, waiter, VoidReason::GuestChangedMind, None)
            .await
            .expect("the first void"),
        |racer| service::void_line(
            &mut racer,
            steak,
            waiter,
            VoidReason::EnteredByMistake,
            None
        )
        .await
    );
    assert_eq!(code_of(second_serve, "a second void"), "line_not_voidable");

    // Two serves: a fresh ready dish.
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let bill = service::open_bill_of(&mut tx, sent.round.visit_id)
        .await
        .expect("the bill")
        .expect("an open bill");
    let more = send(
        &mut tx,
        sent.round.visit_id,
        bill,
        waiter,
        Uuid::now_v7(),
        &one(f.soup),
    )
    .await;
    let dish = more.lines[0].id;
    service::mark_line_ready(&mut tx, dish, chef)
        .await
        .expect("ready");
    tx.commit().await.expect("committing");

    let (_, second) = race!(
        database,
        restaurant_id,
        |tx| service::mark_line_served(&mut tx, dish)
            .await
            .expect("the first serve"),
        |racer| service::mark_line_served(&mut racer, dish).await
    );
    assert_eq!(code_of(second, "a second serve"), "line_not_ready");

    common::drop_restaurant(&database, restaurant_id).await;
}

/// AC-4, AC-17: two waiters taking over the same table from the same colleague
/// leave one of them responsible.
#[tokio::test]
async fn two_take_overs_of_one_table_leave_one_winner() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let (f, visit, _) = committed_table(&database, restaurant_id).await;

    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let suffix = restaurant_id.as_uuid().simple().to_string();
    let first = common::seed_staff(
        &mut tx,
        restaurant_id.as_uuid(),
        &format!("first-{suffix}@example.test"),
        "First",
        "waiter",
    )
    .await;
    let second = common::seed_staff(
        &mut tx,
        restaurant_id.as_uuid(),
        &format!("second-{suffix}@example.test"),
        "Second",
        "waiter",
    )
    .await;
    tx.commit().await.expect("committing");
    let owner = f.waiter;

    let (_, loser) = race!(
        database,
        restaurant_id,
        |tx| service::take_over_visit(&mut tx, visit, owner, first)
            .await
            .expect("the first take over"),
        |racer| service::take_over_visit(&mut racer, visit, owner, second).await
    );
    assert_eq!(code_of(loser, "the second take over"), "table_taken_over");

    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    assert_eq!(
        service::visit(&mut tx, visit)
            .await
            .expect("the visit")
            .responsible_staff_id,
        first
    );
    drop(tx);

    common::drop_restaurant(&database, restaurant_id).await;
}

/// AC-15, AC-17: two parties moving to one free table leave one of them there.
#[tokio::test]
async fn two_moves_to_one_table_leave_one_party_there() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let (f, first, _) = committed_table(&database, restaurant_id).await;

    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let spare = common::seed_table_labelled(&mut tx, &f, "T3").await;
    let (second, _) = seat(&mut tx, spare, f.waiter).await;
    tx.commit().await.expect("committing");
    let (table_two, waiter) = (f.table_two, f.waiter);

    let (_, loser) = race!(
        database,
        restaurant_id,
        |tx| service::move_visit(&mut tx, first, table_two, waiter)
            .await
            .expect("the first move"),
        |racer| service::move_visit(&mut racer, second, table_two, waiter).await
    );
    assert_eq!(code_of(loser, "the second move"), "table_occupied");

    common::drop_restaurant(&database, restaurant_id).await;
}

/// AC-8: two sends with one key at the same instant make exactly one ticket.
#[tokio::test]
async fn two_sends_with_one_key_at_once_make_one_ticket() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let (f, visit, _) = committed_table(&database, restaurant_id).await;
    let key = Uuid::now_v7();
    let (waiter, soup) = (f.waiter, f.soup);

    let (first, second) = race!(
        database,
        restaurant_id,
        |tx| service::send_round(&mut tx, visit, waiter, key, &one(soup))
            .await
            .expect("the first send"),
        |racer| service::send_round(&mut racer, visit, waiter, key, &one(soup)).await
    );
    let second = second.expect("the second send is answered");
    assert!(!first.replayed);
    assert!(second.replayed, "two sends with one key both made a ticket");
    assert_eq!(first.round.id, second.round.id);

    common::drop_restaurant(&database, restaurant_id).await;
}

/// The lock order: a send and a close on one table at once end cleanly, one of
/// them refused with a conflict, never a deadlock.
#[tokio::test]
async fn a_send_and_a_close_on_one_table_never_deadlock() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let (f, visit, sent) = committed_table(&database, restaurant_id).await;
    let (waiter, chef, soup) = (f.waiter, f.chef, f.soup);

    // Everything on the table served, so the close is allowed on its own.
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    for line in &sent.lines {
        service::mark_line_ready(&mut tx, line.id, chef)
            .await
            .expect("ready");
        service::mark_line_served(&mut tx, line.id)
            .await
            .expect("served");
    }
    tx.commit().await.expect("committing");

    let (_, send_after_close) = race!(
        database,
        restaurant_id,
        |tx| billing::end_visit(&mut tx, visit, waiter)
            .await
            .expect("the close"),
        |racer| service::send_round(&mut racer, visit, waiter, Uuid::now_v7(), &one(soup)).await
    );
    assert_eq!(
        code_of(send_after_close, "a send racing a close"),
        "visit_not_open"
    );

    common::drop_restaurant(&database, restaurant_id).await;
}

/// AC-16, AC-17: a void of the last chargeable dish racing the close of its
/// table leaves the bill consistent: the close waits for the void, then finds
/// nothing to charge and voids the bill.
#[tokio::test]
async fn a_void_racing_an_empty_close_leaves_the_bill_consistent() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let mut setup = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut setup, restaurant_id).await;
    let (visit, bill) = seat(&mut setup, f.table_one, f.waiter).await;
    let sent = send(
        &mut setup,
        visit,
        bill,
        f.waiter,
        Uuid::now_v7(),
        &one(f.soup),
    )
    .await;
    setup.commit().await.expect("committing");
    let (line, waiter) = (sent.lines[0].id, f.waiter);

    let (_, closed) = race!(
        database,
        restaurant_id,
        |tx| service::void_line(&mut tx, line, waiter, VoidReason::GuestChangedMind, None)
            .await
            .expect("the void"),
        |racer| billing::end_visit(&mut racer, visit, waiter).await
    );
    let closed = closed.expect("the close after the void");
    assert_eq!(closed.status, BillStatus::Voided);
    assert_eq!(closed.number, None);

    common::drop_restaurant(&database, restaurant_id).await;
}
