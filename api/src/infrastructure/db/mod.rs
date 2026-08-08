//! Database access. The only module in the codebase that holds a [`PgPool`].
//!
//! Handlers never import this pool and cannot reach it: the field is private
//! and no method hands it out. The one way to run a query against tenant data
//! is [`Database::begin_scoped`], which returns a transaction that already has
//! the restaurant scope applied. Bypassing that is a compile error rather than
//! something a tired engineer has to remember not to do.

pub mod scoped;

use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;

use crate::domain::error::{DomainError, DomainResult};
use crate::domain::ids::RestaurantId;

use super::config::Config;
pub use scoped::ScopedTx;

/// Turns a SQLx failure into a domain error.
///
/// Lives here rather than in the domain layer so that `sqlx` stays out of the
/// inner layers entirely. The message is kept for the logs; the presentation
/// layer never shows it to a caller.
impl From<sqlx::Error> for DomainError {
    fn from(error: sqlx::Error) -> Self {
        match error {
            sqlx::Error::RowNotFound => Self::NotFound,
            other => {
                tracing::error!(error = %other, "database call failed");
                Self::Unavailable("database".to_owned())
            }
        }
    }
}

/// The connection pool, and the only door into it.
#[derive(Debug, Clone)]
pub struct Database {
    // Private on purpose. This is the structural half of tenant isolation:
    // nothing outside this module can name it, so nothing outside this module
    // can run an unscoped query.
    pool: PgPool,
}

impl Database {
    /// Opens the pool.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::Unavailable`] if the database will not accept a
    /// connection, which at boot means the process should not start.
    pub async fn connect(config: &Config) -> DomainResult<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(config.database_max_connections)
            .connect(&config.database_url)
            .await?;

        Ok(Self { pool })
    }

    /// Opens a transaction scoped to one restaurant.
    ///
    /// The scope is set with `set_config('app.restaurant_id', $1, true)`. The
    /// third argument is what makes it local to this transaction. Row level
    /// security policies added by feature 4 read it through the
    /// `current_restaurant_id()` helper.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::Unavailable`] if the transaction cannot be opened
    /// or the scope cannot be set. A failure here must never fall through to an
    /// unscoped query.
    pub async fn begin_scoped(&self, restaurant_id: RestaurantId) -> DomainResult<ScopedTx<'_>> {
        let mut tx = self.pool.begin().await?;

        sqlx::query!(
            "SELECT set_config('app.restaurant_id', $1, true)",
            restaurant_id.to_string()
        )
        .fetch_one(&mut *tx)
        .await?;

        Ok(ScopedTx::new(tx, restaurant_id))
    }

    /// Asks the pool a trivial question, for the health endpoint.
    ///
    /// Touches no tenant data, so it needs no scope.
    pub async fn is_reachable(&self) -> bool {
        match sqlx::query!("SELECT 1 AS ok").fetch_one(&self.pool).await {
            Ok(_) => true,
            Err(error) => {
                tracing::warn!(error = %error, "health probe could not reach the database");
                false
            }
        }
    }

    /// Publishes a change on the global notify channel, inside the caller's
    /// scoped transaction so it cannot name a restaurant the caller is not
    /// already scoped to.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::Unavailable`] if the notify call fails.
    pub async fn notify_entity_change(
        tx: &mut ScopedTx<'_>,
        entity: &str,
        entity_id: uuid::Uuid,
    ) -> DomainResult<()> {
        let restaurant_id = tx.restaurant_id().as_uuid();

        sqlx::query!(
            "SELECT notify_entity_change($1, $2, $3)",
            restaurant_id,
            entity,
            entity_id
        )
        .fetch_one(tx.connection())
        .await?;

        Ok(())
    }
}
