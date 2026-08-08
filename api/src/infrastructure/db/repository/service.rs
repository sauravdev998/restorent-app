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
//! transaction. The rule itself lives in the domain, in
//! [`round_status_from_lines`].

use crate::domain::enums::{LineStatus, RoundStatus, VisitStatus};
use crate::domain::error::{DomainError, DomainResult};
use crate::domain::event::EntityKind;
use crate::domain::ids::{
    BillId, DiningTableId, DishId, OrderLineId, OrderRoundId, StaffId, VisitId,
};
use crate::domain::service::{NewOrderLine, OrderLine, OrderRound, Visit, round_status_from_lines};

use super::super::{Database, ScopedTx};
use super::conflict_on;

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
               opened_by_staff_id, opened_at, closed_at
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
               sent_by_staff_id, sent_at, ready_at, served_at
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
               voided_by_staff_id, voided_at, void_reason
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
    require_live_table(tx, table_id).await?;

    let restaurant_id = tx.restaurant_id().as_uuid();
    let visit_id = VisitId::new();

    sqlx::query!(
        r#"
        INSERT INTO visits
            (id, restaurant_id, table_id, status, guest_count, opened_by_staff_id,
             opened_at, updated_at)
        VALUES ($1, $2, $3, 'open', $4, $5, now(), now())
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
            "that table already has a party at it",
        )
    })?;

    Database::notify_entity_change(tx, EntityKind::Visit, visit_id.as_uuid()).await?;

    visit(tx, visit_id).await
}

/// Moves a party to a different table.
///
/// # Errors
///
/// Returns [`DomainError::Invalid`] if the destination is archived,
/// [`DomainError::Conflict`] if the destination already has a party at it or the
/// visit is no longer open, and [`DomainError::NotFound`] if there is no such
/// visit.
pub async fn move_visit(
    tx: &mut ScopedTx<'_>,
    visit_id: VisitId,
    new_table_id: DiningTableId,
) -> DomainResult<Visit> {
    require_live_table(tx, new_table_id).await?;

    let moved = sqlx::query!(
        r#"
        UPDATE visits
        SET table_id = $2, updated_at = now()
        WHERE id = $1 AND status = 'open'
        RETURNING id
        "#,
        visit_id.as_uuid(),
        new_table_id.as_uuid(),
    )
    .fetch_optional(tx.connection())
    .await
    .map_err(|error| {
        conflict_on(
            error,
            "visits_one_open_per_table",
            "that table already has a party at it",
        )
    })?;

    if moved.is_none() {
        return Err(visit_conflict(tx, visit_id, "open").await);
    }

    Database::notify_entity_change(tx, EntityKind::Visit, visit_id.as_uuid()).await?;

    visit(tx, visit_id).await
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
        return Err(DomainError::Conflict(
            "a bill on this visit is still open".to_owned(),
        ));
    }

    if blockers.line_unassigned {
        return Err(DomainError::Conflict(
            "a dish on this visit has not been put on a bill".to_owned(),
        ));
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
        return Err(visit_conflict(tx, visit_id, "open").await);
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
/// # Errors
///
/// Returns [`DomainError::Invalid`] if the ticket is empty, if a quantity is not
/// positive, or if a dish is archived or currently unavailable, and
/// [`DomainError::Conflict`] if the visit is not open.
pub async fn send_round(
    tx: &mut ScopedTx<'_>,
    visit_id: VisitId,
    sent_by: StaffId,
    new_lines: &[NewOrderLine],
) -> DomainResult<(OrderRound, Vec<OrderLine>)> {
    if new_lines.is_empty() {
        return Err(DomainError::Invalid(
            "a ticket needs at least one dish on it".to_owned(),
        ));
    }

    // Locking the visit is what makes the sequence number safe. Without it two
    // waiters sending at the same moment both read the same maximum and the
    // second insert fails on the unique constraint instead of queueing.
    let locked = sqlx::query!(
        r#"SELECT status AS "status: VisitStatus" FROM visits WHERE id = $1 FOR UPDATE"#,
        visit_id.as_uuid()
    )
    .fetch_optional(tx.connection())
    .await?
    .ok_or(DomainError::NotFound)?;

    if locked.status != VisitStatus::Open {
        return Err(DomainError::Conflict(
            "that party has already left".to_owned(),
        ));
    }

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
             sent_at, updated_at)
        VALUES ($1, $2, $3, $4, 'queued', $5, now(), now())
        "#,
        round_id.as_uuid(),
        restaurant_id,
        visit_id.as_uuid(),
        next_sequence,
        sent_by.as_uuid(),
    )
    .execute(tx.connection())
    .await?;

    let mut line_ids = Vec::with_capacity(new_lines.len());

    for new_line in new_lines {
        if new_line.quantity <= 0 {
            return Err(DomainError::Invalid(
                "a dish has to be ordered at least once".to_owned(),
            ));
        }

        // Archived or switched off is refused here rather than by a constraint,
        // because a dish already on an open bill must be unaffected when the
        // kitchen runs out: only new lines are stopped.
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
        .ok_or_else(|| DomainError::Invalid("that dish is not on the menu right now".to_owned()))?;

        let line_id = OrderLineId::new();
        let line_total = dish.price * rust_decimal::Decimal::from(new_line.quantity);

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
            dish.price,
            dish.name,
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

    Ok((sent, lines))
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

/// Cancels one dish, with a reason and the person who cancelled it.
///
/// A dish already on the table cannot be cancelled: at that point it has been
/// eaten or sent back, and either way that is a different conversation than a
/// void.
///
/// # Errors
///
/// Returns [`DomainError::Invalid`] if no reason is given, and
/// [`DomainError::Conflict`] if the dish has already been served or cancelled.
pub async fn void_line(
    tx: &mut ScopedTx<'_>,
    line_id: OrderLineId,
    staff_id: StaffId,
    reason: &str,
) -> DomainResult<(OrderLine, RoundStatus)> {
    let reason = reason.trim();

    if reason.is_empty() {
        return Err(DomainError::Invalid(
            "cancelling a dish needs a reason".to_owned(),
        ));
    }

    let before = line(tx, line_id).await?;

    let updated = sqlx::query!(
        r#"
        UPDATE order_lines
        SET status = 'voided', voided_by_staff_id = $2, voided_at = now(),
            void_reason = $3, updated_at = now()
        WHERE id = $1 AND status IN ('queued', 'ready')
        RETURNING round_id
        "#,
        line_id.as_uuid(),
        staff_id.as_uuid(),
        reason,
    )
    .fetch_optional(tx.connection())
    .await?;

    let Some(updated) = updated else {
        return Err(line_conflict(tx, line_id, LineStatus::Queued).await);
    };

    super::audit::record(
        tx,
        Some(staff_id),
        crate::domain::audit::AuditAction::LineVoided,
        "order_line",
        line_id.as_uuid(),
        Some(serde_json::json!({
            "status": before.status,
            "dish_name": before.dish_name,
            "line_total": before.line_total,
        })),
        Some(serde_json::json!({
            "status": LineStatus::Voided,
            "void_reason": reason,
        })),
    )
    .await?;

    finish_line_write(tx, line_id, OrderRoundId::from_uuid(updated.round_id)).await
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

/// Refuses a table that is archived or is not this restaurant's.
async fn require_live_table(tx: &mut ScopedTx<'_>, table_id: DiningTableId) -> DomainResult<()> {
    let live = sqlx::query!(
        "SELECT id FROM dining_tables WHERE id = $1 AND archived_at IS NULL",
        table_id.as_uuid()
    )
    .fetch_optional(tx.connection())
    .await?;

    if live.is_none() {
        return Err(DomainError::Invalid(
            "that table is not in use right now".to_owned(),
        ));
    }

    Ok(())
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
        r#"SELECT status AS "status: LineStatus" FROM order_lines WHERE id = $1"#,
        line_id.as_uuid()
    )
    .fetch_optional(tx.connection())
    .await;

    match found {
        Ok(Some(row)) => DomainError::Conflict(format!(
            "that dish is already {}, not {}",
            row.status.as_label(),
            expected.as_label()
        )),
        Ok(None) => DomainError::NotFound,
        Err(error) => DomainError::from(error),
    }
}

/// The same, for a visit.
async fn visit_conflict(tx: &mut ScopedTx<'_>, visit_id: VisitId, expected: &str) -> DomainError {
    let found = sqlx::query!(
        r#"SELECT status AS "status: VisitStatus" FROM visits WHERE id = $1"#,
        visit_id.as_uuid()
    )
    .fetch_optional(tx.connection())
    .await;

    match found {
        Ok(Some(row)) => DomainError::Conflict(format!(
            "that visit is already {}, not {expected}",
            row.status.as_label()
        )),
        Ok(None) => DomainError::NotFound,
        Err(error) => DomainError::from(error),
    }
}
