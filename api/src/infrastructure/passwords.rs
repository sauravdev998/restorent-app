//! Hashing and verifying a password with `argon2id`.
//!
//! Two things here are load bearing and both are easy to get subtly wrong.
//!
//! **The work runs off the async runtime.** An `argon2` hash is deliberately
//! expensive: it costs tens of milliseconds and 19 megabytes by design. Run on
//! a Tokio worker thread it blocks every other task that thread was carrying,
//! so a handful of simultaneous sign ins would stall unrelated requests, live
//! streams included. [`tokio::task::spawn_blocking`] puts it on the pool meant
//! for exactly this.
//!
//! **An unknown address still costs a hash.** [`Argon2Passwords::verify_nothing`]
//! verifies a fixed dummy hash that matches nothing, so sign in takes comparable
//! time whether or not the address exists. Making the status code and the body
//! identical is the easy half of not leaking which addresses have accounts; the
//! clock is the half that gets forgotten.

use std::sync::LazyLock;

use argon2::Argon2;
use argon2::password_hash::{PasswordHash, PasswordHasher as _, PasswordVerifier as _, SaltString};
use rand::TryRngCore as _;
use rand::rngs::OsRng;

use crate::application::ports::PasswordHasher;
use crate::domain::credentials::Password;
use crate::domain::error::{DomainError, DomainResult};

/// A password nobody has, hashed once, for [`Argon2Passwords::verify_nothing`].
///
/// Computed lazily on first use rather than written out as a literal, so it
/// always matches the parameters this build actually uses. A hardcoded hash
/// from an older parameter set would make the dummy verification cheaper than a
/// real one, and the timing would answer the question again.
static DUMMY_HASH: LazyLock<Option<String>> = LazyLock::new(|| {
    hash_blocking("a password that belongs to nobody at all")
        .inspect_err(|error| {
            tracing::error!(error = %error, "could not build the dummy hash; sign in timing will leak");
        })
        .ok()
});

/// `argon2id` at the crate's default parameters.
#[derive(Debug, Clone, Copy, Default)]
pub struct Argon2Passwords;

impl Argon2Passwords {
    /// Builds the hasher. It holds no state; the type exists so the port has
    /// something to be implemented on.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl PasswordHasher for Argon2Passwords {
    async fn hash(&self, password: Password) -> DomainResult<String> {
        run_off_the_runtime(move || hash_blocking(password.exposed())).await
    }

    async fn verify(&self, password: Password, hash: String) -> DomainResult<bool> {
        run_off_the_runtime(move || Ok(verify_blocking(password.exposed(), &hash))).await
    }

    async fn verify_nothing(&self) -> DomainResult<()> {
        run_off_the_runtime(|| {
            // No dummy hash means the parameters themselves are broken, which
            // is already logged. Spending nothing here is the only option left,
            // and it is better than refusing the sign in outright.
            if let Some(dummy) = DUMMY_HASH.as_ref() {
                let _ = verify_blocking("a password that belongs to nobody at all ", dummy);
            }
            Ok(())
        })
        .await
    }
}

/// Runs one hashing job on the blocking pool.
///
/// A join failure means the runtime is shutting down or the task panicked;
/// either way the caller gets an unavailable dependency rather than a panic
/// escaping into a handler.
async fn run_off_the_runtime<T, F>(work: F) -> DomainResult<T>
where
    T: Send + 'static,
    F: FnOnce() -> DomainResult<T> + Send + 'static,
{
    tokio::task::spawn_blocking(work).await.map_err(|error| {
        tracing::error!(error = %error, "the password hashing task did not finish");
        DomainError::Unavailable("password hashing".to_owned())
    })?
}

/// How many random bytes a salt carries.
///
/// The `argon2` crate's own recommended length, and the length its
/// `SaltString::generate` would have used. The salt is generated here rather
/// than by that helper so the whole process draws its randomness from one
/// source, the `rand` crate's `OsRng`, instead of pulling a second copy of an
/// older `rand_core` in behind the hashing crate.
const SALT_BYTES: usize = 16;

/// The hash itself, synchronous, always called from the blocking pool.
fn hash_blocking(password: &str) -> DomainResult<String> {
    let mut salt_bytes = [0u8; SALT_BYTES];
    OsRng.try_fill_bytes(&mut salt_bytes).map_err(|error| {
        tracing::error!(error = %error, "the operating system refused to produce a salt");
        DomainError::Unavailable("randomness".to_owned())
    })?;

    let salt = SaltString::encode_b64(&salt_bytes).map_err(|error| {
        tracing::error!(error = %error, "could not encode a password salt");
        DomainError::Unavailable("password hashing".to_owned())
    })?;

    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|error| {
            // The message can quote the input's length and the parameters, so
            // it stays in the logs.
            tracing::error!(error = %error, "could not hash a password");
            DomainError::Unavailable("password hashing".to_owned())
        })
}

/// The verification itself, synchronous, always called from the blocking pool.
///
/// A stored value that will not parse is reported as "does not match" rather
/// than as an error, and logged. A row with a corrupt hash must not be
/// distinguishable from a wrong password by anybody outside this process.
fn verify_blocking(password: &str, stored: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(stored) else {
        tracing::error!("a stored password hash could not be parsed; treating it as no match");
        return false;
    };

    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn password(raw: &str) -> Password {
        Password::new(raw).expect("the test password satisfies the rules")
    }

    /// covers: AC-5
    #[tokio::test]
    async fn a_password_verifies_against_its_own_hash_and_nothing_else() {
        let hasher = Argon2Passwords::new();
        let stored = hasher
            .hash(password("correct horse battery"))
            .await
            .expect("hashing");

        assert!(
            hasher
                .verify(password("correct horse battery"), stored.clone())
                .await
                .expect("verifying"),
            "the password did not verify against its own hash"
        );
        assert!(
            !hasher
                .verify(password("incorrect horse battery"), stored)
                .await
                .expect("verifying"),
            "a different password verified"
        );
    }

    /// covers: AC-5
    ///
    /// The stored value has to be an `argon2id` hash, not the password and not
    /// some other algorithm that happened to be the default. The prefix is the
    /// only part of the format that says which.
    #[tokio::test]
    async fn the_stored_value_is_an_argon2id_hash_and_never_the_password() {
        let stored = Argon2Passwords::new()
            .hash(password("correct horse battery"))
            .await
            .expect("hashing");

        assert!(
            stored.starts_with("$argon2id$"),
            "the stored value is {stored:?}, which is not an argon2id hash"
        );
        assert!(
            !stored.contains("correct horse"),
            "the password itself appeared in the stored value"
        );
    }

    /// The salt is what stops one leaked hash answering for every account that
    /// picked the same password.
    #[tokio::test]
    async fn the_same_password_hashes_differently_every_time() {
        let hasher = Argon2Passwords::new();

        let first = hasher.hash(password("correct horse")).await.expect("first");
        let second = hasher
            .hash(password("correct horse"))
            .await
            .expect("second");

        assert_ne!(first, second, "two hashes of one password are identical");
        assert!(
            hasher
                .verify(password("correct horse"), second)
                .await
                .expect("verifying"),
            "the second hash does not verify, so the salt is not being stored with it"
        );
    }

    /// covers: AC-5
    ///
    /// The trimming rule reaches all the way through. A password set with a
    /// trailing newline and typed without one is the same password, because the
    /// newtype trimmed both before either reached a hash.
    #[tokio::test]
    async fn a_padded_password_verifies_against_the_hash_of_its_trimmed_self() {
        let hasher = Argon2Passwords::new();
        let stored = hasher
            .hash(password("correct horse battery\n"))
            .await
            .expect("hashing");

        assert!(
            hasher
                .verify(password("  correct horse battery  "), stored)
                .await
                .expect("verifying"),
            "trimming disagrees between hashing and verifying"
        );
    }

    /// covers: AC-4
    ///
    /// A row whose hash is unreadable must look exactly like a wrong password
    /// from outside. Reporting it as an error would let a caller find the
    /// accounts with broken hashes.
    #[tokio::test]
    async fn a_stored_value_that_is_not_a_hash_is_a_mismatch_rather_than_an_error() {
        let matched = Argon2Passwords::new()
            .verify(password("correct horse"), "not-a-real-hash".to_owned())
            .await
            .expect("verification ran");

        assert!(!matched, "a corrupt stored hash matched");
    }

    /// covers: AC-4
    ///
    /// The dummy verification has to do real work. If it returned immediately,
    /// an unknown address would answer measurably faster than a known one and
    /// the identical body would prove nothing.
    #[tokio::test]
    async fn verifying_nothing_still_costs_a_hash() {
        assert!(
            DUMMY_HASH.is_some(),
            "there is no dummy hash, so an unknown address costs nothing to refuse"
        );

        let started = std::time::Instant::now();
        Argon2Passwords::new()
            .verify_nothing()
            .await
            .expect("the dummy verification ran");
        let spent = started.elapsed();

        // A real argon2 verification is tens of milliseconds. The bound is
        // deliberately far below that: what is being caught is a body that
        // returns without hashing at all, not a slow machine.
        assert!(
            spent >= std::time::Duration::from_millis(1),
            "verifying nothing took {spent:?}, which is not the cost of a hash"
        );
    }
}
