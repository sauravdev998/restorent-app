//! Database access. The only module in the codebase that holds a [`PgPool`].
//!
//! Handlers never import this pool and cannot reach it: the field is private
//! and no method hands it out. The one way to run a query against tenant data
//! is [`Database::begin_scoped`], which returns a transaction that already has
//! the restaurant scope applied. Bypassing that is a compile error rather than
//! something a tired engineer has to remember not to do.

pub mod scoped;

use std::time::Duration;

use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;

use crate::domain::error::{DomainError, DomainResult};
use crate::domain::ids::RestaurantId;

use super::config::Config;
pub use scoped::ScopedTx;

/// How long a caller may wait for a pooled connection before giving up.
///
/// `SQLx` defaults this to 30 seconds, which is exactly the router's request
/// timeout. With the database down the two race, the timeout layer wins, and the
/// caller gets a bodiless `408` instead of the `503` that says which component
/// is down. Anything comfortably under the request timeout keeps the error the
/// application chose rather than one the middleware imposed.
const ACQUIRE_TIMEOUT: Duration = Duration::from_secs(5);

/// How long the health probe may take before it reports the database as down.
///
/// Both health checks, the load balancer's and the container's, time out after
/// 5 seconds. A probe that answers later than that is not an answer at all, so
/// this sits well below it: a database that has not produced a connection in two
/// seconds is down as far as a kitchen screen is concerned.
const HEALTH_PROBE_TIMEOUT: Duration = Duration::from_secs(2);

/// Turns a `SQLx` failure into a domain error.
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
            .acquire_timeout(ACQUIRE_TIMEOUT)
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
    ///
    /// Bounded by [`HEALTH_PROBE_TIMEOUT`], and the bound is the point. A probe
    /// that waits as long as the pool allows turns a down database into a health
    /// check that never answers, which reads as a timeout rather than as an
    /// unhealthy instance. Dropping the future on timeout also releases the pool
    /// waiter, so repeated probes during an outage do not pile up behind each
    /// other.
    pub async fn is_reachable(&self) -> bool {
        let probe = sqlx::query!("SELECT 1 AS ok").fetch_one(&self.pool);

        match tokio::time::timeout(HEALTH_PROBE_TIMEOUT, probe).await {
            Ok(Ok(_)) => true,
            Ok(Err(error)) => {
                tracing::warn!(error = %error, "health probe could not reach the database");
                false
            }
            Err(_) => {
                tracing::warn!(
                    timeout_secs = HEALTH_PROBE_TIMEOUT.as_secs(),
                    "health probe gave up waiting for the database"
                );
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The router cuts an ordinary request off after 30 seconds, and both health
    /// checks give up after 5. These bounds only mean anything while they stay
    /// under those numbers.
    ///
    /// Guards the bug where the health endpoint blocked for the pool's default
    /// 30 second acquire timeout, lost the race with the router's own 30 second
    /// timeout, and answered `408` with no body instead of `503` naming the
    /// component that was down.
    #[test]
    fn the_database_waits_are_shorter_than_the_timeouts_around_them() {
        const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
        const HEALTH_CHECK_TIMEOUT: Duration = Duration::from_secs(5);

        assert!(
            ACQUIRE_TIMEOUT < REQUEST_TIMEOUT,
            "a caller waiting for a connection must give up before the router gives up on it, \
             otherwise the response is a bodiless 408 instead of the error the handler chose"
        );
        assert!(
            HEALTH_PROBE_TIMEOUT < HEALTH_CHECK_TIMEOUT,
            "a health probe slower than the health check itself is not an answer"
        );
    }

    /// A row that is not there is the caller's answer, not a broken dependency.
    ///
    /// The two map to different status codes, so collapsing them would turn
    /// every empty lookup into a `503` and make a healthy instance look down.
    #[test]
    fn a_missing_row_is_reported_as_not_found_rather_than_an_outage() {
        let mapped = DomainError::from(sqlx::Error::RowNotFound);

        assert!(
            matches!(mapped, DomainError::NotFound),
            "a missing row became {mapped:?}, which reads as the database being down"
        );
    }

    #[test]
    fn any_other_database_failure_is_reported_as_the_database_being_unavailable() {
        let mapped = DomainError::from(sqlx::Error::PoolTimedOut);

        match mapped {
            DomainError::Unavailable(component) => assert_eq!(component, "database"),
            other => panic!("a pool timeout became {other:?} instead of naming the database"),
        }
    }

    /// The `SQLx` message stays in the logs and never rides out on the response.
    ///
    /// Database errors quote connection strings, host addresses, and sometimes
    /// credentials. One error shape on every API response is only worth having
    /// if the shape cannot be widened by whatever the driver happened to say.
    #[test]
    fn a_database_failure_never_carries_its_message_out_to_the_caller() {
        let leaky = sqlx::Error::Protocol(
            "connecting as app_api with password=hunter2 to 10.0.0.4:5432 failed".to_owned(),
        );

        let shown = DomainError::from(leaky).to_string();

        assert!(
            !shown.contains("hunter2"),
            "a credential reached the caller in {shown:?}"
        );
        assert!(
            !shown.contains("10.0.0.4"),
            "an internal address reached the caller in {shown:?}"
        );
        assert_eq!(shown, "dependency unavailable: database");
    }
}
