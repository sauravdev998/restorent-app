//! The menu, as a waiter taking an order reads it.
//!
//! One endpoint, and the only interesting thing about it is what it does not
//! leave out. An unavailable dish comes back, marked, because a waiter who can
//! see that the kitchen has run out of the fish can tell the customer so; a
//! menu that silently dropped it would leave the waiter walking back to the
//! pass to find out. Ordering one is refused when the round is sent, by
//! `send_round`, so the marking on this screen is a courtesy and not the
//! control.
//!
//! An archived dish, by contrast, never appears at all. It is not off tonight,
//! it is off the menu, and `live_dishes` filters it for every caller.

use axum::Json;
use axum::extract::State;
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::domain::catalog::Dish;
use crate::infrastructure::db::repository::catalog;
use crate::presentation::error::{ApiError, ErrorBody};
use crate::presentation::extract::{Actor, Waiter};
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
            available: dish.is_available,
        }
    }
}

/// The restaurant's live menu, in printed order.
///
/// # Errors
///
/// Returns `401` if nobody is signed in, `403` if the caller is not a waiter,
/// and `503` if the database is unavailable.
#[utoipa::path(
    get,
    path = "/api/menu",
    tag = "orders",
    responses(
        (status = 200, description = "The live menu. Waiters only.", body = MenuResponse),
        (status = 401, description = "Nobody is signed in.", body = ErrorBody),
        (status = 403, description = "Not a waiter.", body = ErrorBody),
    )
)]
pub async fn menu(
    State(state): State<AppState>,
    actor: Actor<Waiter>,
) -> Result<Json<MenuResponse>, ApiError> {
    let mut tx = state.database.begin_scoped(actor.restaurant_id()).await?;

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
