//! What every handler is given.
//!
//! Note what is not here: a `PgPool`. The only database door is
//! [`Database::begin_scoped`], and that returns an already scoped transaction.

use std::sync::Arc;

use crate::infrastructure::config::Environment;
use crate::infrastructure::db::Database;
use crate::infrastructure::events::EventRegistry;
use crate::infrastructure::health::SystemHealth;
use crate::infrastructure::passwords::Argon2Passwords;

/// Shared application state. Cheap to clone: everything inside is already
/// reference counted.
#[derive(Debug, Clone)]
pub struct AppState {
    /// The only way to reach the database, and only through a scoped
    /// transaction.
    pub database: Database,
    /// The open streams on this instance.
    pub events: Arc<EventRegistry>,
    /// What the health endpoint asks.
    pub health: SystemHealth,
    /// Hashes and verifies passwords, off the async runtime.
    pub passwords: Argon2Passwords,
    /// Which environment this is. Read by the cookie builder, which sets
    /// `Secure` everywhere except development, and by the router, which mounts
    /// the development only routes.
    pub environment: Environment,
}
