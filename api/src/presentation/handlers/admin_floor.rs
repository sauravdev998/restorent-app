//! The admin's floor: reading it whole, and every change an admin makes to its
//! sections and tables.
//!
//! Spec 0010. Every endpoint here is admin only, and says so in its own
//! signature through `Actor<Admin>`, so a chef or a waiter is refused with
//! `403` before any body runs. The waiter reads the floor at `GET /api/floor`.
//!
//! **The API is the authority on every rule.** Names, labels, seats, and the
//! range are checked here as field errors the admin's form puts beside the box
//! they concern; the database holds each rule again underneath.
//!
//! **A clash on a live name or label is a field error, except on a restore.**
//! Creating, renaming, and editing catch the unique violation and say
//! `name: already_taken` or `label: already_taken`. A restore has no box to put
//! it beside, so it stays `409 name_taken`. A range and a section restore can
//! clash on many labels at once, and answer `409 labels_taken` with the list.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::domain::catalog::{DiningTable, TableSection};
use crate::domain::error::{ConflictKind, DomainError, FieldError, FieldErrors};
use crate::domain::floor;
use crate::domain::ids::{DiningTableId, RestaurantId, TableSectionId};
use crate::infrastructure::db::Database;
use crate::infrastructure::db::repository::floor as repository;
use crate::infrastructure::db::repository::floor::{NewTable, NewTableRange, TableEdit};
use crate::presentation::error::{ApiError, ErrorBody};
use crate::presentation::extract::{Actor, Admin, JsonBody};
use crate::presentation::state::AppState;

// ===========================================================================
// Shapes
// ===========================================================================

/// The whole floor as the admin works on it, live and archived.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AdminFloorResponse {
    /// The live groups in displayed order. The first is always the no section
    /// group (`id`, `name`, and `version` all `null`), even when it is empty;
    /// then every live section, empty or not.
    pub groups: Vec<AdminFloorGroupDto>,
    /// What has been taken off the floor and can be put back.
    pub archived: ArchivedFloorDto,
}

/// One group of live tables.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AdminFloorGroupDto {
    /// Which section this is, or `null` for the no section group.
    pub id: Option<Uuid>,
    /// What it is called, or `null` for the no section group.
    pub name: Option<String>,
    /// Which edit of the section this is, or `null` for the no section group.
    /// Send it back with a rename.
    pub version: Option<i32>,
    /// Its live tables, in displayed order.
    pub tables: Vec<AdminTableDto>,
}

/// One live table on the admin's floor.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AdminTableDto {
    /// Which table this is.
    pub id: Uuid,
    /// What the staff call it.
    pub label: String,
    /// How many it seats, when recorded.
    pub seats: Option<i16>,
    /// Which edit of the table this is. Send it back with an edit.
    pub version: i32,
    /// Whether a party is at it right now. A table with a party at it cannot
    /// be removed.
    pub occupied: bool,
}

/// Everything in the Archived section.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ArchivedFloorDto {
    /// Archived sections, most recently removed first.
    pub sections: Vec<ArchivedSectionDto>,
    /// Every archived table, in the order they stood before removal.
    pub tables: Vec<ArchivedTableDto>,
}

/// One archived section, with the archived tables a restore can bring back
/// with it.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ArchivedSectionDto {
    /// Which section this is.
    pub id: Uuid,
    /// What it was called.
    pub name: String,
    /// When it was taken off the floor.
    pub archived_at: DateTime<Utc>,
    /// The archived tables whose section is this one, in their old order.
    pub tables: Vec<ArchivedSectionTableDto>,
}

/// One table a section restore can bring back.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ArchivedSectionTableDto {
    /// Which table this is.
    pub id: Uuid,
    /// What the staff called it.
    pub label: String,
    /// How many it seated, when recorded.
    pub seats: Option<i16>,
}

/// One archived table, with what the restore dialog needs.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ArchivedTableDto {
    /// Which table this is.
    pub id: Uuid,
    /// What the staff called it.
    pub label: String,
    /// How many it seated, when recorded.
    pub seats: Option<i16>,
    /// The section it was in when it was removed, if any.
    pub section_id: Option<Uuid>,
    /// What that section is called, carried here because it may be archived
    /// too and so appear nowhere else on the screen.
    pub section_name: Option<String>,
    /// Whether that section is still live. The restore dialog offers it as the
    /// default only when it is.
    pub section_live: bool,
    /// When it was taken off the floor.
    pub archived_at: DateTime<Utc>,
}

/// One section, as a write on it answers.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SectionDto {
    /// Which section this is.
    pub id: Uuid,
    /// What it is called.
    pub name: String,
    /// Which edit of the section this is.
    pub version: i32,
    /// When it was taken off the floor, or `null` while it is on it.
    pub archived_at: Option<DateTime<Utc>>,
}

impl From<TableSection> for SectionDto {
    fn from(section: TableSection) -> Self {
        Self {
            id: section.id.as_uuid(),
            name: section.name,
            version: section.version,
            archived_at: section.archived_at,
        }
    }
}

/// One table, as a write on it answers.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TableDto {
    /// Which table this is.
    pub id: Uuid,
    /// Which section it is in, or `null`.
    pub section_id: Option<Uuid>,
    /// What the staff call it.
    pub label: String,
    /// How many it seats, when recorded.
    pub seats: Option<i16>,
    /// Which edit of the table this is.
    pub version: i32,
    /// When it was taken off the floor, or `null` while it is on it.
    pub archived_at: Option<DateTime<Utc>>,
}

impl From<DiningTable> for TableDto {
    fn from(table: DiningTable) -> Self {
        Self {
            id: table.id.as_uuid(),
            section_id: table.section_id.map(TableSectionId::as_uuid),
            label: table.label,
            seats: table.seats,
            version: table.version,
            archived_at: table.archived_at,
        }
    }
}

// ===========================================================================
// The whole floor
// ===========================================================================

/// The admin's whole floor, live and archived, with each table's occupancy.
///
/// # Errors
///
/// Returns `401` if nobody is signed in, `403` for a waiter or a chef, and
/// `503` if the database is unavailable.
#[utoipa::path(
    get,
    path = "/api/admin/floor",
    tag = "floor",
    responses(
        (status = 200, description = "The whole floor, live and archived. Admins only.", body = AdminFloorResponse),
        (status = 401, description = "Nobody is signed in.", body = ErrorBody),
        (status = 403, description = "Not an admin.", body = ErrorBody),
    )
)]
pub async fn admin_floor(
    State(state): State<AppState>,
    actor: Actor<Admin>,
) -> Result<Json<AdminFloorResponse>, ApiError> {
    // A snapshot: four statements, and a table opened or moved between two of
    // them would otherwise read two different ways on one screen.
    let mut tx = state
        .database
        .begin_scoped_snapshot(actor.restaurant_id())
        .await?;

    let sections = repository::live_table_sections(&mut tx).await?;
    let tables = repository::live_tables_with_occupancy(&mut tx).await?;
    let archived_sections = repository::archived_table_sections(&mut tx).await?;
    let archived_tables = repository::archived_dining_tables(&mut tx).await?;

    tx.commit().await?;

    let live_ids: Vec<TableSectionId> = sections.iter().map(|section| section.id).collect();

    let table_dto = |entry: &crate::domain::catalog::AdminFloorTable| AdminTableDto {
        id: entry.table.id.as_uuid(),
        label: entry.table.label.clone(),
        seats: entry.table.seats,
        version: entry.table.version,
        occupied: entry.occupied,
    };

    // A table with no section, or one whose section was archived before this
    // feature, sits in the no section group, the same as on the waiter's floor.
    let loose = AdminFloorGroupDto {
        id: None,
        name: None,
        version: None,
        tables: tables
            .iter()
            .filter(|entry| {
                entry
                    .table
                    .section_id
                    .is_none_or(|id| !live_ids.contains(&id))
            })
            .map(table_dto)
            .collect(),
    };

    let groups = std::iter::once(loose)
        .chain(sections.into_iter().map(|section| {
            AdminFloorGroupDto {
                id: Some(section.id.as_uuid()),
                tables: tables
                    .iter()
                    .filter(|entry| entry.table.section_id == Some(section.id))
                    .map(table_dto)
                    .collect(),
                name: Some(section.name),
                version: Some(section.version),
            }
        }))
        .collect();

    let archived = ArchivedFloorDto {
        sections: archived_sections
            .into_iter()
            .filter_map(|section| {
                Some(ArchivedSectionDto {
                    id: section.id.as_uuid(),
                    tables: archived_tables
                        .iter()
                        .filter(|archived| archived.table.section_id == Some(section.id))
                        .map(|archived| ArchivedSectionTableDto {
                            id: archived.table.id.as_uuid(),
                            label: archived.table.label.clone(),
                            seats: archived.table.seats,
                        })
                        .collect(),
                    name: section.name,
                    archived_at: section.archived_at?,
                })
            })
            .collect(),
        tables: archived_tables
            .into_iter()
            .filter_map(|archived| {
                Some(ArchivedTableDto {
                    id: archived.table.id.as_uuid(),
                    label: archived.table.label,
                    seats: archived.table.seats,
                    section_id: archived.table.section_id.map(TableSectionId::as_uuid),
                    section_name: archived.section_name,
                    section_live: archived.section_live,
                    archived_at: archived.table.archived_at?,
                })
            })
            .collect(),
    };

    Ok(Json(AdminFloorResponse { groups, archived }))
}

// ===========================================================================
// Sections
// ===========================================================================

/// What adding a section asks for.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateSectionRequest {
    /// What it is called. At most 40 characters, unique among live sections
    /// ignoring letter case.
    pub name: String,
}

/// What renaming a section asks for.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RenameSectionRequest {
    /// What it should be called.
    pub name: String,
    /// The version the rename form loaded. An older one is refused as stale.
    pub version: i32,
}

/// What reordering the sections asks for: the complete list, in its new order.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SectionOrderRequest {
    /// Every live section id, each exactly once, in the new order.
    pub ids: Vec<Uuid>,
}

/// What restoring a section asks for.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RestoreSectionRequest {
    /// The archived tables of this section to bring back with it. May be
    /// empty. Each must be an archived table whose section is this one.
    pub table_ids: Vec<Uuid>,
}

/// A restored section and the tables that came back with it.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RestoredSectionDto {
    /// The section, live again at the end of the list.
    pub section: SectionDto,
    /// The tables restored into it, in their order.
    pub tables: Vec<TableDto>,
}

/// Adds a section to the end of the section list.
///
/// It appears on the admin's screen at once, and on waiters' floors once it
/// holds a live table.
///
/// # Errors
///
/// Returns `400` naming the field that was not accepted, including
/// `fields.name=already_taken`, `401` if nobody is signed in, and `403` for a
/// waiter or a chef.
#[utoipa::path(
    post,
    path = "/api/admin/floor/sections",
    tag = "floor",
    request_body = CreateSectionRequest,
    responses(
        (status = 201, description = "The section, at the end of the list. Admins only.", body = SectionDto),
        (status = 400, description = "A field was not accepted.", body = ErrorBody),
        (status = 401, description = "Nobody is signed in.", body = ErrorBody),
        (status = 403, description = "Not an admin.", body = ErrorBody),
    )
)]
pub async fn create_section(
    State(state): State<AppState>,
    actor: Actor<Admin>,
    JsonBody(request): JsonBody<CreateSectionRequest>,
) -> Result<(StatusCode, Json<SectionDto>), ApiError> {
    let name = section_name(&request.name)?;

    let mut tx = state.database.begin_scoped(actor.restaurant_id()).await?;
    let section = repository::create_table_section(&mut tx, &name, actor.staff_id())
        .await
        .map_err(|error| taken_on_the_box(error, "name"))?;
    tx.commit().await?;

    Ok((StatusCode::CREATED, Json(section.into())))
}

/// Renames a section, provided nobody changed it since the form loaded.
///
/// # Errors
///
/// Returns `409 section_changed` if the stored version is newer, `404` if
/// there is no such live section, `400` naming the field that was not
/// accepted, `401` if nobody is signed in, and `403` for a waiter or a chef.
#[utoipa::path(
    put,
    path = "/api/admin/floor/sections/{id}",
    tag = "floor",
    params(("id" = Uuid, Path, description = "The section to rename.")),
    request_body = RenameSectionRequest,
    responses(
        (status = 200, description = "The renamed section. Admins only.", body = SectionDto),
        (status = 400, description = "A field was not accepted.", body = ErrorBody),
        (status = 401, description = "Nobody is signed in.", body = ErrorBody),
        (status = 403, description = "Not an admin.", body = ErrorBody),
        (status = 404, description = "No such live section.", body = ErrorBody),
        (status = 409, description = "`section_changed`: somebody changed it after the form loaded.", body = ErrorBody),
    )
)]
pub async fn rename_section(
    State(state): State<AppState>,
    actor: Actor<Admin>,
    Path(section_id): Path<Uuid>,
    JsonBody(request): JsonBody<RenameSectionRequest>,
) -> Result<Json<SectionDto>, ApiError> {
    let name = section_name(&request.name)?;

    let mut tx = state.database.begin_scoped(actor.restaurant_id()).await?;
    let section = repository::rename_table_section(
        &mut tx,
        TableSectionId::from_uuid(section_id),
        &name,
        request.version,
        actor.staff_id(),
    )
    .await
    .map_err(|error| taken_on_the_box(error, "name"))?;
    tx.commit().await?;

    Ok(Json(section.into()))
}

/// Puts the sections in a new order, the order waiters then see.
///
/// No audit row: a reorder changes no version, so it makes no open form stale.
///
/// # Errors
///
/// Returns `409 floor_changed` if the list is not exactly the live sections,
/// `401` if nobody is signed in, and `403` for a waiter or a chef.
#[utoipa::path(
    put,
    path = "/api/admin/floor/sections/order",
    tag = "floor",
    request_body = SectionOrderRequest,
    responses(
        (status = 200, description = "The live sections in their new order. Admins only.", body = [SectionDto]),
        (status = 401, description = "Nobody is signed in.", body = ErrorBody),
        (status = 403, description = "Not an admin.", body = ErrorBody),
        (status = 409, description = "`floor_changed`: the list is not the live set any more.", body = ErrorBody),
    )
)]
pub async fn reorder_sections(
    State(state): State<AppState>,
    actor: Actor<Admin>,
    JsonBody(request): JsonBody<SectionOrderRequest>,
) -> Result<Json<Vec<SectionDto>>, ApiError> {
    let mut tx = state.database.begin_scoped(actor.restaurant_id()).await?;
    let sections = repository::reorder_table_sections(&mut tx, &request.ids).await?;
    tx.commit().await?;

    Ok(Json(sections.into_iter().map(SectionDto::from).collect()))
}

/// Takes a section off the floor. It moves to the Archived section.
///
/// Refused while it still holds a live table, including one being created in,
/// moved into, or restored into it at the same instant.
///
/// # Errors
///
/// Returns `409 section_not_empty` if it holds a live table, `404` if there is
/// no such live section, `401` if nobody is signed in, and `403` for a waiter
/// or a chef.
#[utoipa::path(
    post,
    path = "/api/admin/floor/sections/{id}/archive",
    tag = "floor",
    params(("id" = Uuid, Path, description = "The section to remove.")),
    responses(
        (status = 200, description = "The archived section. Admins only.", body = SectionDto),
        (status = 401, description = "Nobody is signed in.", body = ErrorBody),
        (status = 403, description = "Not an admin.", body = ErrorBody),
        (status = 404, description = "No such live section.", body = ErrorBody),
        (status = 409, description = "`section_not_empty`: it still holds a live table.", body = ErrorBody),
    )
)]
pub async fn archive_section(
    State(state): State<AppState>,
    actor: Actor<Admin>,
    Path(section_id): Path<Uuid>,
) -> Result<Json<SectionDto>, ApiError> {
    let mut tx = state.database.begin_scoped(actor.restaurant_id()).await?;
    let section = repository::archive_table_section(
        &mut tx,
        TableSectionId::from_uuid(section_id),
        actor.staff_id(),
    )
    .await?;
    tx.commit().await?;

    Ok(Json(section.into()))
}

/// Puts an archived section back at the end of the list, with the chosen
/// archived tables that were in it, in one transaction.
///
/// # Errors
///
/// Returns `409 name_taken` if a live section now has its name, `409
/// labels_taken` with `labels` if a chosen table's label is taken (either
/// refusal restores nothing), `400` if an id is not an archived table of this
/// section, `404` if there is no such archived section, `401` if nobody is
/// signed in, and `403` for a waiter or a chef.
#[utoipa::path(
    post,
    path = "/api/admin/floor/sections/{id}/restore",
    tag = "floor",
    params(("id" = Uuid, Path, description = "The archived section to put back.")),
    request_body = RestoreSectionRequest,
    responses(
        (status = 200, description = "The section, live again, and its restored tables. Admins only.", body = RestoredSectionDto),
        (status = 400, description = "An id is not an archived table of this section.", body = ErrorBody),
        (status = 401, description = "Nobody is signed in.", body = ErrorBody),
        (status = 403, description = "Not an admin.", body = ErrorBody),
        (status = 404, description = "No such archived section.", body = ErrorBody),
        (status = 409, description = "`name_taken`, or `labels_taken` with the clashing `labels`.", body = ErrorBody),
    )
)]
pub async fn restore_section(
    State(state): State<AppState>,
    actor: Actor<Admin>,
    Path(section_id): Path<Uuid>,
    JsonBody(request): JsonBody<RestoreSectionRequest>,
) -> Result<Json<RestoredSectionDto>, ApiError> {
    let section_id = TableSectionId::from_uuid(section_id);
    let mut tx = state.database.begin_scoped(actor.restaurant_id()).await?;

    let outcome = repository::restore_table_section(
        &mut tx,
        section_id,
        &request.table_ids,
        actor.staff_id(),
    )
    .await;

    let (section, tables) = match outcome {
        Ok(restored) => restored,
        Err(DomainError::Conflict(ConflictKind::LabelsTaken(labels))) if labels.is_empty() => {
            tx.rollback().await?;
            let clashes = late_clashes(
                &state.database,
                actor.restaurant_id(),
                Wanted::ArchivedInSection(section_id, &request.table_ids),
            )
            .await?;
            return Err(DomainError::Conflict(ConflictKind::LabelsTaken(clashes)).into());
        }
        Err(error) => return Err(error.into()),
    };

    tx.commit().await?;

    Ok(Json(RestoredSectionDto {
        section: section.into(),
        tables: tables.into_iter().map(TableDto::from).collect(),
    }))
}

/// A section name, trimmed, or the field error saying why not.
fn section_name(text: &str) -> Result<String, ApiError> {
    floor::section_name(text)
        .map_err(|error| DomainError::InvalidFields(FieldErrors::one("name", error)).into())
}

// ===========================================================================
// Tables
// ===========================================================================

/// What adding one table asks for.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateTableRequest {
    /// Which live section it goes in, or `null` for none.
    #[serde(default)]
    pub section_id: Option<Uuid>,
    /// What the staff call it. At most 12 characters, unique among the
    /// restaurant's live tables ignoring letter case.
    pub label: String,
    /// How many it seats, a whole number from 1 to 50, or `null`.
    #[serde(default)]
    pub seats: Option<i64>,
}

/// What adding a numbered range of tables asks for.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateTableRangeRequest {
    /// Which live section every table goes in, or `null` for none.
    #[serde(default)]
    pub section_id: Option<Uuid>,
    /// Written before each number. Leading spaces are removed and trailing
    /// ones kept, so `"Bar "` gives `Bar 1`. Absent or blank means bare
    /// numbers.
    #[serde(default)]
    pub prefix: Option<String>,
    /// The first number, 1 to 999.
    pub from: i64,
    /// The last number, 1 to 999, not below `from`, and at most 50 tables in
    /// all.
    pub to: i64,
    /// How many each table seats, a whole number from 1 to 50, or `null`.
    #[serde(default)]
    pub seats: Option<i64>,
}

/// What editing a table asks for: everything the table should be, and which
/// version of it the form loaded.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct EditTableRequest {
    /// Which live section it should be in, or `null` for none. A different
    /// group moves it to the end of that group.
    pub section_id: Option<Uuid>,
    /// What the staff should call it.
    pub label: String,
    /// How many it seats, or `null`.
    pub seats: Option<i64>,
    /// The version the edit form loaded.
    pub version: i32,
}

/// What reordering one group of tables asks for.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TableOrderRequest {
    /// Which group: a live section, or `null` for the no section group.
    pub section_id: Option<Uuid>,
    /// Every live table of that group, each exactly once, in the new order.
    pub ids: Vec<Uuid>,
}

/// What restoring a table asks for.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RestoreTableRequest {
    /// Which live section to put it in, or `null` for none. The dialog offers
    /// the table's old section when that is still live.
    pub section_id: Option<Uuid>,
}

/// Adds one table to the end of its group.
///
/// It appears on the admin's screen at once, and on every waiter's floor within
/// about two seconds.
///
/// # Errors
///
/// Returns `400` naming each field that was not accepted, including
/// `fields.label=already_taken`, `409 section_archived` if the section has been
/// archived, `404` for a section this restaurant does not have, `401` if nobody
/// is signed in, and `403` for a waiter or a chef.
#[utoipa::path(
    post,
    path = "/api/admin/floor/tables",
    tag = "floor",
    request_body = CreateTableRequest,
    responses(
        (status = 201, description = "The table, at the end of its group. Admins only.", body = TableDto),
        (status = 400, description = "A field was not accepted.", body = ErrorBody),
        (status = 401, description = "Nobody is signed in.", body = ErrorBody),
        (status = 403, description = "Not an admin.", body = ErrorBody),
        (status = 404, description = "No such section.", body = ErrorBody),
        (status = 409, description = "`section_archived`: that section is no longer on the floor.", body = ErrorBody),
    )
)]
pub async fn create_table(
    State(state): State<AppState>,
    actor: Actor<Admin>,
    JsonBody(request): JsonBody<CreateTableRequest>,
) -> Result<(StatusCode, Json<TableDto>), ApiError> {
    let (label, seats) =
        floor::table_fields(&request.label, request.seats).map_err(DomainError::InvalidFields)?;

    let mut tx = state.database.begin_scoped(actor.restaurant_id()).await?;
    let table = repository::create_dining_table(
        &mut tx,
        &NewTable {
            section_id: request.section_id.map(TableSectionId::from_uuid),
            label: &label,
            seats,
        },
        actor.staff_id(),
    )
    .await
    .map_err(|error| taken_on_the_box(error, "label"))?;
    tx.commit().await?;

    Ok((StatusCode::CREATED, Json(table.into())))
}

/// Adds a numbered range of tables to the end of their group, every one or
/// none.
///
/// # Errors
///
/// Returns `400` naming each field that was not accepted, `409 labels_taken`
/// with `labels` listing every generated label a live table already has (and
/// nothing created), `409 section_archived` if the section has been archived,
/// `404` for a section this restaurant does not have, `401` if nobody is
/// signed in, and `403` for a waiter or a chef.
#[utoipa::path(
    post,
    path = "/api/admin/floor/tables/range",
    tag = "floor",
    request_body = CreateTableRangeRequest,
    responses(
        (status = 201, description = "The created tables, in number order. Admins only.", body = [TableDto]),
        (status = 400, description = "A field was not accepted.", body = ErrorBody),
        (status = 401, description = "Nobody is signed in.", body = ErrorBody),
        (status = 403, description = "Not an admin.", body = ErrorBody),
        (status = 404, description = "No such section.", body = ErrorBody),
        (status = 409, description = "`labels_taken` with the clashing `labels`, or `section_archived`.", body = ErrorBody),
    )
)]
pub async fn create_table_range(
    State(state): State<AppState>,
    actor: Actor<Admin>,
    JsonBody(request): JsonBody<CreateTableRangeRequest>,
) -> Result<(StatusCode, Json<Vec<TableDto>>), ApiError> {
    let range = floor::table_range(
        request.prefix.as_deref(),
        request.from,
        request.to,
        request.seats,
    )
    .map_err(DomainError::InvalidFields)?;

    let mut tx = state.database.begin_scoped(actor.restaurant_id()).await?;
    let outcome = repository::create_table_range(
        &mut tx,
        &NewTableRange {
            section_id: request.section_id.map(TableSectionId::from_uuid),
            labels: &range.labels,
            seats: range.seats,
        },
        actor.staff_id(),
    )
    .await;

    let tables = match outcome {
        Ok(tables) => tables,
        Err(DomainError::Conflict(ConflictKind::LabelsTaken(labels))) if labels.is_empty() => {
            tx.rollback().await?;
            let clashes = late_clashes(
                &state.database,
                actor.restaurant_id(),
                Wanted::Labels(&range.labels),
            )
            .await?;
            return Err(DomainError::Conflict(ConflictKind::LabelsTaken(clashes)).into());
        }
        Err(error) => return Err(error.into()),
    };

    tx.commit().await?;

    Ok((
        StatusCode::CREATED,
        Json(tables.into_iter().map(TableDto::from).collect()),
    ))
}

/// Edits a table's label, seats, or section, provided nobody changed it since
/// the form loaded.
///
/// Allowed while a party sits at it. A changed label reaches every waiter's
/// floor, the waiter's table screen, and the kitchen's tickets within about two
/// seconds.
///
/// # Errors
///
/// Returns `400` naming each field that was not accepted, `409
/// section_archived` if the target section has been archived, `409
/// table_changed` if the stored version is newer, `404` if there is no such
/// live table or section, `401` if nobody is signed in, and `403` for a waiter
/// or a chef.
#[utoipa::path(
    put,
    path = "/api/admin/floor/tables/{id}",
    tag = "floor",
    params(("id" = Uuid, Path, description = "The table to edit.")),
    request_body = EditTableRequest,
    responses(
        (status = 200, description = "The table after the edit. Admins only.", body = TableDto),
        (status = 400, description = "A field was not accepted.", body = ErrorBody),
        (status = 401, description = "Nobody is signed in.", body = ErrorBody),
        (status = 403, description = "Not an admin.", body = ErrorBody),
        (status = 404, description = "No such live table or section.", body = ErrorBody),
        (status = 409, description = "`section_archived` or `table_changed`.", body = ErrorBody),
    )
)]
pub async fn edit_table(
    State(state): State<AppState>,
    actor: Actor<Admin>,
    Path(table_id): Path<Uuid>,
    JsonBody(request): JsonBody<EditTableRequest>,
) -> Result<Json<TableDto>, ApiError> {
    let (label, seats) =
        floor::table_fields(&request.label, request.seats).map_err(DomainError::InvalidFields)?;

    let mut tx = state.database.begin_scoped(actor.restaurant_id()).await?;
    let table = repository::edit_dining_table(
        &mut tx,
        DiningTableId::from_uuid(table_id),
        &TableEdit {
            section_id: request.section_id.map(TableSectionId::from_uuid),
            label,
            seats,
            version: request.version,
        },
        actor.staff_id(),
    )
    .await
    .map_err(|error| taken_on_the_box(error, "label"))?;
    tx.commit().await?;

    Ok(Json(table.into()))
}

/// Puts one group's tables in a new order, the order waiters then see.
///
/// A table cannot be dragged into another group here: moving it is an edit.
///
/// # Errors
///
/// Returns `409 floor_changed` if the list is not exactly that group's live
/// tables, `404` if the section is not a live one, `401` if nobody is signed
/// in, and `403` for a waiter or a chef.
#[utoipa::path(
    put,
    path = "/api/admin/floor/table-order",
    tag = "floor",
    request_body = TableOrderRequest,
    responses(
        (status = 200, description = "The group's live tables in their new order. Admins only.", body = [TableDto]),
        (status = 401, description = "Nobody is signed in.", body = ErrorBody),
        (status = 403, description = "Not an admin.", body = ErrorBody),
        (status = 404, description = "No such live section.", body = ErrorBody),
        (status = 409, description = "`floor_changed`: the list is not the live set any more.", body = ErrorBody),
    )
)]
pub async fn reorder_tables(
    State(state): State<AppState>,
    actor: Actor<Admin>,
    JsonBody(request): JsonBody<TableOrderRequest>,
) -> Result<Json<Vec<TableDto>>, ApiError> {
    let mut tx = state.database.begin_scoped(actor.restaurant_id()).await?;
    let tables = repository::reorder_table_group(
        &mut tx,
        request.section_id.map(TableSectionId::from_uuid),
        &request.ids,
    )
    .await?;
    tx.commit().await?;

    Ok(Json(tables.into_iter().map(TableDto::from).collect()))
}

/// Takes a table off the floor. It moves to the Archived section.
///
/// Refused while a party sits at it, including one being seated at the same
/// instant. Every visit, round, ticket, and bill that named it still resolves.
///
/// # Errors
///
/// Returns `409 table_in_use` if it has an open visit, `404` if there is no
/// such live table, `401` if nobody is signed in, and `403` for a waiter or a
/// chef.
#[utoipa::path(
    post,
    path = "/api/admin/floor/tables/{id}/archive",
    tag = "floor",
    params(("id" = Uuid, Path, description = "The table to remove.")),
    responses(
        (status = 200, description = "The archived table. Admins only.", body = TableDto),
        (status = 401, description = "Nobody is signed in.", body = ErrorBody),
        (status = 403, description = "Not an admin.", body = ErrorBody),
        (status = 404, description = "No such live table.", body = ErrorBody),
        (status = 409, description = "`table_in_use`: a party is at it. Close the table first.", body = ErrorBody),
    )
)]
pub async fn archive_table(
    State(state): State<AppState>,
    actor: Actor<Admin>,
    Path(table_id): Path<Uuid>,
) -> Result<Json<TableDto>, ApiError> {
    let mut tx = state.database.begin_scoped(actor.restaurant_id()).await?;
    let table = repository::archive_dining_table(
        &mut tx,
        DiningTableId::from_uuid(table_id),
        actor.staff_id(),
    )
    .await?;
    tx.commit().await?;

    Ok(Json(table.into()))
}

/// Puts an archived table back, at the end of a live section or of the no
/// section group. It keeps its label and seats.
///
/// # Errors
///
/// Returns `409 name_taken` if a live table now has its label, `409
/// section_archived` if the chosen section has been archived, `404` if there
/// is no such archived table or section, `401` if nobody is signed in, and
/// `403` for a waiter or a chef.
#[utoipa::path(
    post,
    path = "/api/admin/floor/tables/{id}/restore",
    tag = "floor",
    params(("id" = Uuid, Path, description = "The archived table to put back.")),
    request_body = RestoreTableRequest,
    responses(
        (status = 200, description = "The table, live again. Admins only.", body = TableDto),
        (status = 401, description = "Nobody is signed in.", body = ErrorBody),
        (status = 403, description = "Not an admin.", body = ErrorBody),
        (status = 404, description = "No such archived table or section.", body = ErrorBody),
        (status = 409, description = "`name_taken` or `section_archived`.", body = ErrorBody),
    )
)]
pub async fn restore_table(
    State(state): State<AppState>,
    actor: Actor<Admin>,
    Path(table_id): Path<Uuid>,
    JsonBody(request): JsonBody<RestoreTableRequest>,
) -> Result<Json<TableDto>, ApiError> {
    let mut tx = state.database.begin_scoped(actor.restaurant_id()).await?;
    let table = repository::restore_dining_table(
        &mut tx,
        DiningTableId::from_uuid(table_id),
        request.section_id.map(TableSectionId::from_uuid),
        actor.staff_id(),
    )
    .await?;
    tx.commit().await?;

    Ok(Json(table.into()))
}

// ===========================================================================
// Helpers
// ===========================================================================

/// Puts a live name or label clash beside its box rather than at the top of
/// the form.
fn taken_on_the_box(error: DomainError, field: &str) -> ApiError {
    match error {
        DomainError::Conflict(ConflictKind::NameTaken) => {
            DomainError::InvalidFields(FieldErrors::one(field, FieldError::AlreadyTaken)).into()
        }
        other => other.into(),
    }
}

/// Which labels a request meant, for looking a late clash up again.
enum Wanted<'a> {
    /// A range, which built its labels before touching the database.
    Labels(&'a [String]),
    /// A section restore, whose labels are those of the chosen archived
    /// tables, read again in the fresh transaction.
    ArchivedInSection(TableSectionId, &'a [Uuid]),
}

/// Looks a late label clash up again, in a fresh scoped transaction.
///
/// The write that found the clash failed on the unique index, and Postgres
/// refuses every further statement in that transaction, so the caller has
/// already rolled it back. The answer may be empty, when the clashing table was
/// removed again meanwhile.
async fn late_clashes(
    database: &Database,
    restaurant_id: RestaurantId,
    wanted: Wanted<'_>,
) -> Result<Vec<String>, ApiError> {
    let mut tx = database.begin_scoped(restaurant_id).await?;

    let labels = match wanted {
        Wanted::Labels(labels) => labels.to_vec(),
        Wanted::ArchivedInSection(section_id, table_ids) => {
            repository::archived_labels_in_section(&mut tx, section_id, table_ids).await?
        }
    };

    let clashes = repository::clashing_labels(&mut tx, &labels).await?;
    tx.commit().await?;

    Ok(clashes)
}
