//! The menu, as a waiter taking an order and a chef on the Menu tab read it,
//! and the one switch the kitchen may throw on it.
//!
//! The read's only interesting property is what it does not leave out. An unavailable dish comes back, marked, because a waiter who can
//! see that the kitchen has run out of the fish can tell the customer so; a
//! menu that silently dropped it would leave the waiter walking back to the
//! pass to find out. Ordering one is refused when the round is sent, by
//! `send_round`, so the marking on this screen is a courtesy and not the
//! control.
//!
//! An archived dish, by contrast, never appears at all. It is not off tonight,
//! it is off the menu, and `live_dishes` filters it for every caller.
//!
//! The switch sits here rather than with the admin's menu endpoints because it
//! is not an admin's tool first. It is the chef's, the moment the kitchen runs
//! out, and it changes exactly the read above.

use axum::Json;
use axum::extract::{Path, State};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::domain::catalog::Dish;
use crate::domain::ids::DishId;
use crate::infrastructure::db::repository::catalog;
use crate::presentation::dto::{DietDto, DishDto};
use crate::presentation::error::{ApiError, ErrorBody};
use crate::presentation::extract::{Actor, AdminOrChef, JsonBody, WaiterOrChef};
use crate::presentation::state::AppState;

/// The whole menu, grouped the way it is printed.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MenuResponse {
    /// The live categories, in printed order. A category with no live dish is
    /// left out, because an empty heading on a phone is a row of wasted space.
    pub categories: Vec<MenuCategoryDto>,
    /// What this restaurant charges in. Carried here rather than left to the
    /// screen to look up, so a price and the currency it is in arrive together.
    pub currency_code: String,
    /// How many decimal places that currency uses.
    pub currency_decimals: u32,
}

/// One group of dishes.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MenuCategoryDto {
    /// Which category this is.
    pub id: Uuid,
    /// What it is called.
    pub name: String,
    /// Its dishes, in printed order.
    pub dishes: Vec<MenuDishDto>,
}

/// One item on the menu.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MenuDishDto {
    /// Which dish this is.
    pub id: Uuid,
    /// What it is called today.
    pub name: String,
    /// What it is, for the waiter to read out.
    pub description: Option<String>,
    /// What it costs, as an exact decimal string. Never a JSON number.
    pub price: String,
    /// Whether it is veg, non veg, or egg, drawn as the square mark.
    pub diet: DietDto,
    /// Whether the kitchen can make it right now. `false` greys it on the
    /// ordering screen and stops it being added to a basket.
    pub available: bool,
}

impl From<Dish> for MenuDishDto {
    fn from(dish: Dish) -> Self {
        Self {
            id: dish.id.as_uuid(),
            name: dish.name,
            description: dish.description,
            price: dish.price.to_string(),
            diet: dish.diet.into(),
            available: dish.is_available,
        }
    }
}

/// The restaurant's live menu, in printed order.
///
/// # Errors
///
/// Returns `401` if nobody is signed in, `403` if the caller is neither a
/// waiter nor a chef, and `503` if the database is unavailable.
#[utoipa::path(
    get,
    path = "/api/menu",
    tag = "orders",
    responses(
        (status = 200, description = "The live menu. Waiters and chefs.", body = MenuResponse),
        (status = 401, description = "Nobody is signed in.", body = ErrorBody),
        (status = 403, description = "Neither a waiter nor a chef.", body = ErrorBody),
    )
)]
pub async fn menu(
    State(state): State<AppState>,
    actor: Actor<WaiterOrChef>,
) -> Result<Json<MenuResponse>, ApiError> {
    // A snapshot: three statements, and a dish switched off between two of them
    // would arrive under a category read before the change.
    let mut tx = state
        .database
        .begin_scoped_snapshot(actor.restaurant_id())
        .await?;

    let restaurant = catalog::restaurant(&mut tx).await?;
    let categories = catalog::live_menu_categories(&mut tx).await?;
    let dishes = catalog::live_dishes(&mut tx).await?;

    tx.commit().await?;

    // `live_dishes` comes back flat and already in position order, so grouping
    // is a filter per category rather than a sort. The category order is the
    // category list's own, which is why the outer loop drives it.
    let grouped = categories
        .into_iter()
        .map(|category| {
            let dishes: Vec<MenuDishDto> = dishes
                .iter()
                .filter(|dish| dish.category_id == category.id)
                .cloned()
                .map(MenuDishDto::from)
                .collect();

            MenuCategoryDto {
                id: category.id.as_uuid(),
                name: category.name,
                dishes,
            }
        })
        .filter(|category| !category.dishes.is_empty())
        .collect();

    Ok(Json(MenuResponse {
        categories: grouped,
        currency_code: restaurant.currency.code().to_owned(),
        currency_decimals: restaurant.currency.decimals(),
    }))
}

/// What the availability switch asks for.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SetAvailabilityRequest {
    /// What the dish should be: `true` for orderable, `false` for off. An
    /// absolute value, never a toggle, so two quick taps cannot cancel out.
    pub available: bool,
}

/// Switches a dish on or off.
///
/// Admins and chefs, and it is the only menu change a chef can make. Every
/// waiter's ordering screen greys the dish within a second or two, through the
/// `dish` event this write sends. Never refused as stale; setting the value it
/// already has succeeds and changes nothing.
///
/// # Errors
///
/// Returns `404` if there is no such live dish, including one archived a moment
/// earlier, `401` if nobody is signed in, and `403` for a waiter.
#[utoipa::path(
    put,
    path = "/api/dishes/{id}/availability",
    tag = "menu",
    params(("id" = Uuid, Path, description = "The dish to switch.")),
    request_body = SetAvailabilityRequest,
    responses(
        (status = 200, description = "The dish after the switch. Admins and chefs.", body = DishDto),
        (status = 401, description = "Nobody is signed in.", body = ErrorBody),
        (status = 403, description = "A waiter asked.", body = ErrorBody),
        (status = 404, description = "No such live dish.", body = ErrorBody),
    )
)]
pub async fn set_availability(
    State(state): State<AppState>,
    actor: Actor<AdminOrChef>,
    Path(dish_id): Path<Uuid>,
    JsonBody(request): JsonBody<SetAvailabilityRequest>,
) -> Result<Json<DishDto>, ApiError> {
    let mut tx = state.database.begin_scoped(actor.restaurant_id()).await?;

    let dish = catalog::set_dish_availability(
        &mut tx,
        DishId::from_uuid(dish_id),
        request.available,
        actor.staff_id(),
    )
    .await?;

    tx.commit().await?;

    Ok(Json(dish.into()))
}
