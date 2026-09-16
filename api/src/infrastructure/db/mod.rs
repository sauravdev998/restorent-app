//! Database access. The only module in the codebase that holds a [`PgPool`].
//!
//! Handlers never import this pool and cannot reach it: the field is private
//! and no method hands it out. The one way to run a query against tenant data
//! is [`Database::begin_scoped`], which returns a transaction that already has
//! the restaurant scope applied. Bypassing that is a compile error rather than
//! something a tired engineer has to remember not to do.

pub mod pg_enum;
pub mod repository;
pub mod scoped;

use std::time::Duration;

use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;

use crate::domain::enums::StaffRole;
use crate::domain::error::{DomainError, DomainResult};
use crate::domain::event::EntityKind;
use crate::domain::ids::{RestaurantId, SessionId, StaffId};
use crate::domain::people::{ResolvedSession, StaffCredentials};
use crate::domain::throttle::{MAX_ATTEMPTS_PER_EMAIL, MAX_ATTEMPTS_PER_IP, THROTTLE_WINDOW};

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

        Self::apply_scope(&mut tx, restaurant_id).await?;

        Ok(ScopedTx::new(tx, restaurant_id))
    }

    /// The same, but every statement in it sees one moment in time.
    ///
    /// For a read that assembles a document out of several statements, which is
    /// what the visit screen, the kitchen queue, and the floor all are.
    ///
    /// The default isolation level is `READ COMMITTED`, and under it each
    /// statement takes its own fresh snapshot. That is fine for a single query
    /// and quietly wrong for a document: a chef's write can commit between two
    /// of the statements, and the answer then carries half of it. The shape that
    /// actually reached a waiter's screen was a ticket reading "cooking" with
    /// every dish on it reading "ready", which is a state that never existed in
    /// the database and which stopped the ready alert from ever firing. The
    /// window is milliseconds on a database next door and a good deal wider on
    /// one across the internet, which is the sort of bug that hides in
    /// development and appears on a busy Friday.
    ///
    /// `REPEATABLE READ` takes one snapshot for the whole transaction, so every
    /// statement in it agrees. It costs nothing for a read only transaction:
    /// serialization failures under this level come from writes, and there are
    /// none here. Use [`Self::begin_scoped`] for anything that writes.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::Unavailable`] if the transaction cannot be opened.
    pub async fn begin_scoped_snapshot(
        &self,
        restaurant_id: RestaurantId,
    ) -> DomainResult<ScopedTx<'_>> {
        let mut tx = self.pool.begin().await?;

        // Before anything else, including the scope. Postgres refuses to change
        // the isolation level once a statement has run in the transaction.
        sqlx::raw_sql("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ")
            .execute(&mut *tx)
            .await?;

        Self::apply_scope(&mut tx, restaurant_id).await?;

        Ok(ScopedTx::new(tx, restaurant_id))
    }

    /// Puts the restaurant on the transaction, for the row level security
    /// policies to read.
    ///
    /// `set_config(..., true)` is `SET LOCAL`: it lasts for this transaction
    /// only, so a pooled connection can never carry one request's restaurant
    /// into the next request.
    async fn apply_scope(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        restaurant_id: RestaurantId,
    ) -> DomainResult<()> {
        sqlx::query!(
            "SELECT set_config('app.restaurant_id', $1, true)",
            restaurant_id.to_string()
        )
        .fetch_one(&mut **tx)
        .await?;

        Ok(())
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
    /// Takes an [`EntityKind`] rather than a string, so the closed vocabulary
    /// the listener understands is enforced by the compiler instead of by
    /// everyone spelling `order_round` the same way.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::Unavailable`] if the notify call fails.
    pub async fn notify_entity_change(
        tx: &mut ScopedTx<'_>,
        entity: EntityKind,
        entity_id: uuid::Uuid,
    ) -> DomainResult<()> {
        let restaurant_id = tx.restaurant_id().as_uuid();

        sqlx::query!(
            "SELECT notify_entity_change($1, $2, $3)",
            restaurant_id,
            entity.as_label(),
            entity_id
        )
        .fetch_one(tx.connection())
        .await?;

        Ok(())
    }

    /// Finds an account by email, across every restaurant on the platform.
    ///
    /// One of exactly two reads in the whole system that happen without a
    /// restaurant scope, and it exists because sign in has a chicken and egg
    /// problem: you cannot scope a transaction to a restaurant until you know
    /// which restaurant the person belongs to.
    ///
    /// The bypass is not this method. It is the `find_staff_for_login` function
    /// in the database, which is `SECURITY DEFINER`, owned by the `auth_lookup`
    /// role, and returns a fixed narrow shape. Everything this method could
    /// possibly learn is decided there, in SQL anybody can read, rather than
    /// here.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::Unavailable`] if the lookup cannot run. An address
    /// nobody has returns [`Ok(None)`], which callers must not let the caller
    /// tell apart from a wrong password.
    pub async fn find_staff_for_login(
        &self,
        email: &str,
    ) -> DomainResult<Option<StaffCredentials>> {
        let found = sqlx::query!(
            r#"
            SELECT staff_id      AS "staff_id!",
                   restaurant_id AS "restaurant_id!",
                   role          AS "role!: StaffRole",
                   password_hash AS "password_hash!",
                   deactivated_at
            FROM find_staff_for_login($1)
            "#,
            email
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(found.map(|row| StaffCredentials {
            staff_id: StaffId::from_uuid(row.staff_id),
            restaurant_id: RestaurantId::from_uuid(row.restaurant_id),
            role: row.role,
            password_hash: row.password_hash,
            deactivated_at: row.deactivated_at,
        }))
    }

    /// Works out whose session a token belongs to, across every restaurant.
    ///
    /// The other of exactly two unscoped reads. Its answer's `restaurant_id` is
    /// what every later query in the request is scoped to, which makes this the
    /// value the whole tenant isolation story hangs from.
    ///
    /// An expired or revoked session yields [`Ok(None)`]. Whether the account
    /// behind it is deactivated is feature 7's question, not this one's.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::Unavailable`] if the lookup cannot run.
    pub async fn resolve_session(
        &self,
        token_hash: &[u8],
    ) -> DomainResult<Option<ResolvedSession>> {
        let found = sqlx::query!(
            r#"
            SELECT session_id    AS "session_id!",
                   staff_id      AS "staff_id!",
                   restaurant_id AS "restaurant_id!",
                   role          AS "role!: StaffRole",
                   expires_at    AS "expires_at!",
                   last_seen_at  AS "last_seen_at!"
            FROM resolve_session($1)
            "#,
            token_hash
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(found.map(|row| ResolvedSession {
            session_id: SessionId::from_uuid(row.session_id),
            staff_id: StaffId::from_uuid(row.staff_id),
            restaurant_id: RestaurantId::from_uuid(row.restaurant_id),
            role: row.role,
            expires_at: row.expires_at,
            last_seen_at: row.last_seen_at,
        }))
    }

    /// Counts the recent attempts for this address and this caller, records
    /// this one, and refuses if either bucket is already full.
    ///
    /// Called before the password is checked, on both `/api/auth/sign-in` and
    /// `/api/auth/register`. Registration is included because it answers
    /// whether an address is taken, so without a throttle it would be a way to
    /// work through a list of addresses at speed.
    ///
    /// Recording before the check is what keeps a flood cheap to refuse, and it
    /// is why [`Database::clear_login_attempts`] exists: the attempt that works
    /// takes its own row with it, so what is left to count is failures.
    ///
    /// # Why this is unscoped, and why that is not a third door
    ///
    /// `login_attempts` has no `restaurant_id`, no foreign key, and no row
    /// level security. It could not have one: a failed sign in happens before
    /// anybody knows which restaurant the address belongs to, and often for an
    /// address that belongs to none. It holds nothing that answers a question
    /// about any restaurant, so the two cross tenant reads spec 0003 named are
    /// still the only two.
    ///
    /// # Why the advisory lock
    ///
    /// The count and this attempt's own insert have to be one atomic step. Ten
    /// requests for one address arriving together would otherwise every one of
    /// them read a count below the limit, before any of them had committed a
    /// row, and every one of them would be allowed. The lock is transaction
    /// level and keyed on the lowered address, so it is released by the commit
    /// and two different addresses never wait on each other.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::Throttled`] with the seconds until the window
    /// clears when either bucket is full, and [`DomainError::Unavailable`] if a
    /// statement fails.
    pub async fn record_login_attempt(
        &self,
        email: &str,
        client_address: Option<std::net::IpAddr>,
    ) -> DomainResult<()> {
        let lowered = email.trim().to_lowercase();
        let window = THROTTLE_WINDOW.as_secs_f64();

        let mut tx = self.pool.begin().await?;

        // `hashtext` rather than a hash computed in Rust, so the key is a pure
        // function of the address that anybody reading this SQL can reproduce.
        sqlx::query!("SELECT pg_advisory_xact_lock(hashtext($1))", lowered)
            .fetch_one(&mut *tx)
            .await?;

        let counts = sqlx::query!(
            r#"
            SELECT
                count(*) FILTER (WHERE email = $1)                       AS "by_email!",
                count(*) FILTER (WHERE $2::inet IS NOT NULL AND ip = $2) AS "by_address!"
            FROM login_attempts
            WHERE attempted_at > now() - make_interval(secs => $3)
            "#,
            lowered,
            client_address.map(sqlx::types::ipnetwork::IpNetwork::from),
            window,
        )
        .fetch_one(&mut *tx)
        .await?;

        if counts.by_email >= MAX_ATTEMPTS_PER_EMAIL || counts.by_address >= MAX_ATTEMPTS_PER_IP {
            // The transaction is dropped, so this attempt is not recorded.
            // Recording it would let somebody hold a locked out account locked
            // out for as long as they kept knocking, which turns a fifteen
            // minute wait into an indefinite one.
            return Err(DomainError::Throttled(THROTTLE_WINDOW.as_secs()));
        }

        sqlx::query!(
            "INSERT INTO login_attempts (id, email, ip) VALUES ($1, $2, $3)",
            uuid::Uuid::now_v7(),
            lowered,
            client_address.map(sqlx::types::ipnetwork::IpNetwork::from),
        )
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(())
    }

    /// Empties this one address's bucket, every row of it.
    ///
    /// Run on a successful sign in and a successful registration, and this is
    /// the half that makes [`MAX_ATTEMPTS_PER_EMAIL`] a count of failures.
    /// Attempts are recorded before the password is checked, so without this
    /// the row a working sign in leaves behind would count against the next
    /// one, and five ordinary sign ins on five devices would lock somebody out
    /// of their own restaurant for a quarter of an hour.
    ///
    /// Only somebody who holds the account can reach it, which is what keeps it
    /// from being a way around the throttle: an attacker guessing a password
    /// never gets far enough to clear the rows their guesses left.
    ///
    /// Scoped to that address on purpose: nobody's routine sign in should be a
    /// delete across the whole table. Returns how many rows went.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::Unavailable`] if the delete fails.
    pub async fn clear_login_attempts(&self, email: &str) -> DomainResult<u64> {
        let lowered = email.trim().to_lowercase();

        let affected = sqlx::query!("DELETE FROM login_attempts WHERE email = $1", lowered)
            .execute(&self.pool)
            .await?
            .rows_affected();

        Ok(affected)
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
