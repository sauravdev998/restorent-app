//! The waiter's view of one table, and the end of the meal.
//!
//! Two endpoints. One reads the whole visit as a document: its rounds, their
//! dishes, and the bill's running figures. The other ends it.
//!
//! **Nothing here computes money.** Not a subtotal, not a tax, not a rounding.
//! Every figure comes out of `close_bill`, which spec 0003 built and which is
//! the only place in the product that decides what a customer is charged. This
//! layer opens a transaction and shapes the answer.

use axum::Json;
use axum::extract::{Path, State};
use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::domain::error::DomainError;
use crate::domain::ids::VisitId;
use crate::infrastructure::db::ScopedTx;
use crate::infrastructure::db::repository::{accounts, billing, service};
use crate::presentation::dto::{BillDto, OrderRoundDto};
use crate::presentation::error::{ApiError, ErrorBody};
use crate::presentation::extract::{Actor, Waiter};
use crate::presentation::state::AppState;

/// One table's whole meal, as the waiter's screen reads it.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct VisitResponse {
    /// Which visit this is.
    pub id: Uuid,
    /// Which table they are at.
    pub table_id: Uuid,
    /// What the staff call that table.
    pub table_label: String,
    /// Whether they are still there: `open` or `closed`.
    pub status: String,
    /// How many of them, when the waiter recorded it.
    pub guest_count: Option<i16>,
    /// What to call the waiter who seated them.
    pub opened_by: String,
    /// When they sat down.
    pub opened_at: DateTime<Utc>,
    /// Every ticket on the visit, oldest first, with its dishes.
    pub rounds: Vec<OrderRoundDto>,
    /// The bill, with its running subtotal while open and every figure once
    /// closed. `null` only for a visit that somehow has none.
    pub bill: Option<BillDto>,
    /// What the server's clock reads, at the moment this answer was built. The
    /// screen corrects every age it draws by the difference from its own clock.
    pub server_time: DateTime<Utc>,
}

/// One table's whole meal.
///
/// # Errors
///
/// Returns `404` if there is no such visit in this restaurant, which is also
/// the answer for another restaurant's visit so the two cannot be told apart,
/// `401` if nobody is signed in, and `403` if the caller is not a waiter.
#[utoipa::path(
    get,
    path = "/api/visits/{id}",
    tag = "orders",
    params(("id" = Uuid, Path, description = "The visit to read.")),
    responses(
        (status = 200, description = "The visit, its rounds, and its bill. Waiters only.", body = VisitResponse),
        (status = 401, description = "Nobody is signed in.", body = ErrorBody),
        (status = 403, description = "Not a waiter.", body = ErrorBody),
        (status = 404, description = "No such visit.", body = ErrorBody),
    )
)]
pub async fn visit(
    State(state): State<AppState>,
    actor: Actor<Waiter>,
    Path(visit_id): Path<Uuid>,
) -> Result<Json<VisitResponse>, ApiError> {
    let visit_id = VisitId::from_uuid(visit_id);

    // A snapshot, and this is the read the isolation level was raised for. The
    // document is a dozen statements, and under the default level a chef's tap
    // landing between two of them produced a ticket reading "cooking" with
    // every dish on it reading "ready". A waiter watching that screen was never
    // told the food was up.
    let mut tx = state
        .database
        .begin_scoped_snapshot(actor.restaurant_id())
        .await?;

    let visit = service::visit(&mut tx, visit_id).await?;
    let table = service::table_label(&mut tx, visit.table_id).await?;
    let opener = accounts::staff_by_id(&mut tx, visit.opened_by_staff_id).await?;
    let rounds = service::rounds_for_visit(&mut tx, visit_id).await?;
    let bill = latest_bill(&mut tx, visit_id).await?;

    tx.commit().await?;

    Ok(Json(VisitResponse {
        id: visit.id.as_uuid(),
        table_id: visit.table_id.as_uuid(),
        table_label: table,
        status: visit.status.as_label().to_owned(),
        guest_count: visit.guest_count,
        opened_by: opener.display_name,
        opened_at: visit.opened_at,
        rounds: rounds
            .into_iter()
            .map(|(round, lines)| OrderRoundDto::new(&round, lines))
            .collect(),
        bill,
        server_time: Utc::now(),
    }))
}

/// Ends the meal: closes the bill, then closes the visit.
///
/// In that order and in one transaction. `close_visit` refuses while a bill on
/// the visit is still open, so the order is not a preference; and one
/// transaction is what stops a bill being numbered and totalled while its table
/// stays occupied for ever because the second write failed.
///
/// # Errors
///
/// Returns `409 bill_has_unserved_lines` if a dish has not reached the table,
/// `409 bill_already_closed` if somebody closed it first, `409
/// bill_has_no_lines` if nothing was ordered, `404` if there is no such visit or
/// it has no bill, `401` if nobody is signed in, and `403` if the caller is not
/// a waiter.
#[utoipa::path(
    post,
    path = "/api/visits/{id}/close",
    tag = "orders",
    params(("id" = Uuid, Path, description = "The visit to close.")),
    responses(
        (status = 200, description = "The closed bill, with its number and every figure. Waiters only.", body = BillDto),
        (status = 401, description = "Nobody is signed in.", body = ErrorBody),
        (status = 403, description = "Not a waiter.", body = ErrorBody),
        (status = 404, description = "No such visit, or it has no bill.", body = ErrorBody),
        (status = 409, description = "A dish is still out, or the bill has already closed.", body = ErrorBody),
    )
)]
pub async fn close_visit(
    State(state): State<AppState>,
    actor: Actor<Waiter>,
    Path(visit_id): Path<Uuid>,
) -> Result<Json<BillDto>, ApiError> {
    let visit_id = VisitId::from_uuid(visit_id);

    let mut tx = state.database.begin_scoped(actor.restaurant_id()).await?;

    // Read through the visit rather than taking a bill id from the caller. A
    // bill identifier in the body would be one more thing a client could name,
    // and there is nothing it could usefully say that the visit does not
    // already decide.
    let bill_id = service::open_bill_of(&mut tx, visit_id)
        .await?
        .ok_or(DomainError::NotFound)?;

    let closed = billing::close_bill(&mut tx, bill_id, actor.staff_id()).await?;
    service::close_visit(&mut tx, visit_id).await?;

    let taxes = billing::bill_taxes(&mut tx, bill_id).await?;

    tx.commit().await?;

    Ok(Json(BillDto::new(&closed, taxes)))
}

/// The visit's bill: the open one while the meal is on, the closed one after.
///
/// A visit has exactly one in this slice, because opening a table creates one
/// and nothing else creates another. Written as "the open one, else the most
/// recent" rather than "the one" so that splitting a bill, which feature 23
/// owns, changes this function instead of contradicting an assertion.
async fn latest_bill(
    tx: &mut ScopedTx<'_>,
    visit_id: VisitId,
) -> Result<Option<BillDto>, DomainError> {
    let Some(bill_id) = billing::latest_bill_of(tx, visit_id).await? else {
        return Ok(None);
    };

    let bill = billing::bill(tx, bill_id).await?;
    let taxes = billing::bill_taxes(tx, bill_id).await?;

    Ok(Some(BillDto::new(&bill, taxes)))
}
