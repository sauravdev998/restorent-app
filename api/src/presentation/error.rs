//! One error shape for every route.
//!
//! Handlers return [`DomainError`] and this maps it to a status code and a JSON
//! body. Doing it in one place is what stops the API growing five different
//! error shapes as features land, and it is what lets the generated TypeScript
//! client have a single error type.

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use utoipa::ToSchema;

use crate::domain::error::DomainError;

/// The body every failed request returns.
#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorBody {
    /// A stable machine readable code, safe to branch on in the client.
    #[schema(example = "not_found")]
    pub error: &'static str,
    /// A human readable sentence. Never contains internal detail.
    #[schema(example = "The requested resource does not exist.")]
    pub message: String,
}

/// A domain error on its way out as an HTTP response.
#[derive(Debug)]
pub struct ApiError(DomainError);

impl From<DomainError> for ApiError {
    fn from(error: DomainError) -> Self {
        Self(error)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code, message) = match &self.0 {
            DomainError::NotFound => (
                StatusCode::NOT_FOUND,
                "not_found",
                "The requested resource does not exist.".to_owned(),
            ),
            DomainError::Unauthenticated => (
                StatusCode::UNAUTHORIZED,
                "unauthenticated",
                "You are not signed in.".to_owned(),
            ),
            DomainError::Forbidden => (
                StatusCode::FORBIDDEN,
                "forbidden",
                "You are not allowed to do that.".to_owned(),
            ),
            DomainError::Invalid(reason) => (StatusCode::BAD_REQUEST, "invalid", reason.clone()),
            DomainError::Conflict(reason) => (StatusCode::CONFLICT, "conflict", reason.clone()),
            DomainError::Unavailable(what) => {
                // The caller did nothing wrong and must learn nothing about our
                // internals, so the detail goes to the logs and not the body.
                tracing::error!(dependency = %what, "request failed on an unavailable dependency");
                (
                    StatusCode::SERVICE_UNAVAILABLE,
                    "unavailable",
                    "The service is temporarily unavailable. Please try again.".to_owned(),
                )
            }
        };

        (
            status,
            Json(ErrorBody {
                error: code,
                message,
            }),
        )
            .into_response()
    }
}
