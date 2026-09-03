//! Creating, sliding, revoking, and sweeping sessions.
//!
//! Every function here takes a [`ScopedTx`], which means `app.restaurant_id` is
//! already set and the row level security policy on `sessions` applies. That is
//! load bearing rather than tidy: `sessions` is under `FORCE ROW LEVEL
//! SECURITY`, so a statement run against it without the scope set matches zero
//! rows and raises nothing at all. A slide that silently touched nothing would
//! look exactly like a slide that worked.
//!
//! Which is why the writes here count their rows and say so. Postgres will not.
//!
//! Every timestamp is Postgres's `now()`, never a Rust clock. Two containers
//! whose clocks disagree would otherwise honour different expiries for the same
//! session. The lifetimes themselves are Rust constants passed in as seconds, so
//! `domain::session` stays the one place they are written down.

use chrono::{DateTime, Utc};

use crate::domain::error::{DomainError, DomainResult};
use crate::domain::ids::{SessionId, StaffId};
use crate::domain::session::{DEAD_SESSION_RETENTION, SESSION_ABSOLUTE_LIFETIME, SESSION_LIFETIME};

use super::super::ScopedTx;

/// A session as it was just written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssuedSession {
    /// Which session this is.
    pub id: SessionId,
    /// When it stops being valid unless it is used again.
    pub expires_at: DateTime<Utc>,
    /// The ceiling it can never slide past.
    pub absolute_expires_at: DateTime<Utc>,
}

/// Opens a session for somebody who has just proved who they are.
///
/// The caller has already minted the token and hashed it; this only writes the
/// row. Both expiries are computed by Postgres from its own clock.
///
/// # Errors
///
/// Returns [`DomainError::Unavailable`] if the insert fails, and
/// [`DomainError::Conflict`] in the astronomically unlikely event that two
/// tokens hash to the same value, which is worth naming rather than reporting
/// as an outage.
pub async fn open(
    tx: &mut ScopedTx<'_>,
    staff_id: StaffId,
    token_hash: &[u8],
) -> DomainResult<IssuedSession> {
    let id = SessionId::new();

    let row = sqlx::query!(
        r#"
        INSERT INTO sessions
            (id, restaurant_id, staff_id, token_hash, expires_at, last_seen_at,
             absolute_expires_at, updated_at)
        VALUES ($1, $2, $3, $4,
                now() + make_interval(secs => $5),
                now(),
                now() + make_interval(secs => $6),
                now())
        RETURNING expires_at, absolute_expires_at
        "#,
        id.as_uuid(),
        tx.restaurant_id().as_uuid(),
        staff_id.as_uuid(),
        token_hash,
        seconds(SESSION_LIFETIME),
        seconds(SESSION_ABSOLUTE_LIFETIME),
    )
    .fetch_one(tx.connection())
    .await
    .map_err(|error| {
        super::conflict_on(
            error,
            "sessions_token_hash_key",
            "that session token is already in use",
        )
    })?;

    Ok(IssuedSession {
        id,
        expires_at: row.expires_at,
        absolute_expires_at: row.absolute_expires_at,
    })
}

/// Moves a session's expiry forward.
///
/// Called only when the caller has already decided the session is stale enough
/// to be worth writing, so this statement carries no time condition of its own.
/// That is what lets it assert what it touched: with no `WHERE` clause that can
/// legitimately match nothing, zero rows means `app.restaurant_id` was not set
/// the way the caller thought, and Postgres will not say a word about it. Row
/// level security filters an `UPDATE` into matching nothing rather than into an
/// error, so this assertion is the only thing between a mis scoped transaction
/// and a session that silently never slides.
///
/// It is loud and it does not fail the request. A person whose session did not
/// slide is not having a bad time yet; the engineer reading the logs is the one
/// who needs to know.
///
/// `absolute_expires_at` is deliberately absent from the `SET` list. It is
/// written once at sign in and never moves, and leaving it out of the statement
/// is what makes that true rather than remembered.
///
/// # Errors
///
/// Returns [`DomainError::Unavailable`] if the update fails.
pub async fn slide(tx: &mut ScopedTx<'_>, session_id: SessionId) -> DomainResult<()> {
    let affected = sqlx::query!(
        r#"
        UPDATE sessions
           SET expires_at   = now() + make_interval(secs => $2),
               last_seen_at = now(),
               updated_at   = now()
         WHERE id = $1
        "#,
        session_id.as_uuid(),
        seconds(SESSION_LIFETIME),
    )
    .execute(tx.connection())
    .await?
    .rows_affected();

    if affected != 1 {
        tracing::error!(
            session_id = %session_id,
            restaurant_id = %tx.restaurant_id(),
            affected,
            "sliding a session touched the wrong number of rows; the transaction's restaurant \
             scope is probably not the session's own, and no session on this path is sliding"
        );
    }

    Ok(())
}

/// Ends exactly the session that made the request.
///
/// Every other session belonging to the same person keeps working, which is
/// what signing out on one device rather than everywhere means.
///
/// # Errors
///
/// Returns [`DomainError::Unavailable`] if the update fails, and
/// [`DomainError::NotFound`] if no live session with that id is visible to this
/// transaction, which means it was already revoked or the scope is wrong.
pub async fn revoke(tx: &mut ScopedTx<'_>, session_id: SessionId) -> DomainResult<()> {
    let affected = sqlx::query!(
        r#"
        UPDATE sessions
           SET revoked_at = now(),
               updated_at = now()
         WHERE id = $1
           AND revoked_at IS NULL
        "#,
        session_id.as_uuid(),
    )
    .execute(tx.connection())
    .await?
    .rows_affected();

    if affected == 0 {
        return Err(DomainError::NotFound);
    }

    Ok(())
}

/// Ends every session this person holds except the one named.
///
/// Used by a password change: the person who just proved they know the new
/// password stays signed in where they are, and every other device is turned
/// out. Returns how many were ended, for the audit row and the log.
///
/// # Errors
///
/// Returns [`DomainError::Unavailable`] if the update fails.
pub async fn revoke_every_other(
    tx: &mut ScopedTx<'_>,
    staff_id: StaffId,
    keep: SessionId,
) -> DomainResult<u64> {
    let affected = sqlx::query!(
        r#"
        UPDATE sessions
           SET revoked_at = now(),
               updated_at = now()
         WHERE staff_id = $1
           AND id <> $2
           AND revoked_at IS NULL
        "#,
        staff_id.as_uuid(),
        keep.as_uuid(),
    )
    .execute(tx.connection())
    .await?
    .rows_affected();

    Ok(affected)
}

/// Clears out this person's own long dead session rows.
///
/// Run on the sign in path, and scoped to the person signing in on purpose:
/// nobody's routine sign in should be a write across every row in the table.
/// Returns how many rows went.
///
/// # Errors
///
/// Returns [`DomainError::Unavailable`] if the delete fails.
pub async fn sweep_dead(tx: &mut ScopedTx<'_>, staff_id: StaffId) -> DomainResult<u64> {
    let affected = sqlx::query!(
        r#"
        DELETE FROM sessions
         WHERE staff_id = $1
           AND (
                 (revoked_at IS NOT NULL AND revoked_at < now() - make_interval(secs => $2))
              OR (expires_at < now() - make_interval(secs => $2))
               )
        "#,
        staff_id.as_uuid(),
        seconds(DEAD_SESSION_RETENTION),
    )
    .execute(tx.connection())
    .await?
    .rows_affected();

    Ok(affected)
}

/// Records that somebody signed in, for feature 10's staff list.
///
/// # Errors
///
/// Returns [`DomainError::Unavailable`] if the update fails.
pub async fn mark_signed_in(tx: &mut ScopedTx<'_>, staff_id: StaffId) -> DomainResult<()> {
    sqlx::query!(
        r#"
        UPDATE staff
           SET last_sign_in_at = now(),
               updated_at      = now()
         WHERE id = $1
        "#,
        staff_id.as_uuid(),
    )
    .execute(tx.connection())
    .await?;

    Ok(())
}

/// A [`Duration`](std::time::Duration) as the seconds Postgres wants.
///
/// `make_interval(secs => ...)` takes a double, which is what keeps the number
/// itself in Rust where a test can pin it rather than written as an interval
/// literal inside a string nothing checks.
fn seconds(duration: std::time::Duration) -> f64 {
    duration.as_secs_f64()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The lifetimes reach SQL as seconds, and a rounding surprise at this
    /// scale would be a session that lives a day too long or too little.
    #[test]
    fn every_lifetime_survives_the_trip_into_seconds() {
        assert!((seconds(SESSION_LIFETIME) - 1_209_600.0).abs() < f64::EPSILON);
        assert!((seconds(SESSION_ABSOLUTE_LIFETIME) - 7_776_000.0).abs() < f64::EPSILON);
        assert!((seconds(DEAD_SESSION_RETENTION) - 604_800.0).abs() < f64::EPSILON);
    }
}
