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

/// The query parameter the development placeholder also accepts.
///
/// The browser's `EventSource` cannot set a header, so the live stream has no
/// way to send one. In production this problem does not exist, because the
/// restaurant comes from the session cookie and `EventSource` sends cookies. So
/// this is only ever read in development, same as the header.
const DEV_RESTAURANT_QUERY: &str = "restaurant_id";

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

        let from_header = parts
            .headers
            .get(DEV_RESTAURANT_HEADER)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);

        let raw = match from_header {
            Some(value) => value,
            None => query_value(parts.uri.query(), DEV_RESTAURANT_QUERY)
                .ok_or(DomainError::Unauthenticated)?,
        };

        let restaurant_id = raw.trim().parse::<RestaurantId>().map_err(|_| {
            DomainError::Invalid(format!(
                "`{DEV_RESTAURANT_HEADER}` (or `?{DEV_RESTAURANT_QUERY}=`) is not a UUID"
            ))
        })?;

        Ok(Self(restaurant_id))
    }
}

/// Pulls one value out of a raw query string.
///
/// Hand rolled rather than pulling in a parser, because this is a development
/// only placeholder that feature 7 deletes.
fn query_value(query: Option<&str>, key: &str) -> Option<String> {
    query?.split('&').find_map(|pair| {
        let (name, value) = pair.split_once('=')?;
        (name == key).then(|| value.to_owned())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_named_value_from_a_query_string() {
        let query = Some("foo=1&restaurant_id=abc&bar=2");
        assert_eq!(query_value(query, "restaurant_id"), Some("abc".to_owned()));
        assert_eq!(query_value(query, "missing"), None);
        assert_eq!(query_value(None, "restaurant_id"), None);
    }
}
