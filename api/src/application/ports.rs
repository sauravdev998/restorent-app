//! Interfaces the application needs, which the infrastructure layer implements.
//!
//! This is where the dependency rule is bought: the use case below depends on
//! this trait, and the database and listener code depends on it too, so neither
//! the use case nor the domain has to know a `PgPool` exists.

use std::future::Future;

use crate::domain::credentials::Password;
use crate::domain::error::DomainResult;

/// Whatever can turn a password into a stored hash, and check one against it.
///
/// A trait rather than a direct call, for the usual reason: the use case knows
/// that a password is hashed and verified, and knows nothing about which
/// algorithm does it or that the work runs on a blocking thread.
///
/// Both methods are async and both are expensive on purpose. A password hash
/// that is cheap is a password hash that is worth attacking offline, so the
/// implementation moves the work off the async runtime rather than making it
/// faster.
pub trait PasswordHasher: Send + Sync {
    /// Turns a password into the value stored in `staff.password_hash`.
    ///
    /// # Errors
    ///
    /// Returns an error only if the hash itself could not be computed, which is
    /// a fault of this machine rather than of the password.
    fn hash(&self, password: Password) -> impl Future<Output = DomainResult<String>> + Send;

    /// Checks a password against a stored hash.
    ///
    /// Returns `Ok(false)` for a password that does not match and for a stored
    /// value that is not a hash this implementation understands. Neither is an
    /// error: both mean "not this person", and telling them apart on the way
    /// out would tell a caller which accounts have a broken hash.
    ///
    /// # Errors
    ///
    /// Returns an error only if the verification could not be run at all.
    fn verify(
        &self,
        password: Password,
        hash: String,
    ) -> impl Future<Output = DomainResult<bool>> + Send;

    /// Spends the same time verifying nothing at all.
    ///
    /// Sign in calls this when the address matches no account, so an unknown
    /// address costs one password hash exactly as a known one does. Without it
    /// the response time answers the question "does this address have an
    /// account here", however carefully the body and the status code are made
    /// identical.
    ///
    /// # Errors
    ///
    /// Returns an error only if the verification could not be run at all.
    fn verify_nothing(&self) -> impl Future<Output = DomainResult<()>> + Send;
}

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
