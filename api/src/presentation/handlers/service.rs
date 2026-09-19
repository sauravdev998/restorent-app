//! The service loop over HTTP: the floor, the waiter's Orders list, seating a
//! party, sending a ticket, the kitchen queue, and moving one dish along.
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
//! **One lock order.** Every path that takes more than one lock takes them as
//! visit, then line or ticket, then bill, then the bill number counter. Send
//! takes visit then bill; void and serve take line and ticket then bill; close
//! takes the visit first (`service::lock_visit`), then the bill, then the
//! counter. No path takes a visit lock after a bill lock, which is what lets a
//! send and a close on one table race without deadlocking. Feature 23 will
//! lock two bills at once and must fit in here: lower bill id first.
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

use crate::domain::catalog::TableSection;
use crate::domain::error::{DomainError, FieldErrors};
use crate::domain::ids::{
    DiningTableId, DishId, OrderLineId, OrderRoundId, StaffId, TableSectionId, VisitId,
};
use crate::domain::service::{FloorTable, NewOrderLine, normalize_note, void_reason_text};
use crate::infrastructure::db::repository::{billing, floor, service};
use crate::presentation::dto::{
    LineStatusDto, OrderLineDto, OrderRoundDto, RoundStatusDto, VoidReasonDto,
};
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
    /// The waiter who hears the ready chime for this table.
    pub responsible_staff_id: Uuid,
    /// What to call that waiter.
    pub responsible_name: String,
    /// When they sat down.
    pub opened_at: DateTime<Utc>,
    /// How many of them, when the waiter recorded it.
    pub guest_count: Option<i16>,
    /// How many dishes on this visit are waiting to be carried out.
    pub ready_dish_count: i64,
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

    let sections = floor::live_table_sections(&mut tx).await?;
    let tables = service::floor(&mut tx).await?;

    tx.commit().await?;

    Ok(Json(FloorResponse {
        sections: group_floor(sections, &tables),
    }))
}

/// The floor in walking order: each live section that holds a live table, then
/// the tables no live section holds.
///
/// A section with no live table is left out, so a waiter never reads a heading
/// with nothing under it. A table can belong to no section at all, or to one
/// archived before spec 0010 made that impossible. It is still a table somebody
/// sits at, so it goes into a group of its own at the end rather than
/// disappearing from the floor.
fn group_floor(sections: Vec<TableSection>, tables: &[FloorTable]) -> Vec<FloorSectionDto> {
    let mut grouped: Vec<FloorSectionDto> = sections
        .into_iter()
        .map(|section| FloorSectionDto {
            id: Some(section.id.as_uuid()),
            name: Some(section.name),
            tables: tables_in(tables, Some(section.id.as_uuid())),
        })
        .filter(|section| !section.tables.is_empty())
        .collect();

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

    grouped
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
                responsible_staff_id: occupancy.responsible_staff_id.as_uuid(),
                responsible_name: occupancy.responsible_name.clone(),
                opened_at: occupancy.opened_at,
                guest_count: occupancy.guest_count,
                ready_dish_count: occupancy.ready_dish_count,
            }),
    }
}

// ===========================================================================
// The Orders list
// ===========================================================================

/// Every open table in the restaurant, with every ticket on it.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct OpenOrdersResponse {
    /// The open visits, in the order they were opened. The screen sorts them
    /// by what is ready.
    pub visits: Vec<OpenOrderDto>,
    /// What the server's clock reads, at the moment this answer was built. The
    /// screen corrects every age it draws by the difference from its own clock.
    pub server_time: DateTime<Utc>,
}

/// One open table on the Orders list.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct OpenOrderDto {
    /// The visit.
    pub id: Uuid,
    /// Which table they are at.
    pub table_id: Uuid,
    /// What the staff call that table.
    pub table_label: String,
    /// When they sat down.
    pub opened_at: DateTime<Utc>,
    /// The waiter who hears the ready chime for this table.
    pub responsible_staff_id: Uuid,
    /// What to call that waiter.
    pub responsible_name: String,
    /// Every ticket on the visit, oldest first, with its dishes.
    pub rounds: Vec<OrderRoundDto>,
}

/// Every open table and every ticket on it.
///
/// # Errors
///
/// Returns `401` if nobody is signed in, `403` if the caller is not a waiter,
/// and `503` if the database is unavailable.
#[utoipa::path(
    get,
    path = "/api/orders/open",
    tag = "orders",
    responses(
        (status = 200, description = "Every open visit with its tickets and dishes. Waiters only.", body = OpenOrdersResponse),
        (status = 401, description = "Nobody is signed in.", body = ErrorBody),
        (status = 403, description = "Not a waiter.", body = ErrorBody),
    )
)]
pub async fn open_orders(
    State(state): State<AppState>,
    actor: Actor<Waiter>,
) -> Result<Json<OpenOrdersResponse>, ApiError> {
    // A snapshot: the visits and their dishes are two statements, and a dish
    // marked ready between them must not appear on a ticket read as cooking.
    let mut tx = state
        .database
        .begin_scoped_snapshot(actor.restaurant_id())
        .await?;
    let orders = service::open_orders(&mut tx).await?;
    tx.commit().await?;

    Ok(Json(OpenOrdersResponse {
        visits: orders
            .into_iter()
            .map(|order| OpenOrderDto {
                id: order.visit_id.as_uuid(),
                table_id: order.table_id.as_uuid(),
                table_label: order.table_label,
                opened_at: order.opened_at,
                responsible_staff_id: order.responsible_staff_id.as_uuid(),
                responsible_name: order.responsible_name,
                rounds: order
                    .rounds
                    .into_iter()
                    .map(|(round, lines)| OrderRoundDto::new(&round, lines))
                    .collect(),
            })
            .collect(),
        server_time: Utc::now(),
    }))
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
    /// A key the phone made for this basket, the same on every retry until one
    /// send succeeds. A key the server already holds for this visit sends
    /// nothing new and answers with the first ticket.
    pub client_key: Uuid,
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
    /// What the guest asked for, such as no onions. At most 140 characters
    /// after trimming; blank is the same as none.
    #[serde(default)]
    pub note: Option<String>,
}

/// Sends one ticket to the kitchen and puts its dishes on the bill.
///
/// One transaction, so a ticket whose lines are on no bill cannot exist. That
/// is what keeps the bill's running subtotal true throughout the meal and
/// leaves the close with no assignment left to do.
///
/// Safe to repeat: the same `clientKey` again answers `200` with the ticket the
/// first send made, and writes nothing.
///
/// # Errors
///
/// Returns `400` if the basket is empty, a quantity is not positive, or a note
/// is longer than 140 characters (as `fields."lines.N.note" = too_long`), `409
/// dish_not_orderable` if a dish was switched off or taken off the menu before
/// the ticket went, which refuses the whole ticket, `409 visit_not_open` if the
/// party has left, `409 client_key_reused` if the key already sent a ticket for
/// another visit, `404` if there is no such visit, `401` if nobody is signed
/// in, and `403` if the caller is not a waiter.
#[utoipa::path(
    post,
    path = "/api/visits/{id}/rounds",
    tag = "orders",
    params(("id" = Uuid, Path, description = "The visit to send the ticket for.")),
    request_body = SendRoundRequest,
    responses(
        (status = 200, description = "An earlier send with the same key already made this ticket; nothing new was sent. Waiters only.", body = OrderRoundDto),
        (status = 201, description = "The ticket that was sent. Waiters only.", body = OrderRoundDto),
        (status = 400, description = "An empty basket, a quantity below one, or a note over 140 characters.", body = ErrorBody),
        (status = 401, description = "Nobody is signed in.", body = ErrorBody),
        (status = 403, description = "Not a waiter.", body = ErrorBody),
        (status = 404, description = "No such visit.", body = ErrorBody),
        (status = 409, description = "`visit_not_open`: that party has already left. `dish_not_orderable`: a dish went off before the ticket went, and nothing was sent. `client_key_reused`: that key already sent a ticket for another table.", body = ErrorBody),
    )
)]
pub async fn send_round(
    State(state): State<AppState>,
    actor: Actor<Waiter>,
    Path(visit_id): Path<Uuid>,
    JsonBody(request): JsonBody<SendRoundRequest>,
) -> Result<(StatusCode, Json<OrderRoundDto>), ApiError> {
    let visit_id = VisitId::from_uuid(visit_id);

    let mut new_lines = Vec::with_capacity(request.lines.len());
    let mut problems = FieldErrors::default();

    for (index, line) in request.lines.into_iter().enumerate() {
        match normalize_note(line.note.as_deref()) {
            Ok(note) => new_lines.push(NewOrderLine {
                dish_id: DishId::from_uuid(line.dish_id),
                quantity: line.quantity,
                note,
            }),
            Err(problem) => problems.add(&format!("lines.{index}.note"), problem),
        }
    }

    if !problems.is_empty() {
        return Err(DomainError::InvalidFields(problems).into());
    }

    let mut tx = state.database.begin_scoped(actor.restaurant_id()).await?;

    let sent = service::send_round(
        &mut tx,
        visit_id,
        actor.staff_id(),
        request.client_key,
        &new_lines,
    )
    .await?;

    if sent.replayed {
        // Nothing was written, so there is nothing to put on the bill and no
        // subtotal to recompute or announce.
        tx.commit().await?;
        return Ok((
            StatusCode::OK,
            Json(OrderRoundDto::new(&sent.round, sent.lines)),
        ));
    }

    // Straight onto the visit's open bill, in this same transaction. A visit
    // opened by this API always has one; a visit that somehow has none is a
    // refusal rather than a ticket nobody can be charged for.
    let bill_id = service::open_bill_of(&mut tx, visit_id)
        .await?
        .ok_or(DomainError::NotFound)?;

    let line_ids: Vec<OrderLineId> = sent.lines.iter().map(|line| line.id).collect();
    billing::assign_lines_to_bill(&mut tx, bill_id, &line_ids).await?;

    tx.commit().await?;

    Ok((
        StatusCode::CREATED,
        Json(OrderRoundDto::new(&sent.round, sent.lines)),
    ))
}

// ===========================================================================
// Taking over and moving a table
// ===========================================================================

/// What taking over a table asks for.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TakeOverRequest {
    /// The waiter the screen showed as responsible. If somebody else has taken
    /// the table since, the request is refused rather than taking it from them.
    pub expected_staff_id: Uuid,
}

/// Who is responsible for a table now.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TakeOverResponse {
    /// The visit.
    pub visit_id: Uuid,
    /// The waiter now responsible.
    pub responsible_staff_id: Uuid,
    /// What to call them.
    pub responsible_name: String,
}

/// Makes the caller the responsible waiter for a table, so its ready chime
/// comes to them.
///
/// # Errors
///
/// Returns `409 table_taken_over` if somebody else became responsible after
/// the screen was drawn, `409 visit_not_open` if the party has left, `404` if
/// there is no such visit, `401` if nobody is signed in, and `403` if the
/// caller is not a waiter.
#[utoipa::path(
    post,
    path = "/api/visits/{id}/take-over",
    tag = "orders",
    params(("id" = Uuid, Path, description = "The visit to take over.")),
    request_body = TakeOverRequest,
    responses(
        (status = 200, description = "The caller is now responsible. Waiters only.", body = TakeOverResponse),
        (status = 401, description = "Nobody is signed in.", body = ErrorBody),
        (status = 403, description = "Not a waiter.", body = ErrorBody),
        (status = 404, description = "No such visit.", body = ErrorBody),
        (status = 409, description = "`table_taken_over`: somebody else took it first. `visit_not_open`: the party has left.", body = ErrorBody),
    )
)]
pub async fn take_over(
    State(state): State<AppState>,
    actor: Actor<Waiter>,
    Path(visit_id): Path<Uuid>,
    JsonBody(request): JsonBody<TakeOverRequest>,
) -> Result<Json<TakeOverResponse>, ApiError> {
    let mut tx = state.database.begin_scoped(actor.restaurant_id()).await?;

    let visit = service::take_over_visit(
        &mut tx,
        VisitId::from_uuid(visit_id),
        StaffId::from_uuid(request.expected_staff_id),
        actor.staff_id(),
    )
    .await?;

    let responsible = crate::infrastructure::db::repository::accounts::staff_by_id(
        &mut tx,
        visit.responsible_staff_id,
    )
    .await?;

    tx.commit().await?;

    Ok(Json(TakeOverResponse {
        visit_id: visit.id.as_uuid(),
        responsible_staff_id: visit.responsible_staff_id.as_uuid(),
        responsible_name: responsible.display_name,
    }))
}

/// What moving a party asks for.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MoveVisitRequest {
    /// The free, live table they are moving to.
    pub table_id: Uuid,
}

/// Where a party sits now.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MoveVisitResponse {
    /// The visit.
    pub visit_id: Uuid,
    /// The table they are at now.
    pub table_id: Uuid,
    /// What the staff call it.
    pub table_label: String,
}

/// Moves a party, with their tickets, bill, and responsible waiter, to a free
/// table.
///
/// # Errors
///
/// Returns `409 table_occupied` if the destination has a party at it, `409
/// visit_not_open` if the party has left, `404` if there is no such visit or
/// the destination is archived or unknown, `401` if nobody is signed in, and
/// `403` if the caller is not a waiter.
#[utoipa::path(
    post,
    path = "/api/visits/{id}/move",
    tag = "orders",
    params(("id" = Uuid, Path, description = "The visit to move.")),
    request_body = MoveVisitRequest,
    responses(
        (status = 200, description = "The party's new table. Waiters only.", body = MoveVisitResponse),
        (status = 401, description = "Nobody is signed in.", body = ErrorBody),
        (status = 403, description = "Not a waiter.", body = ErrorBody),
        (status = 404, description = "No such visit, or no such live table.", body = ErrorBody),
        (status = 409, description = "`table_occupied`: that table has a party at it. `visit_not_open`: the party has left.", body = ErrorBody),
    )
)]
pub async fn move_visit(
    State(state): State<AppState>,
    actor: Actor<Waiter>,
    Path(visit_id): Path<Uuid>,
    JsonBody(request): JsonBody<MoveVisitRequest>,
) -> Result<Json<MoveVisitResponse>, ApiError> {
    let mut tx = state.database.begin_scoped(actor.restaurant_id()).await?;

    let visit = service::move_visit(
        &mut tx,
        VisitId::from_uuid(visit_id),
        DiningTableId::from_uuid(request.table_id),
        actor.staff_id(),
    )
    .await?;
    let label = service::table_label(&mut tx, visit.table_id).await?;

    tx.commit().await?;

    Ok(Json(MoveVisitResponse {
        visit_id: visit.id.as_uuid(),
        table_id: visit.table_id.as_uuid(),
        table_label: label,
    }))
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

/// Marks one dish as having reached the table.
///
/// Touches that dish only. A starter goes out the moment it is ready while the
/// main course on the same ticket is still cooking.
///
/// # Errors
///
/// Returns `409 line_not_ready` if the dish is not waiting to be carried out,
/// `404` if there is no such dish, `401` if nobody is signed in, and `403` if
/// the caller is not a waiter.
#[utoipa::path(
    post,
    path = "/api/order-lines/{id}/served",
    tag = "orders",
    params(("id" = Uuid, Path, description = "The dish that reached the table.")),
    responses(
        (status = 200, description = "The dish and its ticket's new status. Waiters only.", body = MarkedLineResponse),
        (status = 401, description = "Nobody is signed in.", body = ErrorBody),
        (status = 403, description = "Not a waiter.", body = ErrorBody),
        (status = 404, description = "No such dish.", body = ErrorBody),
        (status = 409, description = "`line_not_ready`: that dish is not waiting to be carried out.", body = ErrorBody),
    )
)]
pub async fn mark_line_served(
    State(state): State<AppState>,
    actor: Actor<Waiter>,
    Path(line_id): Path<Uuid>,
) -> Result<Json<MarkedLineResponse>, ApiError> {
    let mut tx = state.database.begin_scoped(actor.restaurant_id()).await?;

    let (line, round_status) =
        service::mark_line_served(&mut tx, OrderLineId::from_uuid(line_id)).await?;

    tx.commit().await?;

    Ok(Json(MarkedLineResponse {
        line: line.into(),
        round_status: round_status.into(),
    }))
}

/// What cancelling a dish asks for.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct VoidLineRequest {
    /// Why, from the closed list.
    pub reason_code: VoidReasonDto,
    /// The waiter's own words. Required for `other`, optional otherwise, at
    /// most 200 characters.
    #[serde(default)]
    pub reason: Option<String>,
}

/// What cancelling a dish changed.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct VoidLineResponse {
    /// The dish, after the write.
    pub line: OrderLineDto,
    /// What the whole ticket became, recomputed from all of its dishes.
    pub round_status: RoundStatusDto,
    /// The bill's running subtotal after the dish came off it, as an exact
    /// decimal string. `null` for a dish on no bill.
    pub bill_subtotal: Option<String>,
}

/// Cancels one dish that is still cooking or waiting on the pass, with a
/// reason, and takes it off the bill.
///
/// # Errors
///
/// Returns `400` if the reason is `other` with no words
/// (`fields.reason = required`) or the words are over 200 characters
/// (`fields.reason = too_long`), `409 line_not_voidable` if the dish has been
/// served or already cancelled, `404` if there is no such dish, `401` if nobody
/// is signed in, and `403` if the caller is not a waiter.
#[utoipa::path(
    post,
    path = "/api/order-lines/{id}/void",
    tag = "orders",
    params(("id" = Uuid, Path, description = "The dish to cancel.")),
    request_body = VoidLineRequest,
    responses(
        (status = 200, description = "The cancelled dish, its ticket, and the bill's subtotal after. Waiters only.", body = VoidLineResponse),
        (status = 400, description = "A missing or over long reason.", body = ErrorBody),
        (status = 401, description = "Nobody is signed in.", body = ErrorBody),
        (status = 403, description = "Not a waiter.", body = ErrorBody),
        (status = 404, description = "No such dish.", body = ErrorBody),
        (status = 409, description = "`line_not_voidable`: that dish has been served or already cancelled.", body = ErrorBody),
    )
)]
pub async fn void_line(
    State(state): State<AppState>,
    actor: Actor<Waiter>,
    Path(line_id): Path<Uuid>,
    JsonBody(request): JsonBody<VoidLineRequest>,
) -> Result<Json<VoidLineResponse>, ApiError> {
    let code = request.reason_code.into();
    let reason = void_reason_text(code, request.reason.as_deref())
        .map_err(|problem| DomainError::InvalidFields(FieldErrors::one("reason", problem)))?;

    let mut tx = state.database.begin_scoped(actor.restaurant_id()).await?;

    let voided = service::void_line(
        &mut tx,
        OrderLineId::from_uuid(line_id),
        actor.staff_id(),
        code,
        reason.as_deref(),
    )
    .await?;

    tx.commit().await?;

    Ok(Json(VoidLineResponse {
        line: voided.line.into(),
        round_status: voided.round_status.into(),
        bill_subtotal: voided.bill_subtotal.map(|subtotal| subtotal.to_string()),
    }))
}

/// Marks every dish on a ticket that is waiting to be carried out.
///
/// Serves what is ready and leaves what is still cooking, so a waiter carrying
/// out the starters on a ticket whose main is still on the stove taps once.
///
/// # Errors
///
/// Returns `409 nothing_ready` if no dish on the ticket is waiting to be
/// carried out, `404` if there is no such ticket, `401` if nobody is signed in,
/// and `403` if the caller is not a waiter.
#[utoipa::path(
    post,
    path = "/api/rounds/{id}/served",
    tag = "orders",
    params(("id" = Uuid, Path, description = "The ticket whose ready dishes reached the table.")),
    responses(
        (status = 200, description = "The ticket after the write. Waiters only.", body = OrderRoundDto),
        (status = 401, description = "Nobody is signed in.", body = ErrorBody),
        (status = 403, description = "Not a waiter.", body = ErrorBody),
        (status = 404, description = "No such ticket.", body = ErrorBody),
        (status = 409, description = "`nothing_ready`: no dish on that ticket is waiting to be carried out.", body = ErrorBody),
    )
)]
pub async fn mark_round_served(
    State(state): State<AppState>,
    actor: Actor<Waiter>,
    Path(round_id): Path<Uuid>,
) -> Result<Json<OrderRoundDto>, ApiError> {
    let round_id = OrderRoundId::from_uuid(round_id);

    let mut tx = state.database.begin_scoped(actor.restaurant_id()).await?;

    service::serve_ready_lines(&mut tx, round_id).await?;

    let served = service::round(&mut tx, round_id).await?;
    let lines = service::lines_for_round(&mut tx, round_id).await?;

    tx.commit().await?;

    Ok(Json(OrderRoundDto::new(&served, lines)))
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::domain::catalog::DiningTable;

    fn section(name: &str, position: i32) -> TableSection {
        TableSection {
            id: TableSectionId::new(),
            name: name.to_owned(),
            position,
            version: 1,
            archived_at: None,
        }
    }

    fn table(label: &str, section_id: Option<TableSectionId>) -> FloorTable {
        FloorTable {
            table: DiningTable {
                id: DiningTableId::new(),
                section_id,
                label: label.to_owned(),
                seats: Some(4),
                position: 1,
                version: 1,
                archived_at: None,
            },
            occupancy: None,
        }
    }

    fn shape(groups: &[FloorSectionDto]) -> Vec<(Option<String>, Vec<String>)> {
        groups
            .iter()
            .map(|group| {
                (
                    group.name.clone(),
                    group.tables.iter().map(|t| t.label.clone()).collect(),
                )
            })
            .collect()
    }

    /// covers: AC-2, AC-15 (spec 0010)
    ///
    /// A section whose tables are all archived, or that has none yet, is not a
    /// heading on the waiter's floor. The read only returns live tables, so an
    /// archived one is simply absent from `tables` here.
    #[test]
    fn a_section_with_no_live_table_is_left_off_the_waiter_floor() {
        let terrace = section("Terrace", 1);
        let garden = section("Garden", 2);
        let tables = [table("T1", Some(terrace.id)), table("T2", Some(terrace.id))];

        let groups = group_floor(vec![terrace, garden], &tables);

        assert_eq!(
            shape(&groups),
            [(
                Some("Terrace".to_owned()),
                vec!["T1".to_owned(), "T2".to_owned()]
            )]
        );
    }

    /// covers: AC-15 (spec 0010)
    ///
    /// A table with no section, or with a section that is no longer live, goes
    /// in its own group at the end, with no name for the screen to translate.
    #[test]
    fn a_table_no_live_section_holds_goes_in_a_group_of_its_own_at_the_end() {
        let terrace = section("Terrace", 1);
        let gone = TableSectionId::new();
        let tables = [
            table("Counter", None),
            table("T1", Some(terrace.id)),
            table("Old", Some(gone)),
        ];

        let groups = group_floor(vec![terrace], &tables);

        assert_eq!(
            shape(&groups),
            [
                (Some("Terrace".to_owned()), vec!["T1".to_owned()]),
                (None, vec!["Counter".to_owned(), "Old".to_owned()]),
            ]
        );
        assert!(groups.last().is_some_and(|group| group.id.is_none()));
    }

    /// covers: AC-15 (spec 0010)
    #[test]
    fn a_restaurant_with_no_live_table_has_no_groups_at_all() {
        assert!(group_floor(vec![section("Terrace", 1)], &[]).is_empty());
    }
}
