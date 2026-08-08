//! Where a request's restaurant comes from.
//!
//! # This is a placeholder, and it fails closed
//!
//! Feature 7 (accounts, restaurants, and roles) owns the real answer: an opaque
//! session token in an httpOnly cookie, looked up against the `sessions` table,
//! which yields the signed in user and their restaurant.
//!
//! Until then this reads a header, and it does so **only in development**. In
//! any other environment it refuses every request. That direction matters: a
//! placeholder that accepted a client supplied restaurant id in production
//! would let anyone read any restaurant's data by editing one header, and it
//! would do it quietly. Refusing is loud and safe.

use axum::extract::FromRequestParts;
use axum::http::request::Parts;

use crate::domain::error::DomainError;
use crate::domain::ids::RestaurantId;
use crate::infrastructure::config::Environment;
use crate::presentation::error::ApiError;
use crate::presentation::state::AppState;

/// The header the development placeholder reads.
const DEV_RESTAURANT_HEADER: &str = "x-restaurant-id";

/// Which restaurant this request acts for.
///
/// A handler taking this argument is guaranteed to know its restaurant, so it
/// can go straight to `state.database.begin_scoped(scope.restaurant_id())`.
#[derive(Debug, Clone, Copy)]
pub struct RestaurantScope(RestaurantId);

impl RestaurantScope {
    /// The restaurant this request is acting for.
    #[must_use]
    pub const fn restaurant_id(&self) -> RestaurantId {
        self.0
    }
}

impl FromRequestParts<AppState> for RestaurantScope {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        if state.environment != Environment::Development {
            // Feature 7 replaces this branch with a real session lookup. Until
            // it does, no request outside development gets a restaurant.
            tracing::error!(
                "a request needed a restaurant scope but no session handling exists yet; refusing"
            );
            return Err(DomainError::Unauthenticated.into());
        }

        let raw = parts
            .headers
            .get(DEV_RESTAURANT_HEADER)
            .ok_or(DomainError::Unauthenticated)?
            .to_str()
            .map_err(|_| {
                DomainError::Invalid(format!("`{DEV_RESTAURANT_HEADER}` is not valid text"))
            })?;

        let restaurant_id = raw.trim().parse::<RestaurantId>().map_err(|_| {
            DomainError::Invalid(format!("`{DEV_RESTAURANT_HEADER}` is not a UUID"))
        })?;

        Ok(Self(restaurant_id))
    }
}
