//! The OpenAPI document, generated from the handlers themselves.
//!
//! This is the honest half of the seam between two languages. `openapi.json` is
//! generated from here, the TypeScript client is generated from that, and both
//! are committed. Continuous integration regenerates them and fails if the
//! result differs, so renaming a field in Rust breaks the pull request instead
//! of breaking a Saturday night.
//!
//! A new route must be listed in `paths` below, or it will not reach the client.

use utoipa::OpenApi;

use super::error::ErrorBody;
use super::handlers::{events, health};

/// The whole public API surface.
#[derive(OpenApi)]
#[openapi(
    info(
        title = "Restaurant operations platform API",
        description = "Orders at the table, tickets live in the kitchen, a bill at the end.",
        version = "0.1.0",
    ),
    paths(health::health, events::events),
    components(schemas(
        ErrorBody,
        health::HealthResponse,
        health::Component,
        events::StreamEvent,
    )),
    tags((name = "system", description = "Health and live updates."))
)]
pub struct ApiDoc;

/// The document as pretty JSON, for `openapi.json`.
///
/// # Errors
///
/// Returns an error only if the document cannot be serialised, which would mean
/// a broken schema annotation.
pub fn to_pretty_json() -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(&ApiDoc::openapi())
}
