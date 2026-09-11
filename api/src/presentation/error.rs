//! One error shape for every route.
//!
//! Handlers return [`DomainError`] and this maps it to a status code and a JSON
//! body. Doing it in one place is what stops the API growing five different
//! error shapes as features land, and it is what lets the generated TypeScript
//! client have a single error type.

use std::collections::BTreeMap;

use axum::Json;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use utoipa::ToSchema;

use crate::domain::error::{DomainError, FieldErrors};

/// The body every failed request returns.
#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorBody {
    /// A stable machine readable code, safe to branch on in the client.
    ///
    /// A refused conflict names which conflict it was, such as
    /// `table_occupied` or `bill_has_unserved_lines`, rather than the blanket
    /// word `conflict`. The closed list is
    /// [`ConflictKind`](crate::domain::error::ConflictKind).
    #[schema(example = "not_found")]
    pub error: &'static str,
    /// A human readable sentence. Never contains internal detail.
    ///
    /// English, and for a log. The web app renders a translation of
    /// [`Self::error`] instead, because the API is a data API and knows nothing
    /// about what language anybody reads.
    #[schema(example = "The requested resource does not exist.")]
    pub message: String,
    /// What is wrong with each named field, when the problem is a form rather
    /// than the request as a whole.
    ///
    /// Absent from every response that has no field level problem, which is
    /// most of them, so nothing that was already reading this body changes.
    /// Each value is one of a closed set of codes: `already_taken`,
    /// `too_short`, `too_long`, `invalid_format`, `unknown_country`,
    /// `not_in_catalogue`, `incorrect`, `required`, `not_a_number`,
    /// `negative`, `too_large`, `too_many_decimals`.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = json!({ "email": "already_taken" }))]
    pub fields: Option<BTreeMap<String, &'static str>>,
}

/// A domain error on its way out as an HTTP response.
#[derive(Debug)]
pub struct ApiError(DomainError);

impl From<DomainError> for ApiError {
    fn from(error: DomainError) -> Self {
        Self(error)
    }
}

impl ApiError {
    /// The response body for a set of field level problems.
    fn field_body(errors: &FieldErrors) -> BTreeMap<String, &'static str> {
        errors
            .pairs()
            .iter()
            .map(|(field, error)| (field.clone(), error.as_code()))
            .collect()
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        if let DomainError::InvalidFields(errors) = &self.0 {
            // Deliberately the same `error` code and the same status as any
            // other invalid request, so a client that has never heard of
            // `fields` still behaves exactly as it did before.
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorBody {
                    error: "invalid",
                    message: "One or more fields were not accepted.".to_owned(),
                    fields: Some(Self::field_body(errors)),
                }),
            )
                .into_response();
        }

        if let DomainError::Throttled(seconds) = self.0 {
            // The one response in the product that carries a header a client is
            // meant to read. Without `Retry-After` a browser has no idea
            // whether to try again in a second or in a quarter of an hour.
            let mut response = (
                StatusCode::TOO_MANY_REQUESTS,
                Json(ErrorBody {
                    error: "throttled",
                    message: "Too many attempts. Try again shortly.".to_owned(),
                    fields: None,
                }),
            )
                .into_response();

            if let Ok(value) = HeaderValue::from_str(&seconds.to_string()) {
                response.headers_mut().insert(header::RETRY_AFTER, value);
            }

            return response;
        }

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
            // Both answered above, before this match: one carries a body member
            // the others do not, and the other carries a header.
            DomainError::Throttled(_) => (
                StatusCode::TOO_MANY_REQUESTS,
                "throttled",
                "Too many attempts. Try again shortly.".to_owned(),
            ),
            DomainError::InvalidFields(_) => (
                StatusCode::BAD_REQUEST,
                "invalid",
                "One or more fields were not accepted.".to_owned(),
            ),
            // The code names what actually happened, not the blanket word
            // "conflict". That is what lets the web say "that table already has
            // a party at it" in the reader's own language instead of showing
            // one English sentence written in Rust.
            DomainError::Conflict(kind) => (StatusCode::CONFLICT, kind.as_code(), kind.to_string()),
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
                fields: None,
            }),
        )
            .into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::domain::error::FieldError;

    fn body_of(error: DomainError) -> serde_json::Value {
        // Rendered through `IntoResponse` rather than by building an `ErrorBody`
        // by hand, because what is under test is the bytes a caller receives.
        let response = ApiError::from(error).into_response();
        let (parts, body) = response.into_parts();

        let bytes = futures::executor::block_on(axum::body::to_bytes(body, usize::MAX))
            .expect("the error body is small enough to read whole");

        let mut json: serde_json::Value =
            serde_json::from_slice(&bytes).expect("the error body is JSON");
        json["status"] = parts.status.as_u16().into();
        json
    }

    /// covers: AC-2
    ///
    /// The whole point of putting `fields` on the existing shape rather than
    /// inventing a second one: every response that has no field problem has to
    /// look exactly as it did before this feature.
    #[test]
    fn an_error_with_no_field_problem_carries_no_fields_member_at_all() {
        let json = body_of(DomainError::NotFound);

        assert_eq!(json["error"], "not_found");
        assert_eq!(json["status"], 404);
        assert!(
            json.get("fields").is_none(),
            "a plain error grew a fields member: {json}"
        );
    }

    /// covers: AC-2
    #[test]
    fn a_field_problem_rides_on_the_same_shape_and_the_same_code() {
        let json = body_of(DomainError::InvalidFields(FieldErrors::one(
            "email",
            FieldError::AlreadyTaken,
        )));

        assert_eq!(json["status"], 400);
        assert_eq!(
            json["error"], "invalid",
            "a field problem must look like any other invalid request to a client that has \
             never heard of the fields member"
        );
        assert_eq!(json["fields"]["email"], "already_taken");
    }

    /// covers: AC-2
    #[test]
    fn a_missing_required_field_is_reported_as_that_field_being_required() {
        let json = body_of(DomainError::InvalidFields(FieldErrors::one(
            "password",
            FieldError::Required,
        )));

        assert_eq!(json["fields"]["password"], "required");
    }

    /// Several problems with one form arrive together, so somebody filling it
    /// in fixes everything at once rather than one field per round trip.
    #[test]
    fn every_field_problem_arrives_in_the_one_response() {
        let mut errors = FieldErrors::one("email", FieldError::InvalidFormat);
        errors.add("password", FieldError::TooShort);
        errors.add("countryCode", FieldError::UnknownCountry);

        let json = body_of(DomainError::InvalidFields(errors));

        assert_eq!(json["fields"]["email"], "invalid_format");
        assert_eq!(json["fields"]["password"], "too_short");
        assert_eq!(json["fields"]["countryCode"], "unknown_country");
    }

    /// covers: AC-10
    ///
    /// The header is the whole point of answering `429` rather than `401`: it
    /// is the only thing that tells a client whether to try again in a second or
    /// in a quarter of an hour.
    #[test]
    fn a_throttled_request_says_how_long_to_wait() {
        let response = ApiError::from(DomainError::Throttled(420)).into_response();

        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(
            response
                .headers()
                .get(header::RETRY_AFTER)
                .and_then(|value| value.to_str().ok()),
            Some("420"),
            "a throttled response with no Retry-After tells a client nothing"
        );

        let json = body_of(DomainError::Throttled(420));
        assert_eq!(json["error"], "throttled");
        assert!(
            json.get("fields").is_none(),
            "a throttled response must not name a field, or it says which address is real"
        );
    }

    /// covers: AC-4
    ///
    /// A wrong password and an unknown address both arrive here as the same
    /// variant, so this is where "identical response" is either true or not.
    #[test]
    fn every_unauthenticated_answer_is_byte_for_byte_the_same() {
        assert_eq!(
            body_of(DomainError::Unauthenticated),
            body_of(DomainError::Unauthenticated)
        );

        let json = body_of(DomainError::Unauthenticated);
        assert_eq!(json["status"], 401);
        assert_eq!(json["error"], "unauthenticated");
        assert!(
            json.get("fields").is_none(),
            "a refused sign in must not name a field, or it says which half was wrong"
        );
    }
}
