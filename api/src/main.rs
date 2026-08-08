//! The binary edge.
//!
//! The one place `anyhow` is allowed: everything below returns a typed error,
//! and this turns whatever reaches the top into a readable exit.

use std::sync::Arc;

use anyhow::Context as _;
use tokio::net::TcpListener;
use tokio::signal;

use api::infrastructure::config::Config;
use api::infrastructure::db::Database;
use api::infrastructure::events::{self, EventRegistry};
use api::infrastructure::health::SystemHealth;
use api::infrastructure::telemetry;
use api::presentation::router;
use api::presentation::state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // A missing .env is normal: in production the values come from the task
    // definition, not a file.
    let _ = dotenvy::dotenv();

    // Before anything opens a socket. A missing or malformed setting fails the
    // deploy here, in the logs, rather than failing the first request in the
    // middle of a service.
    let config = Config::from_env().context("configuration is not usable")?;

    telemetry::init(config.environment);
    tracing::info!(environment = ?config.environment, "starting the api");

    let database = Database::connect(&config)
        .await
        .context("could not open the database connection pool")?;

    let registry = Arc::new(EventRegistry::new());

    // One listen connection for the whole process, taken from outside the pool
    // and held for the life of the process. One per connected screen would
    // exhaust Postgres by about the second restaurant.
    let listener_handle = events::spawn(config.database_url.clone(), Arc::clone(&registry));

    let state = AppState {
        health: SystemHealth::new(database.clone(), listener_handle),
        database,
        events: registry,
        environment: config.environment,
    };

    let app = router::build(state, &config);

    let listener = TcpListener::bind(config.bind_address)
        .await
        .with_context(|| format!("could not bind {}", config.bind_address))?;

    tracing::info!(address = %config.bind_address, "listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("the http server stopped unexpectedly")?;

    tracing::info!("shut down cleanly");
    Ok(())
}

/// Resolves when the process is asked to stop.
///
/// Worth having even before there is much to clean up: a rolling deploy sends
/// SIGTERM, and every open event stream on this instance ends when it does.
async fn shutdown_signal() {
    let interrupt = async {
        signal::ctrl_c()
            .await
            .expect("could not install the ctrl-c handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("could not install the terminate handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = interrupt => tracing::info!("interrupt received, shutting down"),
        () = terminate => tracing::info!("terminate received, shutting down"),
    }
}
