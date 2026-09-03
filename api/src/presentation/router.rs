//! Route table and the middleware stack around it.

use std::time::Duration;

use axum::Router;
use axum::http::StatusCode;
use axum::routing::{get, patch, post};
use tower_http::compression::CompressionLayer;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;

use crate::infrastructure::config::Config;

use super::handlers::{auth, dev, events, health, me};
use super::origin;
use super::state::AppState;

/// How long an ordinary request may take before it is cut off.
///
/// The event stream is mounted outside this: it is meant to stay open for the
/// whole of a dinner service, and a timeout would close it every 30 seconds.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Builds the application router.
pub fn build(state: AppState, config: &Config) -> Router {
    // Long lived, and deliberately outside the timeout and compression layers.
    // Compressing a stream buffers it, which is the one thing a live ticket
    // feed must not do.
    let streaming = Router::new().route("/api/events", get(events::events));

    let mut request_response = Router::new()
        .route("/api/health", get(health::health))
        .route("/api/auth/register", post(auth::register))
        .route("/api/auth/sign-in", post(auth::sign_in))
        .route("/api/auth/sign-out", post(auth::sign_out))
        .route("/api/me", get(auth::me).patch(me::update_me))
        .route("/api/me/password", post(me::change_password))
        .route("/api/restaurant", patch(me::update_restaurant))
        .layer(CompressionLayer::new())
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            REQUEST_TIMEOUT,
        ));

    if config.is_development() {
        tracing::warn!("mounting development only routes under /api/dev");
        request_response = request_response.route("/api/dev/notify", post(dev::notify));
    }

    streaming
        .merge(request_response)
        // Outside both routers, so it covers every mutating route including any
        // added later. A check mounted per route is a check a new route can be
        // added without.
        .layer(axum::middleware::from_fn(origin::same_origin))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
