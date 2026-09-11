//! The admin's menu: reading it whole, and every change an admin makes to it.
//!
//! Every endpoint here is admin only, and says so in its own signature through
//! `Actor<Admin>`, so a chef or a waiter is refused with `403` before any body
//! runs. The one menu change a chef may make, switching a dish on or off, lives
//! in `handlers/menu.rs` beside the read it changes.
//!
//! **The API is the authority on every rule.** Names, lengths, the price and
//! its decimals are checked here, as field errors the admin's form puts beside
//! the box they concern; the database holds each rule again underneath.
//!
//! **A clash on a live name is a field error, except on a restore.** Creating,
//! renaming, and editing all catch the unique violation and say
//! `name: already_taken`, so a race between two creates reads the same as a
//! plain clash. A restore has no name box to put it beside, so it stays a
//! `409 name_taken`.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::domain::error::{ConflictKind, DomainError, FieldError, FieldErrors};
use crate::domain::ids::{DishId, MenuCategoryId};
use crate::domain::menu;
use crate::infrastructure::db::repository::catalog::{self, DishEdit, NewDish};
use crate::presentation::dto::{DietDto, DishDto};
use crate::presentation::error::{ApiError, ErrorBody};
use crate::presentation::extract::{Actor, Admin, JsonBody};
use crate::presentation::state::AppState;

// ===========================================================================
// The whole menu
// ===========================================================================

/// The whole menu as the admin works on it, live and archived.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AdminMenuResponse {
    /// Every live category in printed order, each with every live dish in it,
    /// available or not. Unlike the ordering menu, an empty category is kept:
    /// it is exactly the one an admin is about to fill.
    pub categories: Vec<AdminCategoryDto>,
    /// What has been taken off the menu and can be put back.
    pub archived: ArchivedMenuDto,
    /// What this restaurant charges in.
    pub currency_code: String,
    /// How many decimal places that currency uses, which is also how many a
    /// price may carry.
    pub currency_decimals: u32,
}

/// One live category with its dishes.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AdminCategoryDto {
    /// Which category this is.
    pub id: Uuid,
    /// What it is called.
    pub name: String,
    /// Which edit of the category this is. Send it back with a rename.
    pub version: i32,
    /// Its live dishes, in printed order.
    pub dishes: Vec<DishDto>,
}

/// Everything in the Archived section.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ArchivedMenuDto {
    /// Archived categories, most recently removed first.
    pub categories: Vec<ArchivedCategoryDto>,
    /// Archived dishes, most recently removed first.
    pub dishes: Vec<ArchivedDishDto>,
}

/// One archived category.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ArchivedCategoryDto {
    /// Which category this is.
    pub id: Uuid,
    /// What it was called.
    pub name: String,
    /// When it was taken off the menu.
    pub archived_at: DateTime<Utc>,
}

/// One archived dish, with what the restore dialog needs.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ArchivedDishDto {
    /// Which dish this is.
    pub id: Uuid,
    /// What it is called.
    pub name: String,
    /// Whether it is veg, non veg, or egg.
    pub diet: DietDto,
    /// The category it was in when it was removed.
    pub category_id: Uuid,
    /// What that category is called, carried here because it may be archived
    /// too and so appear nowhere else on the screen.
    pub category_name: String,
    /// Whether that category is still live. The restore dialog offers it as
    /// the default only when it is.
    pub category_live: bool,
    /// When it was taken off the menu.
    pub archived_at: DateTime<Utc>,
}

/// One category, as a write on it answers.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CategoryDto {
    /// Which category this is.
    pub id: Uuid,
    /// What it is called.
    pub name: String,
    /// Which edit of the category this is.
    pub version: i32,
    /// When it was taken off the menu, or `null` while it is on it.
    pub archived_at: Option<DateTime<Utc>>,
}

impl From<crate::domain::catalog::MenuCategory> for CategoryDto {
    fn from(category: crate::domain::catalog::MenuCategory) -> Self {
        Self {
            id: category.id.as_uuid(),
            name: category.name,
            version: category.version,
            archived_at: category.archived_at,
        }
    }
}

/// The admin's whole menu, live and archived.
///
/// # Errors
///
/// Returns `401` if nobody is signed in, `403` for a waiter or a chef, and
/// `503` if the database is unavailable.
#[utoipa::path(
    get,
    path = "/api/admin/menu",
    tag = "menu",
    responses(
        (status = 200, description = "The whole menu, live and archived. Admins only.", body = AdminMenuResponse),
        (status = 401, description = "Nobody is signed in.", body = ErrorBody),
        (status = 403, description = "Not an admin.", body = ErrorBody),
    )
)]
pub async fn admin_menu(
    State(state): State<AppState>,
    actor: Actor<Admin>,
) -> Result<Json<AdminMenuResponse>, ApiError> {
    // A snapshot: five statements, and a dish moved between two of them would
    // otherwise appear in both categories or in neither.
    let mut tx = state
        .database
        .begin_scoped_snapshot(actor.restaurant_id())
        .await?;

    let restaurant = catalog::restaurant(&mut tx).await?;
    let categories = catalog::live_menu_categories(&mut tx).await?;
    let dishes = catalog::live_dishes(&mut tx).await?;
    let archived_categories = catalog::archived_menu_categories(&mut tx).await?;
    let archived_dishes = catalog::archived_dishes(&mut tx).await?;

    tx.commit().await?;

    let categories = categories
        .into_iter()
        .map(|category| AdminCategoryDto {
            id: category.id.as_uuid(),
            name: category.name,
            version: category.version,
            dishes: dishes
                .iter()
                .filter(|dish| dish.category_id == category.id)
                .cloned()
                .map(DishDto::from)
                .collect(),
        })
        .collect();

    let archived = ArchivedMenuDto {
        categories: archived_categories
            .into_iter()
            .filter_map(|category| {
                Some(ArchivedCategoryDto {
                    id: category.id.as_uuid(),
                    name: category.name,
                    archived_at: category.archived_at?,
                })
            })
            .collect(),
        dishes: archived_dishes
            .into_iter()
            .filter_map(|archived| {
                Some(ArchivedDishDto {
                    id: archived.dish.id.as_uuid(),
                    name: archived.dish.name,
                    diet: archived.dish.diet.into(),
                    category_id: archived.dish.category_id.as_uuid(),
                    category_name: archived.category_name,
                    category_live: archived.category_live,
                    archived_at: archived.dish.archived_at?,
                })
            })
            .collect(),
    };

    Ok(Json(AdminMenuResponse {
        categories,
        archived,
        currency_code: restaurant.currency.code().to_owned(),
        currency_decimals: restaurant.currency.decimals(),
    }))
}

// ===========================================================================
// Categories
// ===========================================================================

/// What adding a category asks for.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateCategoryRequest {
    /// What it is called. At most 60 characters, unique among live categories
    /// ignoring letter case.
    pub name: String,
}

/// What renaming a category asks for.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RenameCategoryRequest {
    /// What it should be called.
    pub name: String,
    /// The version the rename form loaded. An older one is refused as stale.
    pub version: i32,
}

/// Adds a category to the end of the category list.
///
/// It appears on the admin's screen at once, and on waiters' screens as soon as
/// it holds a live dish: the ordering menu leaves an empty heading out.
///
/// # Errors
///
/// Returns `400` naming the field that was not accepted, including
/// `fields.name=already_taken`, `401` if nobody is signed in, and `403` for a
/// waiter or a chef.
#[utoipa::path(
    post,
    path = "/api/admin/menu/categories",
    tag = "menu",
    request_body = CreateCategoryRequest,
    responses(
        (status = 201, description = "The category, at the end of the list. Admins only.", body = CategoryDto),
        (status = 400, description = "A field was not accepted.", body = ErrorBody),
        (status = 401, description = "Nobody is signed in.", body = ErrorBody),
        (status = 403, description = "Not an admin.", body = ErrorBody),
    )
)]
pub async fn create_category(
    State(state): State<AppState>,
    actor: Actor<Admin>,
    JsonBody(request): JsonBody<CreateCategoryRequest>,
) -> Result<(StatusCode, Json<CategoryDto>), ApiError> {
    let name = category_name(&request.name)?;

    let mut tx = state.database.begin_scoped(actor.restaurant_id()).await?;

    let category = catalog::create_menu_category(&mut tx, &name, actor.staff_id())
        .await
        .map_err(name_taken_on_the_name_box)?;

    tx.commit().await?;

    Ok((StatusCode::CREATED, Json(category.into())))
}

/// Renames a category, provided nobody changed it since the form loaded.
///
/// # Errors
///
/// Returns `409 category_changed` if the stored version is newer, `404` if
/// there is no such live category, `400` naming the field that was not
/// accepted, `401` if nobody is signed in, and `403` for a waiter or a chef.
#[utoipa::path(
    put,
    path = "/api/admin/menu/categories/{id}",
    tag = "menu",
    params(("id" = Uuid, Path, description = "The category to rename.")),
    request_body = RenameCategoryRequest,
    responses(
        (status = 200, description = "The renamed category. Admins only.", body = CategoryDto),
        (status = 400, description = "A field was not accepted.", body = ErrorBody),
        (status = 401, description = "Nobody is signed in.", body = ErrorBody),
        (status = 403, description = "Not an admin.", body = ErrorBody),
        (status = 404, description = "No such live category.", body = ErrorBody),
        (status = 409, description = "`category_changed`: somebody changed it after the form loaded.", body = ErrorBody),
    )
)]
pub async fn rename_category(
    State(state): State<AppState>,
    actor: Actor<Admin>,
    Path(category_id): Path<Uuid>,
    JsonBody(request): JsonBody<RenameCategoryRequest>,
) -> Result<Json<CategoryDto>, ApiError> {
    let name = category_name(&request.name)?;

    let mut tx = state.database.begin_scoped(actor.restaurant_id()).await?;

    let category = catalog::rename_menu_category(
        &mut tx,
        MenuCategoryId::from_uuid(category_id),
        &name,
        request.version,
        actor.staff_id(),
    )
    .await
    .map_err(name_taken_on_the_name_box)?;

    tx.commit().await?;

    Ok(Json(category.into()))
}

/// What a reorder asks for: the complete list, in its new order.
///
/// The whole list rather than a move, so the server never has to guess what
/// "after that one" meant when the list changed underneath the drag. A list
/// that is not exactly the live set is refused with `409 menu_changed`.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReorderRequest {
    /// Every live id in the list, each exactly once, in the new order.
    pub ids: Vec<Uuid>,
}

/// Puts the categories in a new order, the order waiters then see.
///
/// No audit row: a reorder moves nothing of consequence and changes no
/// version, so it makes no open form stale.
///
/// # Errors
///
/// Returns `409 menu_changed` if the list is not exactly the live categories,
/// `401` if nobody is signed in, and `403` for a waiter or a chef.
#[utoipa::path(
    put,
    path = "/api/admin/menu/categories/order",
    tag = "menu",
    request_body = ReorderRequest,
    responses(
        (status = 200, description = "The live categories in their new order. Admins only.", body = [CategoryDto]),
        (status = 401, description = "Nobody is signed in.", body = ErrorBody),
        (status = 403, description = "Not an admin.", body = ErrorBody),
        (status = 409, description = "`menu_changed`: the list is not the live set any more.", body = ErrorBody),
    )
)]
pub async fn reorder_categories(
    State(state): State<AppState>,
    actor: Actor<Admin>,
    JsonBody(request): JsonBody<ReorderRequest>,
) -> Result<Json<Vec<CategoryDto>>, ApiError> {
    let mut tx = state.database.begin_scoped(actor.restaurant_id()).await?;
    let categories = catalog::reorder_menu_categories(&mut tx, &request.ids).await?;
    tx.commit().await?;

    Ok(Json(
        categories.into_iter().map(CategoryDto::from).collect(),
    ))
}

/// Puts one category's dishes in a new order.
///
/// A dish cannot be dragged into another category here: moving it is an edit,
/// made in the edit form, where its version is checked.
///
/// # Errors
///
/// Returns `409 menu_changed` if the list is not exactly that category's live
/// dishes, `404` if there is no such live category, `401` if nobody is signed
/// in, and `403` for a waiter or a chef.
#[utoipa::path(
    put,
    path = "/api/admin/menu/categories/{id}/dish-order",
    tag = "menu",
    params(("id" = Uuid, Path, description = "The category whose dishes are being ordered.")),
    request_body = ReorderRequest,
    responses(
        (status = 200, description = "The category's live dishes in their new order. Admins only.", body = [DishDto]),
        (status = 401, description = "Nobody is signed in.", body = ErrorBody),
        (status = 403, description = "Not an admin.", body = ErrorBody),
        (status = 404, description = "No such live category.", body = ErrorBody),
        (status = 409, description = "`menu_changed`: the list is not the live set any more.", body = ErrorBody),
    )
)]
pub async fn reorder_dishes(
    State(state): State<AppState>,
    actor: Actor<Admin>,
    Path(category_id): Path<Uuid>,
    JsonBody(request): JsonBody<ReorderRequest>,
) -> Result<Json<Vec<DishDto>>, ApiError> {
    let mut tx = state.database.begin_scoped(actor.restaurant_id()).await?;
    let dishes = catalog::reorder_dishes(
        &mut tx,
        MenuCategoryId::from_uuid(category_id),
        &request.ids,
    )
    .await?;
    tx.commit().await?;

    Ok(Json(dishes.into_iter().map(DishDto::from).collect()))
}

/// Takes a category off the menu. It moves to the Archived section.
///
/// Refused while it still holds a live dish, including one being created in or
/// moved into it at the same instant: no live dish ever sits in an archived
/// category.
///
/// # Errors
///
/// Returns `409 category_not_empty` if it holds a live dish, `404` if there is
/// no such live category, `401` if nobody is signed in, and `403` for a waiter
/// or a chef.
#[utoipa::path(
    post,
    path = "/api/admin/menu/categories/{id}/archive",
    tag = "menu",
    params(("id" = Uuid, Path, description = "The category to remove.")),
    responses(
        (status = 200, description = "The archived category. Admins only.", body = CategoryDto),
        (status = 401, description = "Nobody is signed in.", body = ErrorBody),
        (status = 403, description = "Not an admin.", body = ErrorBody),
        (status = 404, description = "No such live category.", body = ErrorBody),
        (status = 409, description = "`category_not_empty`: it still holds a live dish.", body = ErrorBody),
    )
)]
pub async fn archive_category(
    State(state): State<AppState>,
    actor: Actor<Admin>,
    Path(category_id): Path<Uuid>,
) -> Result<Json<CategoryDto>, ApiError> {
    let mut tx = state.database.begin_scoped(actor.restaurant_id()).await?;
    let category = catalog::archive_menu_category(
        &mut tx,
        MenuCategoryId::from_uuid(category_id),
        actor.staff_id(),
    )
    .await?;
    tx.commit().await?;

    Ok(Json(category.into()))
}

/// Puts an archived category back, at the end of the category list.
///
/// # Errors
///
/// Returns `409 name_taken` if a live category now has its name, `404` if
/// there is no such archived category, `401` if nobody is signed in, and `403`
/// for a waiter or a chef.
#[utoipa::path(
    post,
    path = "/api/admin/menu/categories/{id}/restore",
    tag = "menu",
    params(("id" = Uuid, Path, description = "The archived category to put back.")),
    responses(
        (status = 200, description = "The category, live again. Admins only.", body = CategoryDto),
        (status = 401, description = "Nobody is signed in.", body = ErrorBody),
        (status = 403, description = "Not an admin.", body = ErrorBody),
        (status = 404, description = "No such archived category.", body = ErrorBody),
        (status = 409, description = "`name_taken`: a live category has that name now.", body = ErrorBody),
    )
)]
pub async fn restore_category(
    State(state): State<AppState>,
    actor: Actor<Admin>,
    Path(category_id): Path<Uuid>,
) -> Result<Json<CategoryDto>, ApiError> {
    let mut tx = state.database.begin_scoped(actor.restaurant_id()).await?;
    let category = catalog::restore_menu_category(
        &mut tx,
        MenuCategoryId::from_uuid(category_id),
        actor.staff_id(),
    )
    .await?;
    tx.commit().await?;

    Ok(Json(category.into()))
}

/// A category name, trimmed, or the field error saying why not.
fn category_name(text: &str) -> Result<String, ApiError> {
    menu::name(text, menu::CATEGORY_NAME_MAX)
        .map_err(|error| DomainError::InvalidFields(FieldErrors::one("name", error)).into())
}

// ===========================================================================
// Dishes
// ===========================================================================

/// What adding a dish asks for.
///
/// No availability: a new dish is available, and only the switch changes that.
/// No position: it goes to the end of its category.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateDishRequest {
    /// Which live category it goes in.
    pub category_id: Uuid,
    /// What it is called. At most 80 characters, unique among live dishes
    /// ignoring letter case.
    pub name: String,
    /// What it is, for the waiter to read out. At most 300 characters.
    #[serde(default)]
    pub description: Option<String>,
    /// What it costs, as a decimal string such as `"320.50"`. Never a JSON
    /// number. Zero or more, below ten billion, and no more decimal places than
    /// the restaurant's currency uses.
    pub price: String,
    /// Whether it is veg, non veg, or egg. Required.
    pub diet: DietDto,
}

/// Adds a dish to the end of a category.
///
/// # Errors
///
/// Returns `400` naming each field that was not accepted, `409
/// category_archived` if the category has been archived, `404` for a category
/// this restaurant does not have, `401` if nobody is signed in, and `403` for a
/// waiter or a chef.
#[utoipa::path(
    post,
    path = "/api/admin/menu/dishes",
    tag = "menu",
    request_body = CreateDishRequest,
    responses(
        (status = 201, description = "The dish, available, at the end of its category. Admins only.", body = DishDto),
        (status = 400, description = "A field was not accepted.", body = ErrorBody),
        (status = 401, description = "Nobody is signed in.", body = ErrorBody),
        (status = 403, description = "Not an admin.", body = ErrorBody),
        (status = 404, description = "No such category.", body = ErrorBody),
        (status = 409, description = "`category_archived`: that category is no longer on the menu.", body = ErrorBody),
    )
)]
pub async fn create_dish(
    State(state): State<AppState>,
    actor: Actor<Admin>,
    JsonBody(request): JsonBody<CreateDishRequest>,
) -> Result<(StatusCode, Json<DishDto>), ApiError> {
    let mut tx = state.database.begin_scoped(actor.restaurant_id()).await?;

    // The price rule depends on the currency, so the fields are checked inside
    // the transaction, against the same restaurant row the write will see.
    let restaurant = catalog::restaurant(&mut tx).await?;
    let fields = menu::dish_fields(
        &request.name,
        request.description.as_deref(),
        &request.price,
        restaurant.currency.decimals(),
    )
    .map_err(DomainError::InvalidFields)?;

    let dish = catalog::create_dish(
        &mut tx,
        &NewDish {
            category_id: MenuCategoryId::from_uuid(request.category_id),
            name: &fields.name,
            description: fields.description.as_deref(),
            price: fields.price,
            diet: request.diet.into(),
        },
        actor.staff_id(),
    )
    .await
    .map_err(name_taken_on_the_name_box)?;

    tx.commit().await?;

    Ok((StatusCode::CREATED, Json(dish.into())))
}

/// What editing a dish asks for: everything the dish should be, and which
/// version of it the form loaded.
///
/// No availability. Only the switch writes that, so an edit form opened before
/// the kitchen switched a dish off cannot switch it back on; the switch bumps
/// the version, and the form's save is refused as stale.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct EditDishRequest {
    /// Which live category it should sit under. A different one moves it to
    /// the end of that category.
    pub category_id: Uuid,
    /// What it should be called.
    pub name: String,
    /// What it is, or `null` for no description.
    #[serde(default)]
    pub description: Option<String>,
    /// What it should cost, as a decimal string. Lines already sent keep the
    /// price they copied.
    pub price: String,
    /// Whether it is veg, non veg, or egg.
    pub diet: DietDto,
    /// The version the edit form loaded.
    pub version: i32,
}

/// Edits a dish, moving it when its category changes.
///
/// The target category is checked before the version, so an edit that is both
/// stale and aimed at a removed category reports the category first: that is
/// what the admin has to change before anything else will save.
///
/// # Errors
///
/// Returns `400` naming each field that was not accepted, `409
/// category_archived` if the target category has been archived, `409
/// dish_changed` if the stored version is newer, `404` if there is no such live
/// dish or category, `401` if nobody is signed in, and `403` for a waiter or a
/// chef.
#[utoipa::path(
    put,
    path = "/api/admin/menu/dishes/{id}",
    tag = "menu",
    params(("id" = Uuid, Path, description = "The dish to edit.")),
    request_body = EditDishRequest,
    responses(
        (status = 200, description = "The dish after the edit. Admins only.", body = DishDto),
        (status = 400, description = "A field was not accepted.", body = ErrorBody),
        (status = 401, description = "Nobody is signed in.", body = ErrorBody),
        (status = 403, description = "Not an admin.", body = ErrorBody),
        (status = 404, description = "No such live dish or category.", body = ErrorBody),
        (status = 409, description = "`category_archived` or `dish_changed`.", body = ErrorBody),
    )
)]
pub async fn edit_dish(
    State(state): State<AppState>,
    actor: Actor<Admin>,
    Path(dish_id): Path<Uuid>,
    JsonBody(request): JsonBody<EditDishRequest>,
) -> Result<Json<DishDto>, ApiError> {
    let mut tx = state.database.begin_scoped(actor.restaurant_id()).await?;

    let restaurant = catalog::restaurant(&mut tx).await?;
    let fields = menu::dish_fields(
        &request.name,
        request.description.as_deref(),
        &request.price,
        restaurant.currency.decimals(),
    )
    .map_err(DomainError::InvalidFields)?;

    let dish = catalog::update_dish(
        &mut tx,
        DishId::from_uuid(dish_id),
        &DishEdit {
            category_id: MenuCategoryId::from_uuid(request.category_id),
            name: fields.name,
            description: fields.description,
            price: fields.price,
            diet: request.diet.into(),
            version: request.version,
        },
        actor.staff_id(),
    )
    .await
    .map_err(name_taken_on_the_name_box)?;

    tx.commit().await?;

    Ok(Json(dish.into()))
}

/// Takes a dish off the menu. It moves to the Archived section.
///
/// Every order line, round, and bill that referred to it still resolves and
/// reads exactly as before: a line copied the name and the price when it was
/// sent.
///
/// # Errors
///
/// Returns `404` if there is no such live dish, `401` if nobody is signed in,
/// and `403` for a waiter or a chef.
#[utoipa::path(
    post,
    path = "/api/admin/menu/dishes/{id}/archive",
    tag = "menu",
    params(("id" = Uuid, Path, description = "The dish to remove.")),
    responses(
        (status = 200, description = "The archived dish. Admins only.", body = DishDto),
        (status = 401, description = "Nobody is signed in.", body = ErrorBody),
        (status = 403, description = "Not an admin.", body = ErrorBody),
        (status = 404, description = "No such live dish.", body = ErrorBody),
    )
)]
pub async fn archive_dish(
    State(state): State<AppState>,
    actor: Actor<Admin>,
    Path(dish_id): Path<Uuid>,
) -> Result<Json<DishDto>, ApiError> {
    let mut tx = state.database.begin_scoped(actor.restaurant_id()).await?;
    let dish = catalog::archive_dish(&mut tx, DishId::from_uuid(dish_id), actor.staff_id()).await?;
    tx.commit().await?;

    Ok(Json(dish.into()))
}

/// What restoring a dish asks for.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RestoreDishRequest {
    /// Which live category to put it in. The dialog offers the dish's old one
    /// when that is still live.
    pub category_id: Uuid,
}

/// Puts an archived dish back, at the end of a live category.
///
/// It keeps its name, description, price, diet marker, and availability.
///
/// # Errors
///
/// Returns `409 name_taken` if a live dish now has its name, `409
/// category_archived` if the chosen category has been archived, `404` if there
/// is no such archived dish or category, `401` if nobody is signed in, and
/// `403` for a waiter or a chef.
#[utoipa::path(
    post,
    path = "/api/admin/menu/dishes/{id}/restore",
    tag = "menu",
    params(("id" = Uuid, Path, description = "The archived dish to put back.")),
    request_body = RestoreDishRequest,
    responses(
        (status = 200, description = "The dish, live again. Admins only.", body = DishDto),
        (status = 401, description = "Nobody is signed in.", body = ErrorBody),
        (status = 403, description = "Not an admin.", body = ErrorBody),
        (status = 404, description = "No such archived dish or category.", body = ErrorBody),
        (status = 409, description = "`name_taken` or `category_archived`.", body = ErrorBody),
    )
)]
pub async fn restore_dish(
    State(state): State<AppState>,
    actor: Actor<Admin>,
    Path(dish_id): Path<Uuid>,
    JsonBody(request): JsonBody<RestoreDishRequest>,
) -> Result<Json<DishDto>, ApiError> {
    let mut tx = state.database.begin_scoped(actor.restaurant_id()).await?;
    let dish = catalog::restore_dish(
        &mut tx,
        DishId::from_uuid(dish_id),
        MenuCategoryId::from_uuid(request.category_id),
        actor.staff_id(),
    )
    .await?;
    tx.commit().await?;

    Ok(Json(dish.into()))
}

/// Puts a live name clash beside the name box rather than at the top of the
/// form.
fn name_taken_on_the_name_box(error: DomainError) -> ApiError {
    match error {
        DomainError::Conflict(ConflictKind::NameTaken) => {
            DomainError::InvalidFields(FieldErrors::one("name", FieldError::AlreadyTaken)).into()
        }
        other => other.into(),
    }
}
