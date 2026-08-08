//! The adapter that lets the health use case ask real dependencies how they are.
//!
//! It implements a trait declared in the application layer, which is what keeps
//! the dependency rule pointing inward: the use case knows this trait, not this
//! struct, and certainly not `PgPool`.

use crate::application::ports::HealthPort;

use super::db::Database;
use super::events::ListenerHandle;

/// Everything this instance needs in order to be useful, in one place.
#[derive(Debug, Clone)]
pub struct SystemHealth {
    database: Database,
    listener: ListenerHandle,
}

impl SystemHealth {
    /// Builds the adapter from the live dependencies.
    #[must_use]
    pub const fn new(database: Database, listener: ListenerHandle) -> Self {
        Self { database, listener }
    }
}

impl HealthPort for SystemHealth {
    async fn database_reachable(&self) -> bool {
        self.database.is_reachable().await
    }

    fn listener_alive(&self) -> bool {
        self.listener.is_alive()
    }
}
