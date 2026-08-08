//! Development only routes. Never mounted outside development.
//!
//! This exists so the scaffold can prove the whole live path for real: an HTTP
//! request opens a scoped transaction, the transaction calls
//! `notify_entity_change`, Postgres notifies every instance, this instance's
//! listener picks it up, the registry routes it by restaurant, and it arrives
//! on an open browser stream. Feature 8 replaces it with a waiter sending a
//! real dish.

use axum::Json;
use axum::extract::State;
use serde::Serialize;
use uuid::Uuid;

use crate::infrastructure::db::Database;
use crate::presentation::error::ApiError;
use crate::presentation::extract::RestaurantScope;
use crate::presentation::state::AppState;

/// What the probe returns, so a caller can match it against what arrived on the
/// stream.
#[derive(Debug, Serialize)]
pub struct ProbeSent {
    /// The id published on the stream.
    pub entity_id: Uuid,
}

/// Publishes one probe event to the caller's restaurant.
///
/// # Errors
///
/// Returns [`ApiError`] if the request carries no restaurant scope, or if the
/// database refuses the transaction, the notify, or the commit.
pub async fn notify(
    State(state): State<AppState>,
    scope: RestaurantScope,
) -> Result<Json<ProbeSent>, ApiError> {
    let entity_id = Uuid::new_v4();

    // Exactly the shape every real write will take: open a scoped transaction,
    // do the work, notify inside it, commit.
    let mut tx = state.database.begin_scoped(scope.restaurant_id()).await?;
    Database::notify_entity_change(&mut tx, "probe", entity_id).await?;
    tx.commit().await?;

    Ok(Json(ProbeSent { entity_id }))
}
