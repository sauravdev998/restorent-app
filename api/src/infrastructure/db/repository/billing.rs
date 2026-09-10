//! Bills: opening one, deciding which dishes go on it, closing it, and
//! recording that it was paid.
//!
//! Closing is the one operation in this schema that turns live references into a
//! document. Before it, a bill points at lines and settings that can still move.
//! After it, every figure and every name on it is a copy, and nothing anybody
//! does to the menu, the tax rules, or the service charge can reach back and
//! change what the customer was charged.
//!
//! Both operations that touch a bill's figures take the bill's row lock first,
//! so a close can never snapshot totals while lines are being reassigned
//! underneath it.

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde_json::json;
use uuid::Uuid;

use crate::domain::audit::AuditAction;
use crate::domain::billing::{Bill, BillTax, Payment};
use crate::domain::enums::{BillStatus, PaymentMethod, VisitStatus};
use crate::domain::error::{ConflictKind, DomainError, DomainResult};
use crate::domain::event::EntityKind;
use crate::domain::ids::{BillId, BillTaxId, OrderLineId, PaymentId, StaffId, VisitId};
use crate::domain::money::{Currency, apply_percent};

use super::super::{Database, ScopedTx};
use super::catalog;

/// Reads one bill.
///
/// # Errors
///
/// Returns [`DomainError::NotFound`] if no such bill belongs to this restaurant.
pub async fn bill(tx: &mut ScopedTx<'_>, bill_id: BillId) -> DomainResult<Bill> {
    let row = sqlx::query!(
        r#"
        SELECT id, visit_id, number, status AS "status: BillStatus",
               currency_code, currency_decimals, subtotal, service_charge_percent,
               service_charge_amount, tax_total, total,
               opened_by_staff_id, closed_by_staff_id, closed_at
        FROM bills
        WHERE id = $1
        "#,
        bill_id.as_uuid()
    )
    .fetch_optional(tx.connection())
    .await?
    .ok_or(DomainError::NotFound)?;

    let decimals = u32::try_from(row.currency_decimals).map_err(|_| {
        DomainError::Invalid("a bill stored a negative number of currency decimals".to_owned())
    })?;

    Ok(Bill {
        id: BillId::from_uuid(row.id),
        visit_id: VisitId::from_uuid(row.visit_id),
        number: row.number,
        status: row.status,
        currency: Currency::new(row.currency_code.trim(), decimals)?,
        subtotal: row.subtotal,
        service_charge_percent: row.service_charge_percent,
        service_charge_amount: row.service_charge_amount,
        tax_total: row.tax_total,
        total: row.total,
        opened_by_staff_id: StaffId::from_uuid(row.opened_by_staff_id),
        closed_by_staff_id: row.closed_by_staff_id.map(StaffId::from_uuid),
        closed_at: row.closed_at,
    })
}

/// The tax breakdown printed on a bill, in the order it was copied.
///
/// These are copies made at close, not references. Editing the restaurant's tax
/// rules afterwards leaves them exactly as they are.
///
/// # Errors
///
/// Returns [`DomainError::Unavailable`] if the read fails.
pub async fn bill_taxes(tx: &mut ScopedTx<'_>, bill_id: BillId) -> DomainResult<Vec<BillTax>> {
    let rows = sqlx::query!(
        r#"
        SELECT id, name, rate_percent, amount
        FROM bill_taxes
        WHERE bill_id = $1
        ORDER BY name
        "#,
        bill_id.as_uuid()
    )
    .fetch_all(tx.connection())
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| BillTax {
            id: BillTaxId::from_uuid(row.id),
            bill_id,
            name: row.name,
            rate_percent: row.rate_percent,
            amount: row.amount,
        })
        .collect())
}

/// The visit's bill: the open one while the meal is on, the most recent after.
///
/// A visit has exactly one in this slice, because opening a table creates one
/// and nothing else creates another. Written as an ordering rather than an
/// assertion so that splitting a bill, which feature 23 owns, changes what this
/// returns instead of contradicting a promise made here.
///
/// # Errors
///
/// Returns [`DomainError::Unavailable`] if the read fails.
pub async fn latest_bill_of(
    tx: &mut ScopedTx<'_>,
    visit_id: VisitId,
) -> DomainResult<Option<BillId>> {
    let found = sqlx::query!(
        r#"
        SELECT id
        FROM bills
        WHERE visit_id = $1
        ORDER BY (status = 'open') DESC, created_at DESC
        LIMIT 1
        "#,
        visit_id.as_uuid()
    )
    .fetch_optional(tx.connection())
    .await?;

    Ok(found.map(|row| BillId::from_uuid(row.id)))
}

/// Which local day a closed bill belongs to.
///
/// Worked out from the restaurant's own timezone, never the server's. A
/// restaurant that stops serving at one in the morning would otherwise post half
/// of Friday evening to Saturday, and its owner would find the daily takings
/// split across two days for reasons nobody could explain.
///
/// Returns [`None`] for a bill that has not closed.
///
/// # Errors
///
/// Returns [`DomainError::NotFound`] if no such bill belongs to this restaurant.
pub async fn bill_local_day(
    tx: &mut ScopedTx<'_>,
    bill_id: BillId,
) -> DomainResult<Option<NaiveDate>> {
    let row = sqlx::query!(
        r#"
        SELECT (b.closed_at AT TIME ZONE r.timezone)::date AS "local_day?"
        FROM bills AS b
        JOIN restaurants AS r ON r.id = b.restaurant_id
        WHERE b.id = $1
        "#,
        bill_id.as_uuid()
    )
    .fetch_optional(tx.connection())
    .await?
    .ok_or(DomainError::NotFound)?;

    Ok(row.local_day)
}

/// Opens a bill against a visit, with every figure at zero.
///
/// Zero rather than null, so no screen ever has to decide what a missing total
/// means. The currency is stamped on now because the column cannot be empty, and
/// stamped again at close, which is the moment that decides what the customer
/// actually paid in.
///
/// # Errors
///
/// Returns [`DomainError::Conflict`] if the party has already left, and
/// [`DomainError::NotFound`] if there is no such visit.
pub async fn open_bill(
    tx: &mut ScopedTx<'_>,
    visit_id: VisitId,
    opened_by: StaffId,
) -> DomainResult<Bill> {
    let visit = sqlx::query!(
        r#"SELECT status AS "status: VisitStatus" FROM visits WHERE id = $1"#,
        visit_id.as_uuid()
    )
    .fetch_optional(tx.connection())
    .await?
    .ok_or(DomainError::NotFound)?;

    if visit.status != VisitStatus::Open {
        return Err(DomainError::Conflict(ConflictKind::VisitNot(
            VisitStatus::Open,
        )));
    }

    let restaurant = catalog::restaurant(tx).await?;
    let restaurant_id = tx.restaurant_id().as_uuid();
    let bill_id = BillId::new();
    let decimals = i16::try_from(restaurant.currency.decimals()).unwrap_or(0);

    sqlx::query!(
        r#"
        INSERT INTO bills
            (id, restaurant_id, visit_id, status, currency_code, currency_decimals,
             subtotal, service_charge_amount, tax_total, total, opened_by_staff_id, updated_at)
        VALUES ($1, $2, $3, 'open', $4, $5, 0, 0, 0, 0, $6, now())
        "#,
        bill_id.as_uuid(),
        restaurant_id,
        visit_id.as_uuid(),
        restaurant.currency.code(),
        decimals,
        opened_by.as_uuid(),
    )
    .execute(tx.connection())
    .await?;

    Database::notify_entity_change(tx, EntityKind::Bill, bill_id.as_uuid()).await?;

    bill(tx, bill_id).await
}

/// Decides which dishes go on which bill.
///
/// Moving a line between two bills of the same restaurant is allowed and
/// recomputes both, which is the mechanism a future split or merge would use.
/// Moving one to another restaurant's bill is refused by the composite foreign
/// key, not by anything written here.
///
/// Every bill involved is locked in identifier order before anything is written.
/// The order is what stops two waiters moving lines between the same two bills
/// in opposite directions from deadlocking.
///
/// # Errors
///
/// Returns [`DomainError::Conflict`] if the target bill or any bill a line is
/// moving off has already closed, and [`DomainError::NotFound`] if a line does
/// not belong to this restaurant.
pub async fn assign_lines_to_bill(
    tx: &mut ScopedTx<'_>,
    bill_id: BillId,
    line_ids: &[OrderLineId],
) -> DomainResult<Bill> {
    if line_ids.is_empty() {
        return bill(tx, bill_id).await;
    }

    let raw_line_ids: Vec<Uuid> = line_ids.iter().map(|id| id.as_uuid()).collect();

    let sources = sqlx::query!(
        r#"
        SELECT DISTINCT bill_id AS "bill_id!"
        FROM order_lines
        WHERE id = ANY($1) AND bill_id IS NOT NULL
        "#,
        &raw_line_ids
    )
    .fetch_all(tx.connection())
    .await?;

    let mut affected: Vec<Uuid> = sources.into_iter().map(|row| row.bill_id).collect();
    if !affected.contains(&bill_id.as_uuid()) {
        affected.push(bill_id.as_uuid());
    }
    affected.sort_unstable();

    let locked = sqlx::query!(
        r#"
        SELECT id, status AS "status: BillStatus"
        FROM bills
        WHERE id = ANY($1)
        ORDER BY id
        FOR UPDATE
        "#,
        &affected
    )
    .fetch_all(tx.connection())
    .await?;

    if !locked.iter().any(|row| row.id == bill_id.as_uuid()) {
        return Err(DomainError::NotFound);
    }

    for row in &locked {
        if row.status != BillStatus::Open {
            return Err(DomainError::Conflict(if row.id == bill_id.as_uuid() {
                ConflictKind::BillNotOpen
            } else {
                ConflictKind::LineOnClosedBill
            }));
        }
    }

    let moved = sqlx::query!(
        r#"
        UPDATE order_lines
        SET bill_id = $1, updated_at = now()
        WHERE id = ANY($2)
        RETURNING id
        "#,
        bill_id.as_uuid(),
        &raw_line_ids
    )
    .fetch_all(tx.connection())
    .await?;

    if moved.len() != line_ids.len() {
        return Err(DomainError::NotFound);
    }

    // Both ends move, not just the destination, or a line that left a bill would
    // stay in that bill's total for ever.
    for id in affected {
        recompute_subtotal(tx, BillId::from_uuid(id)).await?;
        Database::notify_entity_change(tx, EntityKind::Bill, id).await?;
    }

    bill(tx, bill_id).await
}

/// Adds up a bill's assigned dishes and stores the result.
///
/// The subtotal is the only figure that moves before a bill closes. The service
/// charge, the taxes, and the total stay at zero until the close computes all
/// three at once, so no screen can ever read a half computed money figure.
async fn recompute_subtotal(tx: &mut ScopedTx<'_>, bill_id: BillId) -> DomainResult<Decimal> {
    let currency = bill(tx, bill_id).await?.currency;

    let raw = sqlx::query!(
        r#"
        SELECT coalesce(sum(line_total), 0) AS "subtotal!"
        FROM order_lines
        WHERE bill_id = $1 AND status <> 'voided'
        "#,
        bill_id.as_uuid()
    )
    .fetch_one(tx.connection())
    .await?
    .subtotal;

    let subtotal = currency.round(raw);

    sqlx::query!(
        "UPDATE bills SET subtotal = $2, updated_at = now() WHERE id = $1",
        bill_id.as_uuid(),
        subtotal,
    )
    .execute(tx.connection())
    .await?;

    Ok(subtotal)
}

/// Closes a bill: numbers it, totals it, and freezes it.
///
/// The order of the work matters. Every figure is rounded once, to the bill's
/// own currency, and the total is the sum of figures that are already rounded,
/// so the arithmetic a customer can do on the printed receipt comes out exactly.
/// Rounding the total separately would leave the odd penny of residue that makes
/// somebody recount their change.
///
/// The number is allocated last, because it is the one thing that cannot be
/// taken back. If anything above it fails, the transaction rolls back and the
/// number is released, which is what keeps the sequence gapless.
///
/// # Errors
///
/// Returns [`DomainError::Conflict`] if the bill has already closed, if a dish
/// on it has neither reached the table nor been cancelled, or if it has no
/// dishes on it that count.
pub async fn close_bill(
    tx: &mut ScopedTx<'_>,
    bill_id: BillId,
    closed_by: StaffId,
) -> DomainResult<Bill> {
    let locked = sqlx::query!(
        r#"SELECT status AS "status: BillStatus" FROM bills WHERE id = $1 FOR UPDATE"#,
        bill_id.as_uuid()
    )
    .fetch_optional(tx.connection())
    .await?
    .ok_or(DomainError::NotFound)?;

    if locked.status != BillStatus::Open {
        return Err(DomainError::Conflict(ConflictKind::BillAlreadyClosed));
    }

    let figures = closing_figures(tx, bill_id).await?;

    // Last, because it is the one thing that cannot be taken back.
    let number = allocate_bill_number(tx).await?;

    write_bill_taxes(tx, bill_id, &figures.taxes).await?;
    stamp_closed(tx, bill_id, closed_by, number, &figures).await?;

    let closed = bill(tx, bill_id).await?;

    super::audit::record(
        tx,
        Some(closed_by),
        AuditAction::BillClosed,
        "bill",
        bill_id.as_uuid(),
        Some(json!({ "status": BillStatus::Open })),
        Some(json!({
            "status": BillStatus::Closed,
            "number": number,
            "subtotal": figures.subtotal,
            "service_charge_amount": figures.service_charge_amount,
            "tax_total": figures.tax_total,
            "total": figures.total,
            "currency_code": figures.currency.code(),
        })),
    )
    .await?;

    Database::notify_entity_change(tx, EntityKind::Bill, bill_id.as_uuid()).await?;

    Ok(closed)
}

/// Every figure a close is about to stamp onto a bill, worked out but not yet
/// written.
struct ClosingFigures {
    currency: Currency,
    subtotal: Decimal,
    service_charge_percent: Option<Decimal>,
    service_charge_amount: Decimal,
    tax_total: Decimal,
    total: Decimal,
    /// Name, rate, and amount for each tax, in the order they will print.
    taxes: Vec<(String, Decimal, Decimal)>,
}

/// Works out what the bill comes to, and refuses to if it should not close.
///
/// Every figure is rounded once, to the bill's own currency, and the total is
/// then the sum of figures that are already rounded. That is what makes the
/// arithmetic on the printed receipt come out exactly: rounding the total
/// separately would leave the odd penny of residue that makes a customer recount
/// their change.
async fn closing_figures(tx: &mut ScopedTx<'_>, bill_id: BillId) -> DomainResult<ClosingFigures> {
    let lines = sqlx::query!(
        r#"
        SELECT
            count(*) FILTER (WHERE status NOT IN ('served', 'voided')) AS "still_out!",
            count(*) FILTER (WHERE status <> 'voided') AS "countable!",
            coalesce(sum(line_total) FILTER (WHERE status <> 'voided'), 0) AS "raw_subtotal!"
        FROM order_lines
        WHERE bill_id = $1
        "#,
        bill_id.as_uuid()
    )
    .fetch_one(tx.connection())
    .await?;

    if lines.still_out > 0 {
        return Err(DomainError::Conflict(ConflictKind::BillHasUnservedLines));
    }

    if lines.countable == 0 {
        // Refused rather than closed at zero, so an empty bill never consumes a
        // number and the sequence stays gapless.
        return Err(DomainError::Conflict(ConflictKind::BillHasNoLines));
    }

    let restaurant = catalog::restaurant(tx).await?;
    let currency = restaurant.currency.clone();
    let subtotal = currency.round(lines.raw_subtotal);

    let service_charge_amount = restaurant
        .service_charge_percent
        .map_or(Decimal::ZERO, |percent| {
            currency.round(apply_percent(subtotal, percent))
        });

    let components = catalog::live_tax_components(tx).await?;
    let mut tax_total = Decimal::ZERO;
    let mut taxes = Vec::with_capacity(components.len());

    for component in components {
        let amount = currency.round(apply_percent(subtotal, component.rate_percent));
        tax_total += amount;
        taxes.push((component.name, component.rate_percent, amount));
    }

    Ok(ClosingFigures {
        subtotal,
        service_charge_percent: restaurant.service_charge_percent,
        service_charge_amount,
        tax_total,
        total: subtotal + service_charge_amount + tax_total,
        currency,
        taxes,
    })
}

/// Copies the tax breakdown onto the bill.
///
/// Copies, not references: this is the row a reprint reads a year from now.
async fn write_bill_taxes(
    tx: &mut ScopedTx<'_>,
    bill_id: BillId,
    taxes: &[(String, Decimal, Decimal)],
) -> DomainResult<()> {
    let restaurant_id = tx.restaurant_id().as_uuid();

    for (name, rate_percent, amount) in taxes {
        sqlx::query!(
            r#"
            INSERT INTO bill_taxes
                (id, restaurant_id, bill_id, name, rate_percent, amount, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, now())
            "#,
            BillTaxId::new().as_uuid(),
            restaurant_id,
            bill_id.as_uuid(),
            name,
            rate_percent,
            amount,
        )
        .execute(tx.connection())
        .await?;
    }

    Ok(())
}

/// Writes the figures onto the bill and marks it closed.
///
/// The currency is stamped again here, rather than trusted from when the bill
/// was opened, because close is the moment that decides what the customer
/// actually paid in.
async fn stamp_closed(
    tx: &mut ScopedTx<'_>,
    bill_id: BillId,
    closed_by: StaffId,
    number: i64,
    figures: &ClosingFigures,
) -> DomainResult<()> {
    let decimals = i16::try_from(figures.currency.decimals()).unwrap_or(0);

    sqlx::query!(
        r#"
        UPDATE bills
        SET status = 'closed',
            number = $2,
            currency_code = $3,
            currency_decimals = $4,
            subtotal = $5,
            service_charge_percent = $6,
            service_charge_amount = $7,
            tax_total = $8,
            total = $9,
            closed_by_staff_id = $10,
            closed_at = now(),
            updated_at = now()
        WHERE id = $1
        "#,
        bill_id.as_uuid(),
        number,
        figures.currency.code(),
        decimals,
        figures.subtotal,
        figures.service_charge_percent,
        figures.service_charge_amount,
        figures.tax_total,
        figures.total,
        closed_by.as_uuid(),
    )
    .execute(tx.connection())
    .await?;

    Ok(())
}

/// Takes the next bill number for this restaurant.
///
/// One statement, and it is an upsert rather than an update so that a restaurant
/// whose counter row was never created gets one rather than matching zero rows
/// and silently returning nothing.
///
/// The row stays locked until the transaction ends, which is what makes the
/// sequence gapless: a transaction that rolls back releases the number for the
/// next bill instead of leaving a hole. The cost is that two tills closing bills
/// in the same restaurant at the same second queue behind each other, which at
/// one restaurant's volume is not a cost at all.
async fn allocate_bill_number(tx: &mut ScopedTx<'_>) -> DomainResult<i64> {
    let restaurant_id = tx.restaurant_id().as_uuid();

    let allocated = sqlx::query!(
        r#"
        INSERT INTO bill_number_counters (restaurant_id, next_number, updated_at)
        VALUES ($1, 1, now())
        ON CONFLICT (restaurant_id) DO UPDATE
            SET next_number = bill_number_counters.next_number + 1,
                updated_at = now()
        RETURNING next_number
        "#,
        restaurant_id
    )
    .fetch_one(tx.connection())
    .await?;

    Ok(allocated.next_number)
}

/// Records that a closed bill was paid.
///
/// # Errors
///
/// Returns [`DomainError::Conflict`] if the bill has not closed yet, and
/// [`DomainError::Invalid`] if the amount is not positive.
pub async fn record_payment(
    tx: &mut ScopedTx<'_>,
    bill_id: BillId,
    method: PaymentMethod,
    amount: Decimal,
    taken_by: StaffId,
) -> DomainResult<Payment> {
    if amount <= Decimal::ZERO {
        return Err(DomainError::Invalid(
            "a payment has to be for more than nothing".to_owned(),
        ));
    }

    let target = sqlx::query!(
        r#"SELECT status AS "status: BillStatus" FROM bills WHERE id = $1"#,
        bill_id.as_uuid()
    )
    .fetch_optional(tx.connection())
    .await?
    .ok_or(DomainError::NotFound)?;

    if target.status != BillStatus::Closed {
        return Err(DomainError::Conflict(ConflictKind::BillNotClosed));
    }

    let restaurant_id = tx.restaurant_id().as_uuid();
    let payment_id = PaymentId::new();

    let row = sqlx::query!(
        r#"
        INSERT INTO payments
            (id, restaurant_id, bill_id, method, amount, taken_by_staff_id, taken_at,
             note, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, now(), NULL, now())
        RETURNING taken_at
        "#,
        payment_id.as_uuid(),
        restaurant_id,
        bill_id.as_uuid(),
        method as PaymentMethod,
        amount,
        taken_by.as_uuid(),
    )
    .fetch_one(tx.connection())
    .await?;

    Database::notify_entity_change(tx, EntityKind::Bill, bill_id.as_uuid()).await?;

    Ok(Payment {
        id: payment_id,
        bill_id,
        method,
        amount,
        taken_by_staff_id: taken_by,
        taken_at: row.taken_at,
        note: None,
    })
}
