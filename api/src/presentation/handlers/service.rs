//! The service loop over HTTP: the floor, seating a party, sending a ticket,
//! the kitchen queue, and moving one dish along.
//!
//! Every handler here is thin on purpose. The rules about who may act, what a
//! conditional update expects, and what a ticket's status becomes all live
//! below this layer; what these add is the transaction boundary and the shape
//! that goes on the wire.
//!
//! **Two writes that belong together share one transaction.** Opening a table
//! creates a visit and its bill; sending a ticket creates the round and puts
//! its lines on the bill. Each pair is one `ScopedTx`, so a half opened table
//! with no bill on it, or a ticket whose dishes are on no bill at all, cannot
//! exist for even an instant.
//!
//! **The role is in the signature.** `Actor<Waiter>` and `Actor<Chef>` refuse
//! before the handler body runs, and they reach the `OpenAPI` document, so who
//! may call what is visible to whoever is building the screen.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::domain::enums::{LineStatus, RoundStatus};
use crate::domain::error::{ConflictKind, DomainError, DomainResult};
use crate::domain::ids::{
    DiningTableId, DishId, OrderLineId, OrderRoundId, TableSectionId, VisitId,
};
use crate::domain::service::{FloorTable, NewOrderLine};
use crate::infrastructure::db::ScopedTx;
use crate::infrastructure::db::repository::{billing, catalog, service};
use crate::presentation::dto::{LineStatusDto, OrderLineDto, OrderRoundDto, RoundStatusDto};
use crate::presentation::error::{ApiError, ErrorBody};
use crate::presentation::extract::{Actor, Chef, JsonBody, Waiter};
use crate::presentation::state::AppState;

// ===========================================================================
// The floor
// ===========================================================================

/// Every table in the restaurant, grouped by section.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FloorResponse {
    /// The sections, in displayed order, each with its tables.
    pub sections: Vec<FloorSectionDto>,
}

/// One group of tables.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FloorSectionDto {
    /// Which section this is, or `null` for the group holding tables that
    /// belong to no live section.
    pub id: Option<Uuid>,
    /// What it is called, or `null` for that same group. A screen shows those
    /// tables under a heading of its own choosing, in its own language.
    pub name: Option<String>,
    /// Its tables, in displayed order.
    pub tables: Vec<FloorTableDto>,
}

/// One table, and whoever is sitting at it.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FloorTableDto {
    /// Which table this is.
    pub id: Uuid,
    /// What the staff call it, such as `12` or `Bar 3`.
    pub label: String,
    /// How many it seats, when the restaurant recorded it.
    pub seats: Option<i16>,
    /// Who is at it, or `null` when it is free.
    pub occupancy: Option<OccupancyDto>,
}

/// What a waiter needs to know about an occupied table without opening it.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct OccupancyDto {
    /// The one open visit on this table.
    pub visit_id: Uuid,
    /// What to call the waiter who seated them.
    pub opened_by: String,
    /// When they sat down.
    pub opened_at: DateTime<Utc>,
    /// How many of them, when the waiter recorded it.
    pub guest_count: Option<i16>,
    /// Whether a ticket on this visit is waiting to be carried out.
    pub food_ready: bool,
}

/// The floor, in the order it is walked.
///
/// # Errors
///
/// Returns `401` if nobody is signed in, `403` if the caller is not a waiter,
/// and `503` if the database is unavailable.
#[utoipa::path(
    get,
    path = "/api/floor",
    tag = "orders",
    responses(
        (status = 200, description = "Every live table and its occupancy. Waiters only.", body = FloorResponse),
        (status = 401, description = "Nobody is signed in.", body = ErrorBody),
        (status = 403, description = "Not a waiter.", body = ErrorBody),
    )
)]
pub async fn floor(
    State(state): State<AppState>,
    actor: Actor<Waiter>,
) -> Result<Json<FloorResponse>, ApiError> {
    // A snapshot, because the answer is two statements and a table opened
    // between them would appear in one and not the other.
    let mut tx = state
        .database
        .begin_scoped_snapshot(actor.restaurant_id())
        .await?;

    let sections = catalog::live_table_sections(&mut tx).await?;
    let tables = service::floor(&mut tx).await?;

    tx.commit().await?;

    let mut grouped: Vec<FloorSectionDto> = sections
        .into_iter()
        .map(|section| FloorSectionDto {
            id: Some(section.id.as_uuid()),
            name: Some(section.name),
            tables: tables_in(&tables, Some(section.id.as_uuid())),
        })
        .filter(|section| !section.tables.is_empty())
        .collect();

    // A table can belong to no section at all, or to one that has since been
    // archived. It is still a table somebody sits at, so it goes into a group of
    // its own at the end rather than disappearing from the floor.
    let named: Vec<Uuid> = grouped.iter().filter_map(|section| section.id).collect();

    let loose: Vec<FloorTableDto> = tables
        .iter()
        .filter(|floor_table| {
            floor_table
                .table
                .section_id
                .is_none_or(|id| !named.contains(&id.as_uuid()))
        })
        .map(table_dto)
        .collect();

    if !loose.is_empty() {
        grouped.push(FloorSectionDto {
            id: None,
            name: None,
            tables: loose,
        });
    }

    Ok(Json(FloorResponse { sections: grouped }))
}

/// The tables belonging to one section, in the order the read returned them.
fn tables_in(tables: &[FloorTable], section: Option<Uuid>) -> Vec<FloorTableDto> {
    tables
        .iter()
        .filter(|floor_table| {
            floor_table
                .table
                .section_id
                .map(TableSectionId::as_uuid)
                .eq(&section)
        })
        .map(table_dto)
        .collect()
}

/// One table on the wire.
fn table_dto(floor_table: &FloorTable) -> FloorTableDto {
    FloorTableDto {
        id: floor_table.table.id.as_uuid(),
        label: floor_table.table.label.clone(),
        seats: floor_table.table.seats,
        occupancy: floor_table
            .occupancy
            .as_ref()
            .map(|occupancy| OccupancyDto {
                visit_id: occupancy.visit_id.as_uuid(),
                opened_by: occupancy.opened_by.clone(),
                opened_at: occupancy.opened_at,
                guest_count: occupancy.guest_count,
                food_ready: occupancy.food_ready,
            }),
    }
}

// ===========================================================================
// Opening a table
// ===========================================================================

/// What seating a party asks for.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct OpenVisitRequest {
    /// Which table they are sitting at.
    pub table_id: Uuid,
    /// How many of them, if the waiter counted.
    #[serde(default)]
    pub guest_count: Option<i16>,
}

/// The visit and the bill that were created together.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct OpenVisitResponse {
    /// The party's stay at the table.
    pub visit_id: Uuid,
    /// The bill their dishes will go on.
    pub bill_id: Uuid,
}

/// Seats a party and opens their bill, in one transaction.
///
/// Both or neither. A visit with no bill would be a table that could take an
/// order nobody could be charged for, and it would be discovered at the end of
/// the meal.
///
/// # Errors
///
/// Returns `409 table_occupied` if somebody else opened that table first, `400`
/// if the table is archived or not this restaurant's, `401` if nobody is signed
/// in, and `403` if the caller is not a waiter.
#[utoipa::path(
    post,
    path = "/api/visits",
    tag = "orders",
    request_body = OpenVisitRequest,
    responses(
        (status = 201, description = "The visit and its bill. Waiters only.", body = OpenVisitResponse),
        (status = 400, description = "No such live table.", body = ErrorBody),
        (status = 401, description = "Nobody is signed in.", body = ErrorBody),
        (status = 403, description = "Not a waiter.", body = ErrorBody),
        (status = 409, description = "That table already has a party at it.", body = ErrorBody),
    )
)]
pub async fn open_visit(
    State(state): State<AppState>,
    actor: Actor<Waiter>,
    JsonBody(request): JsonBody<OpenVisitRequest>,
) -> Result<(StatusCode, Json<OpenVisitResponse>), ApiError> {
    if request.guest_count.is_some_and(|count| count <= 0) {
        return Err(DomainError::Invalid("a party is at least one person".to_owned()).into());
    }

    let mut tx = state.database.begin_scoped(actor.restaurant_id()).await?;

    let visit = service::open_visit(
        &mut tx,
        DiningTableId::from_uuid(request.table_id),
        actor.staff_id(),
        request.guest_count,
    )
    .await?;

    let bill = billing::open_bill(&mut tx, visit.id, actor.staff_id()).await?;

    tx.commit().await?;

    Ok((
        StatusCode::CREATED,
        Json(OpenVisitResponse {
            visit_id: visit.id.as_uuid(),
            bill_id: bill.id.as_uuid(),
        }),
    ))
}

// ===========================================================================
// Sending a ticket
// ===========================================================================

/// What sending a ticket asks for.
///
/// No price anywhere in it, and that is the point. The price and the name are
/// read from the dish inside `send_round` and copied onto the line, so a client
/// cannot name its own price however it is built.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SendRoundRequest {
    /// The basket, one entry per dish. At least one.
    pub lines: Vec<SendRoundLine>,
}

/// One dish in the basket.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SendRoundLine {
    /// Which menu item.
    pub dish_id: Uuid,
    /// How many.
    pub quantity: i32,
    /// What the guest asked for, such as no onions.
    #[serde(default)]
    pub note: Option<String>,
}

/// Sends one ticket to the kitchen and puts its dishes on the bill.
///
/// One transaction, so a ticket whose lines are on no bill cannot exist. That
/// is what keeps the bill's running subtotal true throughout the meal and
/// leaves the close with no assignment left to do.
///
/// # Errors
///
/// Returns `400` if the basket is empty, a quantity is not positive, or a dish
/// is archived or currently unavailable, `409 visit_not_open` if the party has
/// left, `404` if there is no such visit, `401` if nobody is signed in, and
/// `403` if the caller is not a waiter.
#[utoipa::path(
    post,
    path = "/api/visits/{id}/rounds",
    tag = "orders",
    params(("id" = Uuid, Path, description = "The visit to send the ticket for.")),
    request_body = SendRoundRequest,
    responses(
        (status = 201, description = "The ticket that was sent. Waiters only.", body = OrderRoundDto),
        (status = 400, description = "An empty basket, or a dish that cannot be ordered.", body = ErrorBody),
        (status = 401, description = "Nobody is signed in.", body = ErrorBody),
        (status = 403, description = "Not a waiter.", body = ErrorBody),
        (status = 404, description = "No such visit.", body = ErrorBody),
        (status = 409, description = "That party has already left.", body = ErrorBody),
    )
)]
pub async fn send_round(
    State(state): State<AppState>,
    actor: Actor<Waiter>,
    Path(visit_id): Path<Uuid>,
    JsonBody(request): JsonBody<SendRoundRequest>,
) -> Result<(StatusCode, Json<OrderRoundDto>), ApiError> {
    let visit_id = VisitId::from_uuid(visit_id);

    let new_lines: Vec<NewOrderLine> = request
        .lines
        .into_iter()
        .map(|line| NewOrderLine {
            dish_id: DishId::from_uuid(line.dish_id),
            quantity: line.quantity,
            note: line.note.filter(|note| !note.trim().is_empty()),
        })
        .collect();

    let mut tx = state.database.begin_scoped(actor.restaurant_id()).await?;

    let (round, lines) =
        service::send_round(&mut tx, visit_id, actor.staff_id(), &new_lines).await?;

    // Straight onto the visit's open bill, in this same transaction. A visit
    // opened by this API always has one; a visit that somehow has none is a
    // refusal rather than a ticket nobody can be charged for.
    let bill_id = service::open_bill_of(&mut tx, visit_id)
        .await?
        .ok_or(DomainError::NotFound)?;

    let line_ids: Vec<OrderLineId> = lines.iter().map(|line| line.id).collect();
    billing::assign_lines_to_bill(&mut tx, bill_id, &line_ids).await?;

    tx.commit().await?;

    Ok((StatusCode::CREATED, Json(OrderRoundDto::new(&round, lines))))
}

// ===========================================================================
// The kitchen
// ===========================================================================

/// Everything the kitchen still has work on.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct KitchenResponse {
    /// The tickets, oldest first.
    pub tickets: Vec<KitchenTicketDto>,
    /// What the server's clock reads, at the moment this answer was built.
    ///
    /// The screen subtracts this from its own clock and corrects every age it
    /// draws by the difference. A kitchen tablet whose clock is twenty minutes
    /// fast would otherwise show every ticket as twenty minutes late, and the
    /// one number a chef acts on would be the one number nobody could trust.
    pub server_time: DateTime<Utc>,
}

/// One ticket on the kitchen screen.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct KitchenTicketDto {
    /// Which ticket this is.
    pub id: Uuid,
    /// Its number within the visit, so a chef can say "table 4, second round".
    pub sequence_no: i32,
    /// Where the food is going.
    pub table_label: String,
    /// When it reached the kitchen. The waiting time is measured from here.
    pub sent_at: DateTime<Utc>,
    /// Where the whole ticket has got to.
    pub status: RoundStatusDto,
    /// Every dish on it, oldest first.
    pub lines: Vec<KitchenLineDto>,
}

/// One dish on a kitchen ticket.
///
/// Deliberately narrower than [`OrderLineDto`]. A kitchen screen has no
/// business showing a price, and a chef tapping a dish done should not be one
/// misread column away from the bill.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct KitchenLineDto {
    /// Which line this is. What the mark ready call names.
    pub id: Uuid,
    /// What to cook.
    pub dish_name: String,
    /// How many.
    pub quantity: i32,
    /// What the guest asked for, such as no onions. Reaches the pass unchanged.
    pub note: Option<String>,
    /// Where this one dish has got to.
    pub status: LineStatusDto,
}

/// The kitchen queue, oldest first.
///
/// # Errors
///
/// Returns `401` if nobody is signed in, `403` if the caller is not a chef, and
/// `503` if the database is unavailable.
#[utoipa::path(
    get,
    path = "/api/kitchen/tickets",
    tag = "orders",
    responses(
        (status = 200, description = "Every queued or ready ticket, oldest first. Chefs only.", body = KitchenResponse),
        (status = 401, description = "Nobody is signed in.", body = ErrorBody),
        (status = 403, description = "Not a chef.", body = ErrorBody),
    )
)]
pub async fn kitchen_tickets(
    State(state): State<AppState>,
    actor: Actor<Chef>,
) -> Result<Json<KitchenResponse>, ApiError> {
    // A snapshot. The queue is one statement for the tickets and another per
    // ticket for its dishes, so without one a chef could be shown a ticket
    // whose status and whose dishes came from either side of a colleague's tap.
    let mut tx = state
        .database
        .begin_scoped_snapshot(actor.restaurant_id())
        .await?;
    let queue = service::kitchen_queue(&mut tx).await?;
    tx.commit().await?;

    let tickets = queue
        .into_iter()
        .map(|ticket| KitchenTicketDto {
            id: ticket.round.id.as_uuid(),
            sequence_no: ticket.round.sequence_no,
            table_label: ticket.table_label,
            sent_at: ticket.round.sent_at,
            status: ticket.round.status.into(),
            lines: ticket
                .lines
                .into_iter()
                .map(|line| KitchenLineDto {
                    id: line.id.as_uuid(),
                    dish_name: line.dish_name,
                    quantity: line.quantity,
                    note: line.note,
                    status: line.status.into(),
                })
                .collect(),
        })
        .collect();

    Ok(Json(KitchenResponse {
        tickets,
        server_time: Utc::now(),
    }))
}

/// What marking one dish changed.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MarkedLineResponse {
    /// The dish, after the write.
    pub line: OrderLineDto,
    /// What the whole ticket became, recomputed from all of its dishes. The
    /// kitchen screen uses this to drop a ticket that has just gone ready.
    pub round_status: RoundStatusDto,
}

/// Marks one dish off the pass.
///
/// Touches that dish and nothing else. The ticket's own status follows from all
/// of its dishes and is recomputed inside the same transaction, which is why
/// nothing here sets it.
///
/// # Errors
///
/// Returns `409 line_not_queued` if another chef marked it first, `404` if
/// there is no such dish, `401` if nobody is signed in, and `403` if the caller
/// is not a chef.
#[utoipa::path(
    post,
    path = "/api/order-lines/{id}/ready",
    tag = "orders",
    params(("id" = Uuid, Path, description = "The dish to mark off the pass.")),
    responses(
        (status = 200, description = "The dish and its ticket's new status. Chefs only.", body = MarkedLineResponse),
        (status = 401, description = "Nobody is signed in.", body = ErrorBody),
        (status = 403, description = "Not a chef.", body = ErrorBody),
        (status = 404, description = "No such dish.", body = ErrorBody),
        (status = 409, description = "Somebody marked that dish first.", body = ErrorBody),
    )
)]
pub async fn mark_line_ready(
    State(state): State<AppState>,
    actor: Actor<Chef>,
    Path(line_id): Path<Uuid>,
) -> Result<Json<MarkedLineResponse>, ApiError> {
    let mut tx = state.database.begin_scoped(actor.restaurant_id()).await?;

    let (line, round_status) =
        service::mark_line_ready(&mut tx, OrderLineId::from_uuid(line_id), actor.staff_id())
            .await?;

    tx.commit().await?;

    Ok(Json(MarkedLineResponse {
        line: line.into(),
        round_status: round_status.into(),
    }))
}

// ===========================================================================
// Carrying the food out
// ===========================================================================

/// Marks a whole ticket as having reached the table.
///
/// The one refusal here that is not a repository operation's own. It reads the
/// ticket first and refuses unless it is `ready` at that moment, because
/// "somebody has already carried this out" is what the waiter needs to be told,
/// and a line level message about one dish would not say it.
///
/// Inside the loop it marks every dish that is `ready` and skips one that is
/// already `served` or was cancelled rather than treating either as a conflict.
/// Two waiters serving the same ticket at the same instant is not a mistake
/// worth stopping the second one over; only the precheck refuses, and only
/// because by then the ticket is no longer ready.
///
/// # Errors
///
/// Returns `409 round_not_ready` if the ticket is not waiting to be carried
/// out, `404` if there is no such ticket, `401` if nobody is signed in, and
/// `403` if the caller is not a waiter.
#[utoipa::path(
    post,
    path = "/api/rounds/{id}/served",
    tag = "orders",
    params(("id" = Uuid, Path, description = "The ticket that reached the table.")),
    responses(
        (status = 200, description = "The ticket after the write. Waiters only.", body = OrderRoundDto),
        (status = 401, description = "Nobody is signed in.", body = ErrorBody),
        (status = 403, description = "Not a waiter.", body = ErrorBody),
        (status = 404, description = "No such ticket.", body = ErrorBody),
        (status = 409, description = "That ticket is not waiting to be carried out.", body = ErrorBody),
    )
)]
pub async fn mark_round_served(
    State(state): State<AppState>,
    actor: Actor<Waiter>,
    Path(round_id): Path<Uuid>,
) -> Result<Json<OrderRoundDto>, ApiError> {
    let round_id = OrderRoundId::from_uuid(round_id);

    let mut tx = state.database.begin_scoped(actor.restaurant_id()).await?;

    let round = service::round(&mut tx, round_id).await?;

    if round.status != RoundStatus::Ready {
        return Err(DomainError::Conflict(ConflictKind::RoundNotReady).into());
    }

    serve_every_ready_line(&mut tx, round_id).await?;

    let served = service::round(&mut tx, round_id).await?;
    let lines = service::lines_for_round(&mut tx, round_id).await?;

    tx.commit().await?;

    Ok(Json(OrderRoundDto::new(&served, lines)))
}

/// Marks every dish on a ticket that is waiting to be carried out.
///
/// A line level conflict raised in here means somebody else served this ticket
/// between the precheck and now, so it is reported as `round_not_ready`: that
/// is what it means to the person reading it, and a message about one dish
/// would send them looking at the wrong thing.
async fn serve_every_ready_line(tx: &mut ScopedTx<'_>, round_id: OrderRoundId) -> DomainResult<()> {
    let lines = service::lines_for_round(tx, round_id).await?;

    for line in lines {
        if line.status != LineStatus::Ready {
            continue;
        }

        match service::mark_line_served(tx, line.id).await {
            Ok(_) => {}
            Err(DomainError::Conflict(_)) => {
                return Err(DomainError::Conflict(ConflictKind::RoundNotReady));
            }
            Err(other) => return Err(other),
        }
    }

    Ok(())
}
