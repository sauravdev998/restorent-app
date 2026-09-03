//! Reading a JSON request body, and answering in this product's own shape when
//! it cannot be read.
//!
//! This exists for one reason. Axum's own [`Json`](axum::Json) extractor refuses
//! a body with a missing field before any handler runs, and it answers in its
//! own shape: a plain text sentence with no `error` code and no `fields`. That
//! would be the one response in the whole product that does not look like every
//! other one, and it would be the response a registration form gets most often
//! while somebody is building against it.
//!
//! So every handler in the project takes `JsonBody<T>` rather than `Json<T>`
//! from here on. A missing field becomes `fields.<name>=required` on the
//! ordinary error body, and anything else unreadable becomes an ordinary
//! `invalid`.

use axum::body::Bytes;
use axum::extract::{FromRequest, Request};
use serde::de::DeserializeOwned;

use crate::domain::error::{DomainError, FieldError, FieldErrors};
use crate::presentation::error::ApiError;

/// A request body, deserialised, refusing in this product's own error shape.
#[derive(Debug, Clone, Copy, Default)]
pub struct JsonBody<T>(
    /// The deserialised body.
    pub T,
);

impl<T, S> FromRequest<S> for JsonBody<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request(request: Request, state: &S) -> Result<Self, Self::Rejection> {
        let bytes = Bytes::from_request(request, state).await.map_err(|error| {
            // The body could not be read off the socket at all: too large, or
            // the connection died halfway. Nothing field level to say.
            tracing::debug!(error = %error, "could not read a request body");
            ApiError::from(DomainError::Invalid(
                "the request body could not be read".to_owned(),
            ))
        })?;

        serde_json::from_slice(&bytes)
            .map(Self)
            .map_err(|error| ApiError::from(refusal(&error.to_string())))
    }
}

/// Turns what `serde_json` said into what the caller is told.
///
/// The only message shape it reads is the one naming a missing field, which is
/// stable across `serde_json` and is the one case an acceptance criterion
/// names. Every other parse failure is an ordinary invalid request: guessing a
/// field name out of "invalid type: integer, expected a string" would put a
/// field code on a body that might name no field at all.
fn refusal(message: &str) -> DomainError {
    match missing_field(message) {
        Some(field) => DomainError::InvalidFields(FieldErrors::one(field, FieldError::Required)),
        None => {
            // Kept out of the response: a parse error quotes the body back, and
            // the body of a registration or a sign in holds a password.
            tracing::debug!("a request body could not be deserialised");
            DomainError::Invalid("the request body could not be read".to_owned())
        }
    }
}

/// The field name out of a message reading:
/// `missing field ...email... at line 1 column 20`, where the dots are the
/// backticks `serde_json` puts around the name.
fn missing_field(message: &str) -> Option<&str> {
    let after = message.strip_prefix("missing field `")?;
    after.split('`').next().filter(|name| !name.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    #[allow(dead_code)]
    struct Body {
        email: String,
        password: String,
    }

    fn refusal_for(raw: &str) -> DomainError {
        let error = serde_json::from_str::<Body>(raw).expect_err("this body should not parse");
        refusal(&error.to_string())
    }

    /// covers: AC-2
    #[test]
    fn a_missing_field_names_itself_as_required() {
        let refused = refusal_for(r#"{"email":"ada@example.com"}"#);

        match refused {
            DomainError::InvalidFields(errors) => {
                assert_eq!(
                    errors.pairs(),
                    [("password".to_owned(), FieldError::Required)]
                );
            }
            other => panic!("a missing field became {other:?} rather than a field error"),
        }
    }

    /// covers: AC-2
    ///
    /// The one thing this extractor exists to prevent: a caller receiving
    /// Axum's own rejection shape instead of the product's.
    #[test]
    fn every_unreadable_body_still_becomes_this_products_own_error() {
        for raw in [
            "",
            "not json at all",
            "[]",
            r#"{"email": 7, "password": "hunter2hunter2"}"#,
        ] {
            let refused = refusal_for(raw);

            assert!(
                matches!(
                    refused,
                    DomainError::Invalid(_) | DomainError::InvalidFields(_)
                ),
                "{raw:?} became {refused:?}, which is not a 400 in this product's shape"
            );
        }
    }

    /// A parse failure quotes the body back, and the body of a sign in holds a
    /// password. Nothing `serde_json` said may reach the response.
    #[test]
    fn a_parse_failure_never_carries_the_body_out_to_the_caller() {
        let refused = refusal_for(r#"{"email": 7, "password": "correct horse battery"}"#);

        let shown = refused.to_string();
        assert!(
            !shown.contains("correct horse"),
            "the password reached the caller in {shown:?}"
        );
    }

    #[test]
    fn a_message_that_is_not_about_a_missing_field_names_no_field() {
        assert_eq!(missing_field("expected value at line 1 column 1"), None);
        assert_eq!(missing_field("missing field `` at line 1"), None);
        assert_eq!(
            missing_field("missing field `countryCode` at line 1 column 60"),
            Some("countryCode")
        );
    }
}
