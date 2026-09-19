//! The service loop: seating a party, sending tickets, and moving dishes along.
//!
//! Two things are worth knowing before reading any of it.
//!
//! **Every state change is a conditional update that names the state it
//! expects.** A chef marking a dish ready and a waiter voiding the same dish at
//! the same moment both run `WHERE ... AND status = <what I think it is>`. One
//! of them changes a row, the other changes none and is told the state moved.
//! Neither overwrites the other, and no lock is held across a request while a
//! busy kitchen waits.
//!
//! **A ticket's status is recomputed, never decided.** It is stored, because the
//! kitchen queue needs to filter on it, but every write that touches a line
//! recomputes the whole ticket from all of its lines inside the same
//! transaction, under the ticket's own row lock. The rule itself lives in the
//! domain, in [`round_status_from_lines`]; the lock is what makes it hold when
//! two dishes on one ticket are marked at the same instant.

use std::collections::HashMap;

use rust_decimal::Decimal;
use serde_json::json;
use uuid::Uuid;

use crate::domain::audit::AuditAction;
use crate::domain::catalog::DiningTable;
use crate::domain::enums::{LineStatus, RoundStatus, VisitStatus, VoidReason};
use crate::domain::error::{ConflictKind, DomainError, DomainResult};
use crate::domain::event::EntityKind;
use crate::domain::ids::{
    BillId, DiningTableId, DishId, OrderLineId, OrderRoundId, StaffId, TableSectionId, VisitId,
};
use crate::domain::service::{
    FloorTable, KitchenTicket, NewOrderLine, OpenOrder, OrderLine, OrderRound, TableOccupancy,
    Visit, round_status_from_lines,
};

use super::super::{Database, ScopedTx};
use super::conflict_on;

/// What a send produced: the ticket and its dishes, and whether it was made
/// just now or found from an earlier send with the same key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SentRound {
    /// The ticket.
    pub round: OrderRound,
    /// Its dishes, in the order they were sent.
    pub lines: Vec<OrderLine>,
    /// `true` when an earlier send with the same key already made this ticket,
    /// so nothing was written this time.
    pub replayed: bool,
}

/// What cancelling a dish changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoidedLine {
    /// The dish, after the write.
    pub line: OrderLine,
    /// What its ticket became, recomputed from all of its dishes.
    pub round_status: RoundStatus,
    /// The bill's running subtotal after the dish came off it, or `None` for a
    /// dish on no bill.
    pub bill_subtotal: Option<Decimal>,
}

/// Reads one visit.
///
/// # Errors
///
/// Returns [`DomainError::NotFound`] if no such visit belongs to this
/// restaurant. A visit belonging to another restaurant is not visible, so the
/// caller cannot tell the two cases apart, which is the intended answer.
pub async fn visit(tx: &mut ScopedTx<'_>, visit_id: VisitId) -> DomainResult<Visit> {
    let row = sqlx::query!(
        r#"
        SELECT id, table_id, status AS "status: VisitStatus", guest_count,
               opened_by_staff_id, responsible_staff_id, opened_at, closed_at
        FROM visits
        WHERE id = $1
        "#,
        visit_id.as_uuid()
    )
    .fetch_optional(tx.connection())
    .await?
    .ok_or(DomainError::NotFound)?;

    Ok(Visit {
        id: VisitId::from_uuid(row.id),
        table_id: DiningTableId::from_uuid(row.table_id),
        status: row.status,
        guest_count: row.guest_count,
        opened_by_staff_id: StaffId::from_uuid(row.opened_by_staff_id),
        responsible_staff_id: StaffId::from_uuid(row.responsible_staff_id),
        opened_at: row.opened_at,
        closed_at: row.closed_at,
    })
}

/// Reads one ticket.
///
/// # Errors
///
/// Returns [`DomainError::NotFound`] if no such ticket belongs to this
/// restaurant.
pub async fn round(tx: &mut ScopedTx<'_>, round_id: OrderRoundId) -> DomainResult<OrderRound> {
    let row = sqlx::query!(
        r#"
        SELECT id, visit_id, sequence_no, status AS "status: RoundStatus",
               sent_by_staff_id, sent_at, ready_at, served_at, client_key
        FROM order_rounds
        WHERE id = $1
        "#,
        round_id.as_uuid()
    )
    .fetch_optional(tx.connection())
    .await?
    .ok_or(DomainError::NotFound)?;

    Ok(OrderRound {
        id: OrderRoundId::from_uuid(row.id),
        visit_id: VisitId::from_uuid(row.visit_id),
        sequence_no: row.sequence_no,
        status: row.status,
        sent_by_staff_id: StaffId::from_uuid(row.sent_by_staff_id),
        sent_at: row.sent_at,
        ready_at: row.ready_at,
        served_at: row.served_at,
        client_key: row.client_key,
    })
}

/// Reads one dish on one ticket.
///
/// This is the single place an [`OrderLine`] is built, which is why the write
/// operations below re read through it rather than each assembling seventeen
/// columns of their own.
///
/// # Errors
///
/// Returns [`DomainError::NotFound`] if no such line belongs to this restaurant.
pub async fn line(tx: &mut ScopedTx<'_>, line_id: OrderLineId) -> DomainResult<OrderLine> {
    let row = sqlx::query!(
        r#"
        SELECT id, round_id, dish_id, bill_id, quantity, unit_price, dish_name, line_total,
               note, status AS "status: LineStatus", ready_by_staff_id, ready_at, served_at,
               voided_by_staff_id, voided_at,
               void_reason_code AS "void_reason_code: VoidReason", void_reason
        FROM order_lines
        WHERE id = $1
        "#,
        line_id.as_uuid()
    )
    .fetch_optional(tx.connection())
    .await?
    .ok_or(DomainError::NotFound)?;

    Ok(OrderLine {
        id: OrderLineId::from_uuid(row.id),
        round_id: OrderRoundId::from_uuid(row.round_id),
        dish_id: DishId::from_uuid(row.dish_id),
        bill_id: row.bill_id.map(BillId::from_uuid),
        quantity: row.quantity,
        unit_price: row.unit_price,
        dish_name: row.dish_name,
        line_total: row.line_total,
        note: row.note,
        status: row.status,
        ready_by_staff_id: row.ready_by_staff_id.map(StaffId::from_uuid),
        ready_at: row.ready_at,
        served_at: row.served_at,
        voided_by_staff_id: row.voided_by_staff_id.map(StaffId::from_uuid),
        voided_at: row.voided_at,
        void_reason_code: row.void_reason_code,
        void_reason: row.void_reason,
    })
}

/// Every dish on one ticket, oldest first.
///
/// # Errors
///
/// Returns [`DomainError::Unavailable`] if the read fails.
pub async fn lines_for_round(
    tx: &mut ScopedTx<'_>,
    round_id: OrderRoundId,
) -> DomainResult<Vec<OrderLine>> {
    let ids = sqlx::query!(
        "SELECT id FROM order_lines WHERE round_id = $1 ORDER BY created_at, id",
        round_id.as_uuid()
    )
    .fetch_all(tx.connection())
    .await?;

    let mut lines = Vec::with_capacity(ids.len());
    for row in ids {
        lines.push(line(tx, OrderLineId::from_uuid(row.id)).await?);
    }

    Ok(lines)
}

/// The whole floor as a waiter sees it, in the order it is walked.
///
/// One statement rather than a table read followed by a visit read per table.
/// A restaurant's floor is small, but the shape matters more than the size: two
/// reads would let a table be opened between them, and the screen would show a
/// table as free that a colleague had just taken.
///
/// Archived tables are left out. An archived section is not: its tables are
/// still real tables somebody can sit at, so they come back here and the
/// handler groups them where a section list can no longer name them.
///
/// # Errors
///
/// Returns [`DomainError::Unavailable`] if the read fails.
pub async fn floor(tx: &mut ScopedTx<'_>) -> DomainResult<Vec<FloorTable>> {
    let rows = sqlx::query!(
        r#"
        SELECT t.id, t.section_id, t.label, t.seats, t.position, t.version, t.archived_at,
               v.id            AS "visit_id?",
               v.opened_at     AS "opened_at?",
               v.guest_count   AS "guest_count?",
               opener.display_name AS "opened_by?",
               v.responsible_staff_id AS "responsible_staff_id?",
               responsible.display_name AS "responsible_name?",
               (
                   SELECT count(*)
                   FROM order_lines AS l
                   JOIN order_rounds AS r ON r.id = l.round_id
                   WHERE r.visit_id = v.id AND l.status = 'ready'
               )               AS "ready_dish_count?"
        FROM dining_tables AS t
        LEFT JOIN table_sections AS s ON s.id = t.section_id
        LEFT JOIN visits AS v ON v.table_id = t.id AND v.status = 'open'
        LEFT JOIN staff AS opener ON opener.id = v.opened_by_staff_id
        LEFT JOIN staff AS responsible ON responsible.id = v.responsible_staff_id
        WHERE t.archived_at IS NULL
        ORDER BY s.position NULLS LAST, s.name NULLS LAST, t.position, t.label, t.id
        "#
    )
    .fetch_all(tx.connection())
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| {
            // Every visit column comes from the same LEFT JOIN, so they are
            // present together or absent together. Built from the visit id
            // rather than from each one separately, so a half occupied table is
            // not representable.
            let occupancy = row.visit_id.map(|visit_id| TableOccupancy {
                visit_id: VisitId::from_uuid(visit_id),
                // A visit always has an opener: the column is not null and the
                // staff row is never deleted. The fallback exists so a read
                // cannot fail on a row that a future deletion made incomplete.
                opened_by: row.opened_by.unwrap_or_default(),
                // Never null on an open visit: the column is required, and it
                // was filled in the same statement that opened it.
                responsible_staff_id: StaffId::from_uuid(
                    row.responsible_staff_id.unwrap_or_default(),
                ),
                responsible_name: row.responsible_name.unwrap_or_default(),
                opened_at: row.opened_at.unwrap_or_else(chrono::Utc::now),
                guest_count: row.guest_count,
                ready_dish_count: row.ready_dish_count.unwrap_or(0),
            });

            FloorTable {
                table: DiningTable {
                    id: DiningTableId::from_uuid(row.id),
                    section_id: row.section_id.map(TableSectionId::from_uuid),
                    label: row.label,
                    seats: row.seats,
                    position: row.position,
                    version: row.version,
                    archived_at: row.archived_at,
                },
                occupancy,
            }
        })
        .collect())
}

/// Every ticket the kitchen still has work on, oldest first.
///
/// `queued` and `ready` only. A served ticket has left the pass and a voided one
/// was cancelled, and neither belongs on a screen a chef is cooking from.
///
/// Ordered by when it was sent, which is the order a kitchen works in, with the
/// identifier breaking a tie so two tickets sent in the same instant do not swap
/// places between two refetches.
///
/// Deliberately unlimited. A kitchen queue is bounded by the food a kitchen can
/// physically have open at once, which is a fact about restaurants rather than a
/// guarantee about this endpoint; feature 13 owns giving it an explicit ceiling.
///
/// # Errors
///
/// Returns [`DomainError::Unavailable`] if a read fails.
pub async fn kitchen_queue(tx: &mut ScopedTx<'_>) -> DomainResult<Vec<KitchenTicket>> {
    let rows = sqlx::query!(
        r#"
        SELECT r.id, r.visit_id, r.sequence_no, r.status AS "status: RoundStatus",
               r.sent_by_staff_id, r.sent_at, r.ready_at, r.served_at, r.client_key,
               t.label AS table_label
        FROM order_rounds AS r
        JOIN visits AS v ON v.id = r.visit_id
        JOIN dining_tables AS t ON t.id = v.table_id
        WHERE r.status IN ('queued', 'ready')
        ORDER BY r.sent_at, r.id
        "#
    )
    .fetch_all(tx.connection())
    .await?;

    let mut tickets = Vec::with_capacity(rows.len());

    for row in rows {
        let round_id = OrderRoundId::from_uuid(row.id);

        tickets.push(KitchenTicket {
            round: OrderRound {
                id: round_id,
                visit_id: VisitId::from_uuid(row.visit_id),
                sequence_no: row.sequence_no,
                status: row.status,
                sent_by_staff_id: StaffId::from_uuid(row.sent_by_staff_id),
                sent_at: row.sent_at,
                ready_at: row.ready_at,
                served_at: row.served_at,
                client_key: row.client_key,
            },
            table_label: row.table_label,
            lines: lines_for_round(tx, round_id).await?,
        });
    }

    Ok(tickets)
}

/// Every open visit in the restaurant, with every ticket and dish on it.
///
/// Two statements whatever the size of the floor: the visits, then every
/// ticket and dish on them in one join. Run it inside a snapshot transaction,
/// or a dish marked ready between the two could appear on a ticket read as
/// still cooking.
///
/// Visits come back in the order they were opened. The screen sorts them by
/// what is ready, which changes far more often than this read does.
///
/// # Errors
///
/// Returns [`DomainError::Unavailable`] if a read fails.
pub async fn open_orders(tx: &mut ScopedTx<'_>) -> DomainResult<Vec<OpenOrder>> {
    let visits = sqlx::query!(
        r#"
        SELECT v.id, v.table_id, t.label AS table_label, v.opened_at,
               v.responsible_staff_id, s.display_name AS responsible_name
        FROM visits AS v
        JOIN dining_tables AS t ON t.id = v.table_id
        JOIN staff AS s ON s.id = v.responsible_staff_id
        WHERE v.status = 'open'
        ORDER BY v.opened_at, v.id
        "#
    )
    .fetch_all(tx.connection())
    .await?;

    let visit_ids: Vec<Uuid> = visits.iter().map(|row| row.id).collect();

    let rows = sqlx::query!(
        r#"
        SELECT r.id AS round_id, r.visit_id, r.sequence_no,
               r.status AS "round_status: RoundStatus",
               r.sent_by_staff_id, r.sent_at, r.ready_at AS round_ready_at,
               r.served_at AS round_served_at, r.client_key,
               l.id AS line_id, l.dish_id, l.bill_id, l.quantity, l.unit_price, l.dish_name,
               l.line_total, l.note, l.status AS "line_status: LineStatus",
               l.ready_by_staff_id, l.ready_at, l.served_at, l.voided_by_staff_id,
               l.voided_at, l.void_reason_code AS "void_reason_code: VoidReason",
               l.void_reason
        FROM order_rounds AS r
        JOIN order_lines AS l ON l.round_id = r.id
        WHERE r.visit_id = ANY($1)
        ORDER BY r.visit_id, r.sequence_no, l.created_at, l.id
        "#,
        &visit_ids
    )
    .fetch_all(tx.connection())
    .await?;

    let mut rounds_by_visit: HashMap<Uuid, Vec<(OrderRound, Vec<OrderLine>)>> = HashMap::new();

    for row in rows {
        let rounds = rounds_by_visit.entry(row.visit_id).or_default();
        let round_id = OrderRoundId::from_uuid(row.round_id);

        let line = OrderLine {
            id: OrderLineId::from_uuid(row.line_id),
            round_id,
            dish_id: DishId::from_uuid(row.dish_id),
            bill_id: row.bill_id.map(BillId::from_uuid),
            quantity: row.quantity,
            unit_price: row.unit_price,
            dish_name: row.dish_name,
            line_total: row.line_total,
            note: row.note,
            status: row.line_status,
            ready_by_staff_id: row.ready_by_staff_id.map(StaffId::from_uuid),
            ready_at: row.ready_at,
            served_at: row.served_at,
            voided_by_staff_id: row.voided_by_staff_id.map(StaffId::from_uuid),
            voided_at: row.voided_at,
            void_reason_code: row.void_reason_code,
            void_reason: row.void_reason,
        };

        // Rows arrive grouped by round, so a new round only ever starts at the
        // end of the list.
        match rounds.last_mut() {
            Some((round, lines)) if round.id == round_id => lines.push(line),
            _ => rounds.push((
                OrderRound {
                    id: round_id,
                    visit_id: VisitId::from_uuid(row.visit_id),
                    sequence_no: row.sequence_no,
                    status: row.round_status,
                    sent_by_staff_id: StaffId::from_uuid(row.sent_by_staff_id),
                    sent_at: row.sent_at,
                    ready_at: row.round_ready_at,
                    served_at: row.round_served_at,
                    client_key: row.client_key,
                },
                vec![line],
            )),
        }
    }

    Ok(visits
        .into_iter()
        .map(|row| OpenOrder {
            visit_id: VisitId::from_uuid(row.id),
            table_id: DiningTableId::from_uuid(row.table_id),
            table_label: row.table_label,
            opened_at: row.opened_at,
            responsible_staff_id: StaffId::from_uuid(row.responsible_staff_id),
            responsible_name: row.responsible_name,
            rounds: rounds_by_visit.remove(&row.id).unwrap_or_default(),
        })
        .collect())
}

/// Every ticket on one visit, oldest first.
///
/// Unlike [`kitchen_queue`] this keeps every status, because the waiter's table
/// screen is a record of the whole meal rather than a work queue.
///
/// # Errors
///
/// Returns [`DomainError::Unavailable`] if a read fails.
pub async fn rounds_for_visit(
    tx: &mut ScopedTx<'_>,
    visit_id: VisitId,
) -> DomainResult<Vec<(OrderRound, Vec<OrderLine>)>> {
    let ids = sqlx::query!(
        "SELECT id FROM order_rounds WHERE visit_id = $1 ORDER BY sequence_no",
        visit_id.as_uuid()
    )
    .fetch_all(tx.connection())
    .await?;

    let mut rounds = Vec::with_capacity(ids.len());

    for row in ids {
        let round_id = OrderRoundId::from_uuid(row.id);
        let sent = round(tx, round_id).await?;
        let lines = lines_for_round(tx, round_id).await?;
        rounds.push((sent, lines));
    }

    Ok(rounds)
}

/// What the staff call one table.
///
/// A read of its own rather than a join onto every visit read, because the
/// label is the only thing about a table that a visit screen shows and pulling
/// the whole row for it would invite somebody to start using the rest.
///
/// # Errors
///
/// Returns [`DomainError::NotFound`] if no such table belongs to this
/// restaurant. Archived tables are included: a party may still be sitting at a
/// table the restaurant has since taken out of use, and their screen has to
/// name it.
pub async fn table_label(tx: &mut ScopedTx<'_>, table_id: DiningTableId) -> DomainResult<String> {
    let row = sqlx::query!(
        "SELECT label FROM dining_tables WHERE id = $1",
        table_id.as_uuid()
    )
    .fetch_optional(tx.connection())
    .await?
    .ok_or(DomainError::NotFound)?;

    Ok(row.label)
}

/// The one open bill on a visit, if it has one.
///
/// A visit has at most one open bill in this slice, because opening a table
/// creates exactly one and nothing else opens another. Splitting a bill would
/// change that, which is why this returns the bill rather than asserting there
/// is only ever one.
///
/// # Errors
///
/// Returns [`DomainError::Unavailable`] if the read fails.
pub async fn open_bill_of(
    tx: &mut ScopedTx<'_>,
    visit_id: VisitId,
) -> DomainResult<Option<BillId>> {
    let found = sqlx::query!(
        r#"
        SELECT id
        FROM bills
        WHERE visit_id = $1 AND status = 'open'
        ORDER BY created_at
        LIMIT 1
        "#,
        visit_id.as_uuid()
    )
    .fetch_optional(tx.connection())
    .await?;

    Ok(found.map(|row| BillId::from_uuid(row.id)))
}

/// Seats a party at a table.
///
/// The table holding at most one open visit is a database guarantee, not a check
/// this function performs: two waiters racing on the same table both insert, and
/// the partial unique index decides. That is why the conflict is recognised from
/// the constraint name rather than from a `SELECT` beforehand, which two racing
/// transactions would both pass.
///
/// # Errors
///
/// Returns [`DomainError::Invalid`] if the table is archived or does not exist,
/// and [`DomainError::Conflict`] if the table already has a party at it.
pub async fn open_visit(
    tx: &mut ScopedTx<'_>,
    table_id: DiningTableId,
    opened_by: StaffId,
    guest_count: Option<i16>,
) -> DomainResult<Visit> {
    if !lock_live_table(tx, table_id).await? {
        return Err(DomainError::Invalid(
            "that table is not in use right now".to_owned(),
        ));
    }

    let restaurant_id = tx.restaurant_id().as_uuid();
    let visit_id = VisitId::new();

    // Whoever opens the table is responsible for it until somebody takes it
    // over, so the same person goes into both columns.
    sqlx::query!(
        r#"
        INSERT INTO visits
            (id, restaurant_id, table_id, status, guest_count, opened_by_staff_id,
             responsible_staff_id, opened_at, updated_at)
        VALUES ($1, $2, $3, 'open', $4, $5, $5, now(), now())
        "#,
        visit_id.as_uuid(),
        restaurant_id,
        table_id.as_uuid(),
        guest_count,
        opened_by.as_uuid(),
    )
    .execute(tx.connection())
    .await
    .map_err(|error| {
        conflict_on(
            error,
            "visits_one_open_per_table",
            ConflictKind::TableOccupied,
        )
    })?;

    Database::notify_entity_change(tx, EntityKind::Visit, visit_id.as_uuid()).await?;

    visit(tx, visit_id).await
}

/// Moves a party to a different table.
///
/// The party takes everything with it: its tickets, its bill, its responsible
/// waiter, and any basket saved on a phone, because all of those hang off the
/// visit rather than the table. One audit row records where it went from and
/// to.
///
/// Every ticket still in the kitchen is announced as well, although no ticket
/// row is written. The kitchen screen reads each ticket's table label live
/// through its visit, so as the kitchen sees them those tickets did change, and
/// without the announcement the pass would send food to the old table.
///
/// # Errors
///
/// Returns [`DomainError::NotFound`] if there is no such visit, or the
/// destination is archived or not this restaurant's, and
/// [`DomainError::Conflict`] if the destination already has a party at it or
/// the visit is no longer open.
pub async fn move_visit(
    tx: &mut ScopedTx<'_>,
    visit_id: VisitId,
    new_table_id: DiningTableId,
    moved_by: StaffId,
) -> DomainResult<Visit> {
    if !lock_live_table(tx, new_table_id).await? {
        return Err(DomainError::NotFound);
    }

    // The visit's own row, locked before it is read, so the table it is moving
    // from in the audit row is the one it really left.
    let before = sqlx::query!(
        r#"
        SELECT table_id, status AS "status: VisitStatus"
        FROM visits
        WHERE id = $1
        FOR UPDATE
        "#,
        visit_id.as_uuid()
    )
    .fetch_optional(tx.connection())
    .await?
    .ok_or(DomainError::NotFound)?;

    if before.status != VisitStatus::Open {
        return Err(DomainError::Conflict(ConflictKind::VisitNot(
            VisitStatus::Open,
        )));
    }

    // Moving to the table it is already at is moving to an occupied table.
    if before.table_id == new_table_id.as_uuid() {
        return Err(DomainError::Conflict(ConflictKind::TableOccupied));
    }

    sqlx::query!(
        r#"
        UPDATE visits
        SET table_id = $2, updated_at = now()
        WHERE id = $1
        "#,
        visit_id.as_uuid(),
        new_table_id.as_uuid(),
    )
    .execute(tx.connection())
    .await
    .map_err(|error| {
        conflict_on(
            error,
            "visits_one_open_per_table",
            ConflictKind::TableOccupied,
        )
    })?;

    super::audit::record(
        tx,
        Some(moved_by),
        AuditAction::VisitMoved,
        "visit",
        visit_id.as_uuid(),
        Some(json!({ "table_id": before.table_id })),
        Some(json!({ "table_id": new_table_id.as_uuid() })),
    )
    .await?;

    Database::notify_entity_change(tx, EntityKind::Visit, visit_id.as_uuid()).await?;

    // The one deliberate notify with no write to the row it names: see above.
    let in_kitchen = sqlx::query!(
        r#"
        SELECT id FROM order_rounds
        WHERE visit_id = $1 AND status IN ('queued', 'ready')
        ORDER BY sequence_no
        "#,
        visit_id.as_uuid()
    )
    .fetch_all(tx.connection())
    .await?;

    for row in in_kitchen {
        Database::notify_entity_change(tx, EntityKind::OrderRound, row.id).await?;
    }

    visit(tx, visit_id).await
}

/// Makes one waiter responsible for an open table, taking it from whoever the
/// screen showed was responsible.
///
/// A conditional update naming that expected waiter, so two waiters taking the
/// same table at once leave exactly one winner, and the loser is told somebody
/// else got there first rather than silently overwriting them.
///
/// # Errors
///
/// Returns [`DomainError::Conflict`] with `table_taken_over` if the responsible
/// waiter is no longer the expected one, with `visit_not_open` if the party has
/// left, and [`DomainError::NotFound`] if there is no such visit.
pub async fn take_over_visit(
    tx: &mut ScopedTx<'_>,
    visit_id: VisitId,
    expected: StaffId,
    taken_by: StaffId,
) -> DomainResult<Visit> {
    let taken = sqlx::query!(
        r#"
        UPDATE visits
        SET responsible_staff_id = $3, updated_at = now()
        WHERE id = $1 AND status = 'open' AND responsible_staff_id = $2
        RETURNING id
        "#,
        visit_id.as_uuid(),
        expected.as_uuid(),
        taken_by.as_uuid(),
    )
    .fetch_optional(tx.connection())
    .await?;

    if taken.is_none() {
        let found = sqlx::query!(
            r#"SELECT status AS "status: VisitStatus" FROM visits WHERE id = $1"#,
            visit_id.as_uuid()
        )
        .fetch_optional(tx.connection())
        .await?
        .ok_or(DomainError::NotFound)?;

        return Err(DomainError::Conflict(
            if found.status == VisitStatus::Open {
                ConflictKind::TableTakenOver
            } else {
                ConflictKind::VisitNot(VisitStatus::Open)
            },
        ));
    }

    super::audit::record(
        tx,
        Some(taken_by),
        AuditAction::VisitTakenOver,
        "visit",
        visit_id.as_uuid(),
        Some(json!({ "responsible_staff_id": expected.as_uuid() })),
        Some(json!({ "responsible_staff_id": taken_by.as_uuid() })),
    )
    .await?;

    Database::notify_entity_change(tx, EntityKind::Visit, visit_id.as_uuid()).await?;

    visit(tx, visit_id).await
}

/// Takes the visit's row lock.
///
/// Every path that locks more than one thing takes them in one order: the
/// visit, then a line or its ticket, then the bill, then the bill number
/// counter. Close calls this first for exactly that reason. Without it, close
/// locked the bill and then updated the visit row, which is a visit lock taken
/// after a bill lock, and a send on the same visit (visit, then bill) could
/// deadlock against it. Feature 23 will lock two bills at once and must fit in
/// here too, lower bill id first.
///
/// # Errors
///
/// Returns [`DomainError::NotFound`] if there is no such visit.
pub async fn lock_visit(tx: &mut ScopedTx<'_>, visit_id: VisitId) -> DomainResult<VisitStatus> {
    let row = sqlx::query!(
        r#"SELECT status AS "status: VisitStatus" FROM visits WHERE id = $1 FOR UPDATE"#,
        visit_id.as_uuid()
    )
    .fetch_optional(tx.connection())
    .await?
    .ok_or(DomainError::NotFound)?;

    Ok(row.status)
}

/// Frees the table when the party leaves.
///
/// Refused while any of the visit's bills is still open, or while any dish that
/// counts has not been assigned to a bill, because either one means somebody is
/// about to lose money. A visit with no bills at all closes freely: that is a
/// party who sat down, changed their mind, and left.
///
/// # Errors
///
/// Returns [`DomainError::Conflict`] if a bill is still open, if a dish is still
/// unassigned, or if the visit is not open.
pub async fn close_visit(tx: &mut ScopedTx<'_>, visit_id: VisitId) -> DomainResult<Visit> {
    let blockers = sqlx::query!(
        r#"
        SELECT
            EXISTS (
                SELECT 1 FROM bills
                WHERE visit_id = $1 AND status = 'open'
            ) AS "bill_still_open!",
            EXISTS (
                SELECT 1
                FROM order_lines AS l
                JOIN order_rounds AS r ON r.id = l.round_id
                WHERE r.visit_id = $1 AND l.status <> 'voided' AND l.bill_id IS NULL
            ) AS "line_unassigned!"
        "#,
        visit_id.as_uuid()
    )
    .fetch_one(tx.connection())
    .await?;

    if blockers.bill_still_open {
        return Err(DomainError::Conflict(ConflictKind::VisitHasOpenBill));
    }

    if blockers.line_unassigned {
        return Err(DomainError::Conflict(ConflictKind::VisitHasUnbilledLine));
    }

    let closed = sqlx::query!(
        r#"
        UPDATE visits
        SET status = 'closed', closed_at = now(), updated_at = now()
        WHERE id = $1 AND status = 'open'
        RETURNING id
        "#,
        visit_id.as_uuid()
    )
    .fetch_optional(tx.connection())
    .await?;

    if closed.is_none() {
        return Err(visit_conflict(tx, visit_id, VisitStatus::Open).await);
    }

    Database::notify_entity_change(tx, EntityKind::Visit, visit_id.as_uuid()).await?;

    visit(tx, visit_id).await
}

/// Sends one ticket of dishes to the kitchen.
///
/// The price and the name are read from the dish here and copied onto the line,
/// so a caller cannot name its own price and a later reprice cannot rewrite this
/// ticket. The ticket's number within the visit is allocated under the visit's
/// row lock, so two waiters sending at once cannot both take the same one.
///
/// **A send is safe to repeat.** The phone makes `client_key` once per basket
/// and sends it every time until one send succeeds. Under the visit lock this
/// first looks for a ticket with that key: found on this visit, it hands that
/// ticket back with `replayed` set and writes nothing, so a waiter who taps
/// Send again after a timeout never makes a second kitchen ticket. Two sends
/// with one key at the same instant queue on the lock, and the second finds
/// the first. The unique index is the guard behind that, for a key reused on
/// another visit at the same instant.
///
/// # Errors
///
/// Returns [`DomainError::Invalid`] if the ticket is empty or a quantity is not
/// positive, and [`DomainError::Conflict`] if the visit is not open, a dish is
/// archived or currently unavailable, or the key already made a ticket on a
/// different visit.
pub async fn send_round(
    tx: &mut ScopedTx<'_>,
    visit_id: VisitId,
    sent_by: StaffId,
    client_key: Uuid,
    new_lines: &[NewOrderLine],
) -> DomainResult<SentRound> {
    if new_lines.is_empty() {
        return Err(DomainError::Invalid(
            "a ticket needs at least one dish on it".to_owned(),
        ));
    }

    // Locking the visit is what makes the sequence number safe, and what makes
    // two sends with one key queue rather than both insert.
    let status = lock_visit(tx, visit_id).await?;

    // Before the status check, so a send whose answer was lost still gets its
    // ticket back after the party has left.
    if let Some(replay) = earlier_send(tx, visit_id, client_key).await? {
        return Ok(replay);
    }

    if status != VisitStatus::Open {
        return Err(DomainError::Conflict(ConflictKind::VisitNot(
            VisitStatus::Open,
        )));
    }

    let priced = price_lines(tx, new_lines).await?;

    let next_sequence = sqlx::query!(
        r#"
        SELECT coalesce(max(sequence_no), 0) + 1 AS "next!"
        FROM order_rounds
        WHERE visit_id = $1
        "#,
        visit_id.as_uuid()
    )
    .fetch_one(tx.connection())
    .await?
    .next;

    let restaurant_id = tx.restaurant_id().as_uuid();
    let round_id = OrderRoundId::new();

    sqlx::query!(
        r#"
        INSERT INTO order_rounds
            (id, restaurant_id, visit_id, sequence_no, status, sent_by_staff_id,
             sent_at, updated_at, client_key)
        VALUES ($1, $2, $3, $4, 'queued', $5, now(), now(), $6)
        "#,
        round_id.as_uuid(),
        restaurant_id,
        visit_id.as_uuid(),
        next_sequence,
        sent_by.as_uuid(),
        client_key,
    )
    .execute(tx.connection())
    .await
    .map_err(|error| {
        conflict_on(
            error,
            "order_rounds_one_per_client_key",
            ConflictKind::ClientKeyReused,
        )
    })?;

    let mut line_ids = Vec::with_capacity(priced.len());

    for (new_line, dish_name, price) in priced {
        let line_id = OrderLineId::new();
        let line_total = price * Decimal::from(new_line.quantity);

        sqlx::query!(
            r#"
            INSERT INTO order_lines
                (id, restaurant_id, round_id, dish_id, quantity, unit_price, dish_name,
                 line_total, note, status, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'queued', now())
            "#,
            line_id.as_uuid(),
            restaurant_id,
            round_id.as_uuid(),
            new_line.dish_id.as_uuid(),
            new_line.quantity,
            price,
            dish_name,
            line_total,
            new_line.note,
        )
        .execute(tx.connection())
        .await?;

        line_ids.push(line_id);
    }

    Database::notify_entity_change(tx, EntityKind::OrderRound, round_id.as_uuid()).await?;

    let sent = round(tx, round_id).await?;
    let mut lines = Vec::with_capacity(line_ids.len());
    for line_id in line_ids {
        lines.push(line(tx, line_id).await?);
    }

    Ok(SentRound {
        round: sent,
        lines,
        replayed: false,
    })
}

/// The ticket an earlier send with this key made, if there was one.
///
/// Found on this visit, it comes back marked as a replay. Found on another
/// visit, the key is being reused, which is refused.
async fn earlier_send(
    tx: &mut ScopedTx<'_>,
    visit_id: VisitId,
    client_key: Uuid,
) -> DomainResult<Option<SentRound>> {
    let earlier = sqlx::query!(
        "SELECT id, visit_id FROM order_rounds WHERE client_key = $1",
        client_key
    )
    .fetch_optional(tx.connection())
    .await?;

    let Some(earlier) = earlier else {
        return Ok(None);
    };

    if earlier.visit_id != visit_id.as_uuid() {
        return Err(DomainError::Conflict(ConflictKind::ClientKeyReused));
    }

    let round_id = OrderRoundId::from_uuid(earlier.id);

    Ok(Some(SentRound {
        round: round(tx, round_id).await?,
        lines: lines_for_round(tx, round_id).await?,
        replayed: true,
    }))
}

/// Reads every dish's name and price, refusing the whole basket if one of them
/// cannot be ordered.
///
/// Every dish is checked before anything is written, so a ticket carrying one
/// that went off is refused whole by `send_round` itself, not only by the
/// caller dropping its transaction. Archived or switched off is refused here
/// rather than by a constraint, because a dish already on an open bill must be
/// unaffected when the kitchen runs out: only new lines are stopped. A conflict
/// rather than an invalid request, because nothing was wrong with the basket
/// when it was built: the menu moved under it, and the waiter's screen
/// refetches the menu and flags the line when it hears this code.
async fn price_lines<'a>(
    tx: &mut ScopedTx<'_>,
    new_lines: &'a [NewOrderLine],
) -> DomainResult<Vec<(&'a NewOrderLine, String, Decimal)>> {
    let mut priced = Vec::with_capacity(new_lines.len());

    for new_line in new_lines {
        if new_line.quantity <= 0 {
            return Err(DomainError::Invalid(
                "a dish has to be ordered at least once".to_owned(),
            ));
        }

        let dish = sqlx::query!(
            r#"
            SELECT name, price
            FROM dishes
            WHERE id = $1 AND archived_at IS NULL AND is_available
            "#,
            new_line.dish_id.as_uuid()
        )
        .fetch_optional(tx.connection())
        .await?
        .ok_or(DomainError::Conflict(ConflictKind::DishNotOrderable))?;

        priced.push((new_line, dish.name, dish.price));
    }

    Ok(priced)
}

/// Marks one dish off the pass.
///
/// Touches only that dish. Every other dish on the ticket is left exactly as it
/// was, which is the whole point: food leaves the kitchen as it is ready rather
/// than in one batch.
///
/// # Errors
///
/// Returns [`DomainError::Conflict`] if the dish is no longer queued, which is
/// what a chef and a waiter acting at the same moment produces for whichever of
/// them is second.
pub async fn mark_line_ready(
    tx: &mut ScopedTx<'_>,
    line_id: OrderLineId,
    staff_id: StaffId,
) -> DomainResult<(OrderLine, RoundStatus)> {
    let updated = sqlx::query!(
        r#"
        UPDATE order_lines
        SET status = 'ready', ready_by_staff_id = $2, ready_at = now(), updated_at = now()
        WHERE id = $1 AND status = 'queued'
        RETURNING round_id
        "#,
        line_id.as_uuid(),
        staff_id.as_uuid(),
    )
    .fetch_optional(tx.connection())
    .await?;

    let Some(updated) = updated else {
        return Err(line_conflict(tx, line_id, LineStatus::Queued).await);
    };

    finish_line_write(tx, line_id, OrderRoundId::from_uuid(updated.round_id)).await
}

/// Marks one dish as having reached the table.
///
/// # Errors
///
/// Returns [`DomainError::Conflict`] if the dish is not waiting to be carried
/// out.
pub async fn mark_line_served(
    tx: &mut ScopedTx<'_>,
    line_id: OrderLineId,
) -> DomainResult<(OrderLine, RoundStatus)> {
    let updated = sqlx::query!(
        r#"
        UPDATE order_lines
        SET status = 'served', served_at = now(), updated_at = now()
        WHERE id = $1 AND status = 'ready'
        RETURNING round_id
        "#,
        line_id.as_uuid()
    )
    .fetch_optional(tx.connection())
    .await?;

    let Some(updated) = updated else {
        return Err(line_conflict(tx, line_id, LineStatus::Ready).await);
    };

    finish_line_write(tx, line_id, OrderRoundId::from_uuid(updated.round_id)).await
}

/// Cancels one dish, with a reason and the person who cancelled it, and takes
/// it off the bill.
///
/// A dish still cooking or waiting on the pass may be cancelled. One already on
/// the table cannot: at that point it has been eaten or sent back, and either
/// way that is a different conversation than a void.
///
/// Four writes in one transaction, in the one lock order: the line, then its
/// ticket (recomputed, which can turn it ready when the last cooking dish is
/// the one cancelled), then the bill's subtotal, then the audit row.
///
/// `reason` must already be tidied by
/// [`void_reason_text`](crate::domain::service::void_reason_text); the check
/// constraint refuses `other` without it either way.
///
/// # Errors
///
/// Returns [`DomainError::Conflict`] with `line_not_voidable` if the dish has
/// already been served or cancelled, and [`DomainError::NotFound`] if there is
/// no such dish.
pub async fn void_line(
    tx: &mut ScopedTx<'_>,
    line_id: OrderLineId,
    staff_id: StaffId,
    reason_code: VoidReason,
    reason: Option<&str>,
) -> DomainResult<VoidedLine> {
    // The line's own row, locked, so the status written to the audit row as
    // "before" is the one this void replaced and not one a chef's tap moved
    // on from a moment earlier.
    let before = sqlx::query!(
        r#"SELECT status AS "status: LineStatus" FROM order_lines WHERE id = $1 FOR UPDATE"#,
        line_id.as_uuid()
    )
    .fetch_optional(tx.connection())
    .await?
    .ok_or(DomainError::NotFound)?
    .status;

    let updated = sqlx::query!(
        r#"
        UPDATE order_lines
        SET status = 'voided', voided_by_staff_id = $2, voided_at = now(),
            void_reason_code = $3, void_reason = $4, updated_at = now()
        WHERE id = $1 AND status IN ('queued', 'ready')
        RETURNING round_id, bill_id
        "#,
        line_id.as_uuid(),
        staff_id.as_uuid(),
        reason_code as VoidReason,
        reason,
    )
    .fetch_optional(tx.connection())
    .await?
    .ok_or(DomainError::Conflict(ConflictKind::LineNotVoidable))?;

    let (written, round_status) =
        finish_line_write(tx, line_id, OrderRoundId::from_uuid(updated.round_id)).await?;

    let bill_subtotal = match updated.bill_id {
        Some(bill_id) => {
            Some(super::billing::recompute_subtotal(tx, BillId::from_uuid(bill_id)).await?)
        }
        None => None,
    };

    super::audit::record(
        tx,
        Some(staff_id),
        AuditAction::LineVoided,
        "order_line",
        line_id.as_uuid(),
        Some(json!({
            "status": before,
            "dish_name": written.dish_name,
            "line_total": written.line_total,
        })),
        Some(json!({
            "status": LineStatus::Voided,
            "void_reason_code": reason_code,
            "void_reason": reason,
        })),
    )
    .await?;

    Ok(VoidedLine {
        line: written,
        round_status,
        bill_subtotal,
    })
}

/// Marks every dish on a ticket that is waiting to be carried out, and leaves
/// the ones still cooking.
///
/// A line level conflict inside the loop means somebody else served that dish
/// between the read and the write, so it is reported as `nothing_ready`: that
/// is what it means to the waiter reading it.
///
/// # Errors
///
/// Returns [`DomainError::Conflict`] with `nothing_ready` if no dish on the
/// ticket is waiting to be carried out, and [`DomainError::NotFound`] if there
/// is no such ticket.
pub async fn serve_ready_lines(tx: &mut ScopedTx<'_>, round_id: OrderRoundId) -> DomainResult<()> {
    // Read first, so a ticket that does not exist is not found rather than a
    // ticket with nothing ready on it.
    round(tx, round_id).await?;

    let ready: Vec<OrderLineId> = lines_for_round(tx, round_id)
        .await?
        .into_iter()
        .filter(|line| line.status == LineStatus::Ready)
        .map(|line| line.id)
        .collect();

    if ready.is_empty() {
        return Err(DomainError::Conflict(ConflictKind::NothingReady));
    }

    for line_id in ready {
        match mark_line_served(tx, line_id).await {
            Ok(_) => {}
            Err(DomainError::Conflict(_)) => {
                return Err(DomainError::Conflict(ConflictKind::NothingReady));
            }
            Err(other) => return Err(other),
        }
    }

    Ok(())
}

/// The tail every line write shares: announce the line, recompute its ticket,
/// and hand both back.
async fn finish_line_write(
    tx: &mut ScopedTx<'_>,
    line_id: OrderLineId,
    round_id: OrderRoundId,
) -> DomainResult<(OrderLine, RoundStatus)> {
    Database::notify_entity_change(tx, EntityKind::OrderLine, line_id.as_uuid()).await?;

    let round_status = recompute_round_status(tx, round_id).await?;
    let written = line(tx, line_id).await?;

    Ok((written, round_status))
}

/// Recomputes a ticket's status from its dishes and stores it if it moved.
///
/// Announces the ticket only when the status actually changed, so a kitchen
/// screen is not woken up nine times while nine dishes are marked ready on a
/// ticket that stays queued throughout.
async fn recompute_round_status(
    tx: &mut ScopedTx<'_>,
    round_id: OrderRoundId,
) -> DomainResult<RoundStatus> {
    // The ticket's own row, locked, before its dishes are read.
    //
    // This is what makes the recompute correct when two dishes on one ticket
    // are marked at the same moment, which is an ordinary evening in a kitchen
    // with two chefs. Without it both transactions read the lines under their
    // own snapshot, each sees the other's dish still queued, and both write
    // "queued" back. The ticket then sits for ever as cooking with every dish
    // on it ready, the waiter is never told the food is up, and nothing
    // anywhere reports an error.
    //
    // With the lock the second recompute waits, and when it runs its next
    // statement it takes a fresh snapshot that includes the first one's
    // committed line. Only one round is ever locked here, so there is no order
    // for two transactions to disagree about and no deadlock to have.
    sqlx::query!(
        "SELECT id FROM order_rounds WHERE id = $1 FOR UPDATE",
        round_id.as_uuid()
    )
    .fetch_optional(tx.connection())
    .await?
    .ok_or(DomainError::NotFound)?;

    let rows = sqlx::query!(
        r#"SELECT status AS "status: LineStatus" FROM order_lines WHERE round_id = $1"#,
        round_id.as_uuid()
    )
    .fetch_all(tx.connection())
    .await?;

    let statuses: Vec<LineStatus> = rows.into_iter().map(|row| row.status).collect();
    let next = round_status_from_lines(&statuses);

    let moved = sqlx::query!(
        r#"
        UPDATE order_rounds
        SET status = $2,
            ready_at = CASE
                WHEN $2 = 'ready'::round_status AND ready_at IS NULL THEN now()
                ELSE ready_at
            END,
            served_at = CASE
                WHEN $2 = 'served'::round_status AND served_at IS NULL THEN now()
                ELSE served_at
            END,
            updated_at = now()
        WHERE id = $1 AND status IS DISTINCT FROM $2
        RETURNING id
        "#,
        round_id.as_uuid(),
        next as RoundStatus,
    )
    .fetch_optional(tx.connection())
    .await?;

    if moved.is_some() {
        Database::notify_entity_change(tx, EntityKind::OrderRound, round_id.as_uuid()).await?;
    }

    Ok(next)
}

/// Whether a table is live and this restaurant's, holding it live until the
/// transaction ends when it is.
///
/// `FOR SHARE`, which conflicts with the `FOR UPDATE` an admin's archive takes
/// (`floor::archive_dining_table`). Whichever comes second waits for the first
/// and then sees its committed result, so a party is never seated at a table
/// that was removed at the same instant, and a table is never removed from
/// under a party seated at that instant. Each caller decides what "not live"
/// means for it: opening a table calls it an invalid request, moving a party
/// calls it not found.
async fn lock_live_table(tx: &mut ScopedTx<'_>, table_id: DiningTableId) -> DomainResult<bool> {
    let live = sqlx::query!(
        "SELECT id FROM dining_tables WHERE id = $1 AND archived_at IS NULL FOR SHARE",
        table_id.as_uuid()
    )
    .fetch_optional(tx.connection())
    .await?;

    Ok(live.is_some())
}

/// Works out why a conditional update on a line matched nothing.
///
/// Only ever runs on the failure path, so the extra read costs nothing in the
/// normal case. It is what turns "zero rows changed" into either "somebody else
/// got there first, and here is where it is now" or "no such dish", which are
/// two very different things to show a chef.
async fn line_conflict(
    tx: &mut ScopedTx<'_>,
    line_id: OrderLineId,
    expected: LineStatus,
) -> DomainError {
    let found = sqlx::query!(
        "SELECT id FROM order_lines WHERE id = $1",
        line_id.as_uuid()
    )
    .fetch_optional(tx.connection())
    .await;

    // The conflict names the state the writer expected rather than the state it
    // found, and that is deliberate. What the reader needs to be told is which
    // action was refused, and the row may well have moved again between the
    // failed update and this read, so reporting what it says now would be a
    // sentence about a moment that has already passed.
    match found {
        Ok(Some(_)) => DomainError::Conflict(ConflictKind::LineNot(expected)),
        Ok(None) => DomainError::NotFound,
        Err(error) => DomainError::from(error),
    }
}

/// The same, for a visit.
async fn visit_conflict(
    tx: &mut ScopedTx<'_>,
    visit_id: VisitId,
    expected: VisitStatus,
) -> DomainError {
    let found = sqlx::query!("SELECT id FROM visits WHERE id = $1", visit_id.as_uuid())
        .fetch_optional(tx.connection())
        .await;

    match found {
        Ok(Some(_)) => DomainError::Conflict(ConflictKind::VisitNot(expected)),
        Ok(None) => DomainError::NotFound,
        Err(error) => DomainError::from(error),
    }
}
