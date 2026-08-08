//! Structured logging, set up once at boot.
//!
//! Production writes JSON to stdout, which is what CloudWatch collects.
//! Development writes the human readable form, because nobody wants to read
//! JSON in a terminal. Crash and error monitoring proper is feature 3 and is a
//! separate thing from this.

use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use super::config::Environment;

/// Installs the global tracing subscriber.
///
/// # Panics
///
/// Panics if called twice, since a process has only one global subscriber.
pub fn init(environment: Environment) {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("api=debug,tower_http=debug,info"));

    let registry = tracing_subscriber::registry().with(filter);

    match environment {
        Environment::Production => {
            registry
                .with(tracing_subscriber::fmt::layer().json().flatten_event(true))
                .init();
        }
        Environment::Development => {
            registry
                .with(tracing_subscriber::fmt::layer().pretty())
                .init();
        }
    }
}
