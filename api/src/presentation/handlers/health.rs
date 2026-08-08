//! `GET /api/health`, which the load balancer target group checks.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use utoipa::ToSchema;

use crate::application::health::{ComponentState, HealthReport, check_health};
use crate::presentation::state::AppState;

/// Whether one dependency is answering.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Component {
    /// Answering normally.
    Up,
    /// Not answering.
    Down,
}

impl From<ComponentState> for Component {
    fn from(state: ComponentState) -> Self {
        match state {
            ComponentState::Up => Self::Up,
            ComponentState::Down => Self::Down,
        }
    }
}

/// What the health endpoint returns.
///
/// A presentation DTO rather than the application's `HealthReport`, so that the
/// wire format can change without touching a use case, and so that no inner
/// layer has to know the `utoipa` schema traits exist.
#[derive(Debug, Serialize, ToSchema)]
pub struct HealthResponse {
    /// True only when every dependency is up.
    pub serving: bool,
    /// Whether the connection pool answers.
    pub database: Component,
    /// Whether this instance's Postgres listen connection is up.
    pub listener: Component,
}

impl From<HealthReport> for HealthResponse {
    fn from(report: HealthReport) -> Self {
        Self {
            serving: report.is_serving(),
            database: report.database.into(),
            listener: report.listener.into(),
        }
    }
}

/// Reports whether this instance can actually serve.
///
/// Returns 200 only when the pool answers **and** the listen connection is
/// alive. An instance whose listener has died still answers HTTP perfectly but
/// delivers no tickets to any kitchen screen, so a check that only proved the
/// process was up would leave it in service forever.
#[utoipa::path(
    get,
    path = "/api/health",
    tag = "system",
    responses(
        (status = 200, description = "Every dependency is up and this instance can serve.", body = HealthResponse),
        (status = 503, description = "A dependency is down. Take this instance out of service.", body = HealthResponse),
    )
)]
pub async fn health(State(state): State<AppState>) -> Response {
    let report = check_health(&state.health).await;
    let status = if report.is_serving() {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    (status, Json(HealthResponse::from(report))).into_response()
}
