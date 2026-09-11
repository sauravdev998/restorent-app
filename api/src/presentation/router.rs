//! Route table and the middleware stack around it.

use std::time::Duration;

use axum::Router;
use axum::http::StatusCode;
use axum::routing::{get, patch, post, put};
use tower_http::compression::CompressionLayer;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;

use crate::infrastructure::config::Config;

use super::handlers::{admin_menu, auth, billing, dev, events, health, me, menu, service};
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
        // The order thread. Each route's role lives in its handler's own
        // signature, not here, so a route added without one does not compile
        // rather than quietly admitting everybody.
        .route("/api/floor", get(service::floor))
        .route("/api/menu", get(menu::menu))
        .route("/api/visits", post(service::open_visit))
        .route("/api/visits/{id}", get(billing::visit))
        .route("/api/visits/{id}/rounds", post(service::send_round))
        .route("/api/visits/{id}/close", post(billing::close_visit))
        .route("/api/rounds/{id}/served", post(service::mark_round_served))
        .route("/api/kitchen/tickets", get(service::kitchen_tickets))
        .route(
            "/api/order-lines/{id}/ready",
            post(service::mark_line_ready),
        )
        // The menu. Admin only under `/api/admin`, and the one switch a chef
        // may throw beside the read it changes.
        .route("/api/admin/menu", get(admin_menu::admin_menu))
        .route(
            "/api/admin/menu/categories",
            post(admin_menu::create_category),
        )
        .route(
            "/api/admin/menu/categories/order",
            put(admin_menu::reorder_categories),
        )
        .route(
            "/api/admin/menu/categories/{id}",
            put(admin_menu::rename_category),
        )
        .route(
            "/api/admin/menu/categories/{id}/dish-order",
            put(admin_menu::reorder_dishes),
        )
        .route("/api/admin/menu/dishes", post(admin_menu::create_dish))
        .route("/api/admin/menu/dishes/{id}", put(admin_menu::edit_dish))
        .route("/api/dishes/{id}/availability", put(menu::set_availability))
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
