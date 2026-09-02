//! Staff and their sessions, plus the two narrow shapes sign in reads.

use chrono::{DateTime, Utc};

use super::enums::StaffRole;
use super::ids::{RestaurantId, SessionId, StaffId};
use super::language::LanguageCode;

/// Somebody who works at a restaurant.
///
/// Email identifies exactly one account across the whole platform, not one per
/// restaurant. That is a real limitation, accepted so that signing in is an
/// email and a password with nothing else to type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Staff {
    /// Which staff member this is.
    pub id: StaffId,
    /// How they sign in.
    pub email: String,
    /// What to call them on screen.
    pub display_name: String,
    /// What they are allowed to be. Which role may do what is feature 7's
    /// decision; this layer carries the role and nothing more.
    pub role: StaffRole,
    /// Which language they personally read the interface in. [`None`] means
    /// they take the restaurant's default, which is what a newly created
    /// account has and what most accounts keep.
    ///
    /// Never consulted on the kitchen surface. That screen is a shared
    /// appliance which several people read across a shift handover, so it
    /// follows the restaurant and not whoever last signed in.
    pub language: Option<LanguageCode>,
    /// When their account was switched off, if it was. A deactivated account
    /// keeps its row so historical bills stay attributable.
    pub deactivated_at: Option<DateTime<Utc>>,
}

/// One signed in session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    /// Which session this is.
    pub id: SessionId,
    /// Whose it is.
    pub staff_id: StaffId,
    /// When it stops being valid.
    pub expires_at: DateTime<Utc>,
    /// When it was last used, for showing somebody their own sessions.
    pub last_seen_at: DateTime<Utc>,
    /// When it was signed out, if it was.
    pub revoked_at: Option<DateTime<Utc>>,
}

/// What the sign in lookup is allowed to learn about an account.
///
/// One of exactly two shapes in the system that can be read without a restaurant
/// scope, so it is deliberately narrow: enough to check a password and work out
/// which restaurant to scope to, and nothing else. Not the display name, not the
/// account's history, nothing a caller could mine by guessing addresses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaffCredentials {
    /// Who they are.
    pub staff_id: StaffId,
    /// Which restaurant to scope the rest of the request to.
    pub restaurant_id: RestaurantId,
    /// What they are allowed to be.
    pub role: StaffRole,
    /// The stored hash, for the caller to verify a password against.
    pub password_hash: String,
    /// Whether the account has been switched off. Reported, not judged: this
    /// lookup answers who the account belongs to, and feature 7 decides what a
    /// deactivated one may do.
    pub deactivated_at: Option<DateTime<Utc>>,
}

/// What resolving a session token yields.
///
/// The other of exactly two shapes readable without a scope. Its `restaurant_id`
/// is what every later query in the request is scoped to, which makes this the
/// value the whole tenant isolation story hangs from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedSession {
    /// Which session answered.
    pub session_id: SessionId,
    /// Whose it is.
    pub staff_id: StaffId,
    /// Which restaurant the rest of this request may see.
    pub restaurant_id: RestaurantId,
    /// What they are allowed to be.
    pub role: StaffRole,
    /// When the session stops being valid.
    pub expires_at: DateTime<Utc>,
}
