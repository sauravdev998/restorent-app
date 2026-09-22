//! The binary edge.
//!
//! The one place `anyhow` is allowed: everything below returns a typed error,
//! and this turns whatever reaches the top into a readable exit.

use std::sync::Arc;

use anyhow::Context as _;
use tokio::net::TcpListener;
use tokio::signal;

use api::domain::{country, language};
use api::infrastructure::config::Config;
use api::infrastructure::db::Database;
use api::infrastructure::events::{self, EventRegistry};
use api::infrastructure::health::SystemHealth;
use api::infrastructure::passwords::Argon2Passwords;
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

    // The same rule, applied to the one committed file the API reads as data.
    // Touching it here parses it once, at the boot, so a malformed catalogue is
    // a refused start rather than a puzzling 400 on the first request that
    // happens to write a language.
    let catalogue = language::catalogue().context("locales/catalogue.json is not usable")?;
    let countries = country::countries().context("locales/countries.json is not usable")?;

    telemetry::init(config.environment);
    tracing::info!(
        environment = ?config.environment,
        languages = catalogue.languages.len(),
        countries = countries.countries.len(),
        "starting the api"
    );

    let database = Database::connect(&config)
        .await
        .context("could not open the database connection pool")?;

    let registry = Arc::new(EventRegistry::new());

    // One listen connection for the whole process, taken from outside the pool
    // and held for the life of the process. One per connected screen would
    // exhaust Postgres by about the second restaurant.
    let listener_handle = events::spawn(config.database_url.clone(), Arc::clone(&registry));

    // Kept back for the shutdown, which has to be able to end the open streams.
    // See `shutdown_signal` for why the process cannot exit without it.
    let streams = Arc::clone(&registry);

    let state = AppState {
        health: SystemHealth::new(database.clone(), listener_handle),
        database,
        events: registry,
        passwords: Argon2Passwords::new(),
        environment: config.environment,
    };

    let app = router::build(state, &config);

    let listener = TcpListener::bind(config.bind_address)
        .await
        .with_context(|| format!("could not bind {}", config.bind_address))?;

    tracing::info!(address = %config.bind_address, "listening");

    // `into_make_service_with_connect_info` rather than the plain one, so the
    // socket's peer address reaches the request. It is what the client address
    // extractor reads in development, where there is no CloudFront to set the
    // header it reads everywhere else.
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal(streams))
    .await
    .context("the http server stopped unexpectedly")?;

    tracing::info!("shut down cleanly");
    Ok(())
}

/// Resolves when the process is asked to stop, once every open event stream on
/// this instance has been ended.
///
/// Ending them is not tidiness, it is what lets the process exit at all.
/// Graceful shutdown stops accepting new connections and then waits for the
/// requests already in flight, and an event stream is a request that finishes
/// only when somebody ends it. Left alone it waits for a stream that waits for
/// it: a rolling deploy hangs until the platform loses patience and kills the
/// task, and until then every kitchen screen holds a socket to a server that
/// will never send it anything again, with nothing on screen to say so.
async fn shutdown_signal(streams: Arc<EventRegistry>) {
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

    // Dropping every channel makes each stream's next `recv` return `Closed`,
    // which is the path the listen connection dropping already takes: the
    // stream ends, the browser reconnects, and it refetches what it missed.
    streams.close_all().await;
}
