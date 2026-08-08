//! Interfaces the application needs, which the infrastructure layer implements.
//!
//! This is where the dependency rule is bought: the use case below depends on
//! this trait, and the database and listener code depends on it too, so neither
//! the use case nor the domain has to know a `PgPool` exists.

use std::future::Future;

/// Whatever can answer "is this instance actually able to serve".
pub trait HealthPort: Send + Sync {
    /// True when the connection pool answers a trivial query right now.
    fn database_reachable(&self) -> impl Future<Output = bool> + Send;

    /// True when this instance's Postgres listen connection is still up.
    ///
    /// Deliberately not async: it reads a flag the listener task keeps current,
    /// so a health check can never hang waiting on the very thing that died.
    fn listener_alive(&self) -> bool;
}
