//! The session token, and the only two forms it ever takes.
//!
//! A session is 32 bytes of cryptographic randomness. The browser gets those
//! bytes, base64url encoded, in a cookie; the database gets their SHA-256 and
//! nothing else. So a dump of the `sessions` table hands an attacker no session
//! at all, and the raw value exists in exactly two places: the `Set-Cookie`
//! header on the way out, and the browser's own cookie jar.
//!
//! SHA-256 rather than a password hash, and that is deliberate rather than an
//! oversight. A password hash is slow on purpose because a password is guessable;
//! this value is 256 bits from the operating system's random source, so there is
//! nothing to guess, and the column is looked up on every single request.

use std::time::Duration;

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rand::TryRngCore as _;
use rand::rngs::OsRng;
use sha2::{Digest as _, Sha256};

use super::error::{DomainError, DomainResult};

// The four lifetimes below are written as seconds multiplied out, and clippy is
// overruled on each of them on purpose. `Duration::from_days` and `from_mins`
// are still unstable, and the stable suggestion, `from_hours(336)`, is a number
// nobody can read as a fortnight. Written as `14 * 24 * 60 * 60` the unit is on
// the page, which is what these constants are for.

/// How long a session lives from its last use.
///
/// Long on purpose. A waiter picks up the house phone at the start of a shift
/// and must not be typing an email address on a phone keyboard in the middle of
/// service, so the window has to cover a run of days off.
#[allow(clippy::duration_suboptimal_units)]
pub const SESSION_LIFETIME: Duration = Duration::from_secs(14 * 24 * 60 * 60);

/// How long a session may live from sign in, however often it is used.
///
/// Written once at sign in and never moved. This is the ceiling that means a
/// borrowed phone eventually stops working even though somebody has been using
/// it every day.
#[allow(clippy::duration_suboptimal_units)]
pub const SESSION_ABSOLUTE_LIFETIME: Duration = Duration::from_secs(90 * 24 * 60 * 60);

/// How stale a session's last use has to be before a request slides it forward.
///
/// The whole point of a floor. Without one, every request would write to
/// `sessions`, which turns a read path into a write path on a busy evening for
/// no benefit: moving an expiry that is thirteen days away by four more minutes
/// changes nothing anybody can observe.
#[allow(clippy::duration_suboptimal_units)]
pub const SLIDE_AFTER: Duration = Duration::from_secs(5 * 60);

/// How long a dead session's row is kept before the next sign in clears it.
///
/// Not zero, because a revoked session that vanishes instantly leaves nothing
/// to look at when somebody asks why they were signed out.
#[allow(clippy::duration_suboptimal_units)]
pub const DEAD_SESSION_RETENTION: Duration = Duration::from_secs(7 * 24 * 60 * 60);

/// How many random bytes a session token carries.
///
/// 32 bytes is 256 bits. Well past anything brute forceable, and it is also the
/// output width of the hash it is stored under, so neither end is the narrow
/// one.
pub const SESSION_TOKEN_BYTES: usize = 32;

/// Whether a session is stale enough that using it should move its expiry.
///
/// Worked out from `expires_at` rather than from `last_seen_at`, because
/// `expires_at` is exactly `last_seen_at` plus [`SESSION_LIFETIME`]: it is set
/// that way when the session opens and again on every slide, and nothing else
/// writes it. So the last use is `expires_at - SESSION_LIFETIME`, and the
/// session lookup already returns `expires_at`. That is the whole reason the
/// lookup did not have to grow a column.
///
/// The clock here is this container's, and that is deliberate and safe. It
/// decides only *whether* to write; the write itself uses Postgres's `now()`,
/// so a container running a minute fast slides a session a minute early rather
/// than writing a timestamp another container will disagree with. Nothing about
/// whether a session is valid is decided here: expiry is checked in
/// `resolve_session`, in SQL, against the database's own clock.
#[must_use]
pub fn is_slide_due(
    expires_at: chrono::DateTime<chrono::Utc>,
    now: chrono::DateTime<chrono::Utc>,
) -> bool {
    let Ok(lifetime) = chrono::Duration::from_std(SESSION_LIFETIME) else {
        return false;
    };
    let Ok(floor) = chrono::Duration::from_std(SLIDE_AFTER) else {
        return false;
    };

    let last_seen_at = expires_at - lifetime;

    now - last_seen_at >= floor
}

/// A freshly minted session token, in the only form the browser ever sees.
///
/// Deliberately not [`Clone`] and deliberately not [`Debug`]: it exists for
/// exactly as long as it takes to write one `Set-Cookie` header, and a copy of
/// it lying around, or one printed into a log line, is the whole failure this
/// type exists to prevent.
pub struct SessionToken {
    /// The raw bytes. Never stored, never logged.
    raw: [u8; SESSION_TOKEN_BYTES],
}

impl SessionToken {
    /// Mints a token from the operating system's cryptographic random source.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::Unavailable`] if the operating system will not
    /// produce randomness, which is a machine that must not go on issuing
    /// sessions.
    pub fn mint() -> DomainResult<Self> {
        let mut raw = [0u8; SESSION_TOKEN_BYTES];

        OsRng.try_fill_bytes(&mut raw).map_err(|error| {
            tracing::error!(error = %error, "the operating system refused to produce randomness");
            DomainError::Unavailable("randomness".to_owned())
        })?;

        Ok(Self { raw })
    }

    /// The cookie value: the raw bytes, base64url encoded without padding.
    ///
    /// URL safe and unpadded, so the value needs no further escaping in a
    /// `Set-Cookie` header and comes back byte for byte.
    #[must_use]
    pub fn cookie_value(&self) -> String {
        URL_SAFE_NO_PAD.encode(self.raw)
    }

    /// What goes in `sessions.token_hash`.
    #[must_use]
    pub fn hash(&self) -> Vec<u8> {
        hash_bytes(&self.raw)
    }
}

/// Turns a cookie value received from a browser back into the stored hash.
///
/// Returns [`None`] for anything that is not a token this server issued, which
/// includes a value that is not base64url and a value of the wrong length. That
/// is a lookup that will not match rather than an error worth reporting: an old
/// cookie, a truncated one, and a hand edited one are all just "not signed in".
#[must_use]
pub fn hash_cookie_value(value: &str) -> Option<Vec<u8>> {
    let raw = URL_SAFE_NO_PAD.decode(value.trim()).ok()?;

    if raw.len() != SESSION_TOKEN_BYTES {
        return None;
    }

    Some(hash_bytes(&raw))
}

/// The one place the hash is computed, so both directions cannot disagree.
fn hash_bytes(raw: &[u8]) -> Vec<u8> {
    Sha256::digest(raw).to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// covers: AC-3
    #[test]
    fn a_token_is_thirty_two_bytes_and_no_two_are_alike() {
        let first = SessionToken::mint().expect("the operating system produced randomness");
        let second = SessionToken::mint().expect("the operating system produced randomness");

        assert_eq!(first.raw.len(), SESSION_TOKEN_BYTES);
        assert_ne!(
            first.raw, second.raw,
            "two freshly minted tokens are identical, so the source is not random"
        );
    }

    /// covers: AC-3
    ///
    /// The round trip the whole scheme rests on. The browser hands back the
    /// cookie value, and the value the server looks up has to be the same hash
    /// it stored at sign in.
    #[test]
    fn the_cookie_value_hashes_back_to_what_was_stored() {
        let token = SessionToken::mint().expect("minting");

        assert_eq!(
            hash_cookie_value(&token.cookie_value()),
            Some(token.hash()),
            "the value handed to the browser does not resolve to the stored hash"
        );
    }

    /// covers: AC-3
    ///
    /// The stored value is a hash, not the token. A column holding the token
    /// itself would turn a read only database leak into every open session.
    #[test]
    fn the_stored_hash_is_not_the_token() {
        let token = SessionToken::mint().expect("minting");

        assert_eq!(token.hash().len(), 32, "sha-256 is 32 bytes");
        assert_ne!(
            token.hash(),
            token.raw.to_vec(),
            "the stored value is the token itself"
        );
        assert!(
            !token.cookie_value().contains(char::is_whitespace),
            "the cookie value must need no escaping"
        );
    }

    #[test]
    fn a_cookie_value_that_was_never_issued_resolves_to_nothing() {
        assert_eq!(hash_cookie_value(""), None, "an empty cookie");
        assert_eq!(hash_cookie_value("not base64!!"), None, "not base64url");
        assert_eq!(
            hash_cookie_value(&URL_SAFE_NO_PAD.encode([0u8; 16])),
            None,
            "the right encoding, the wrong length"
        );
        assert_eq!(
            hash_cookie_value(&URL_SAFE_NO_PAD.encode([0u8; 64])),
            None,
            "too long is refused as well as too short"
        );
    }

    /// covers: AC-7
    ///
    /// These four numbers only mean anything in relation to each other, and
    /// each relationship here is one somebody could break with a plausible
    /// looking edit.
    #[test]
    fn the_session_lifetimes_stay_in_the_order_that_makes_them_meaningful() {
        assert!(
            SESSION_ABSOLUTE_LIFETIME > SESSION_LIFETIME,
            "the ceiling is not above the sliding window, so a session would be dead on arrival              and the check constraint on the column would refuse every insert"
        );
        assert!(
            SLIDE_AFTER < SESSION_LIFETIME,
            "a session that only slides after it has already expired never slides at all"
        );
        // The floor is what keeps the read path a read path. A value anywhere
        // near the lifetime would be indistinguishable from having no floor.
        assert!(
            SLIDE_AFTER * 100 < SESSION_LIFETIME,
            "the slide floor is close enough to the lifetime to be doing nothing"
        );
        assert!(
            DEAD_SESSION_RETENTION < SESSION_LIFETIME,
            "dead rows are kept longer than live ones live, so the sweep never reaches them"
        );
    }

    /// covers: AC-7
    ///
    /// The floor is what keeps a read path a read path. A screen making a
    /// request every second must write once every five minutes, not once a
    /// second.
    #[test]
    fn a_session_used_again_straight_away_is_not_due_a_slide() {
        let now = chrono::Utc::now();
        let lifetime = chrono::Duration::from_std(SESSION_LIFETIME).expect("the lifetime converts");
        let floor = chrono::Duration::from_std(SLIDE_AFTER).expect("the floor converts");

        // Just opened: last seen now, so expiry is a full lifetime away.
        let fresh = now + lifetime;
        assert!(
            !is_slide_due(fresh, now),
            "a session used the instant it opened was written to anyway"
        );

        // Used four minutes ago, which is inside the floor.
        let recent = now + lifetime - chrono::Duration::minutes(4);
        assert!(
            !is_slide_due(recent, now),
            "four minutes is inside the floor"
        );

        // Used six minutes ago, which is past it.
        let stale = now + lifetime - chrono::Duration::minutes(6);
        assert!(is_slide_due(stale, now), "six minutes is past the floor");

        // And exactly on the floor counts, so the boundary is not a gap.
        assert!(
            is_slide_due(now + lifetime - floor, now),
            "a session exactly at the floor should slide"
        );
    }

    /// covers: AC-7, AC-18
    ///
    /// The case the live stream depends on. A screen left open all shift makes
    /// no request but its own heartbeat, and each heartbeat has to find the
    /// session due and move it, or the screen expires under the person watching
    /// it.
    #[test]
    fn a_session_older_than_the_floor_is_always_due() {
        let now = chrono::Utc::now();
        let lifetime = chrono::Duration::from_std(SESSION_LIFETIME).expect("the lifetime converts");

        for hours_since_use in [1, 24, 24 * 13] {
            let expires_at = now + lifetime - chrono::Duration::hours(hours_since_use);
            assert!(
                is_slide_due(expires_at, now),
                "a session last used {hours_since_use} hours ago was not due a slide"
            );
        }
    }

    /// A token trimmed of surrounding whitespace still resolves, because some
    /// intermediaries pad cookie values and a person signed out by a space is a
    /// bug nobody would find.
    #[test]
    fn a_padded_cookie_value_still_resolves() {
        let token = SessionToken::mint().expect("minting");
        let padded = format!(" {} ", token.cookie_value());

        assert_eq!(hash_cookie_value(&padded), Some(token.hash()));
    }
}
