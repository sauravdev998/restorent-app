//! Who is asking, what they may do, and which restaurant this request may see.
//!
//! This replaces the development placeholder that read a header. From here on
//! there is exactly one answer to all three questions and it comes from a
//! session row: no environment behaves differently, and no code path in any
//! environment can name a restaurant that did not come from a resolved session.
//!
//! # The four steps, in this order
//!
//! The order is load bearing, and getting it wrong fails silently rather than
//! loudly, so it is written out.
//!
//! 1. **Resolve, unscoped.** `resolve_session` is `SECURITY DEFINER` and owned
//!    by `auth_lookup`, so it needs no restaurant set. No row means `401` and
//!    nothing further happens.
//! 2. **Check the role**, against the marker type in the handler's own
//!    signature, and refuse with `403` before the handler body is entered. No
//!    database work is done for a refusal.
//! 3. **Slide, in its own scoped transaction, only when due.** It commits on its
//!    own, separately from whatever the handler goes on to do: a handler that
//!    fails afterwards must not un slide a session that was genuinely used.
//! 4. **Run the handler**, which opens its own scoped transaction as every
//!    handler already does.
//!
//! So a request costs one unscoped read always, one short scoped write at most
//! once every five minutes, and the handler's own transaction.

use std::marker::PhantomData;

use axum::extract::FromRequestParts;
use axum::http::HeaderMap;
use axum::http::request::Parts;
use axum_extra::extract::cookie::CookieJar;

use crate::domain::enums::StaffRole;
use crate::domain::error::DomainError;
use crate::domain::ids::{RestaurantId, SessionId, StaffId};
use crate::domain::people::ResolvedSession;
use crate::domain::session::{hash_cookie_value, is_slide_due};
use crate::infrastructure::db::repository::sessions;
use crate::presentation::cookie::SESSION_COOKIE;
use crate::presentation::error::ApiError;
use crate::presentation::state::AppState;

/// What role a handler demands, expressed as a type.
///
/// The whole point of a type rather than a line inside the handler: a handler
/// that forgets the check does not compile with a role requirement in its
/// signature, which turns a forgotten `if` into a visible absence. The same
/// trick already protects tenant scoping.
pub trait RoleRequirement: Send + Sync + 'static {
    /// What the `OpenAPI` document says about who may call this.
    ///
    /// It travels into the generated TypeScript client's documentation, so the
    /// requirement is visible to whoever is building the screen as well as to
    /// whoever is reading the Rust.
    const DESCRIPTION: &'static str;

    /// Whether this role satisfies the requirement.
    fn permits(role: StaffRole) -> bool;
}

/// Any signed in person, whatever they do here.
#[derive(Debug, Clone, Copy)]
pub struct AnyRole;

impl RoleRequirement for AnyRole {
    const DESCRIPTION: &'static str = "Any signed in role.";

    fn permits(_role: StaffRole) -> bool {
        true
    }
}

/// An admin, and nobody else.
#[derive(Debug, Clone, Copy)]
pub struct Admin;

impl RoleRequirement for Admin {
    const DESCRIPTION: &'static str = "Admins only.";

    fn permits(role: StaffRole) -> bool {
        matches!(role, StaffRole::Admin)
    }
}

/// A waiter, and nobody else.
#[derive(Debug, Clone, Copy)]
pub struct Waiter;

impl RoleRequirement for Waiter {
    const DESCRIPTION: &'static str = "Waiters only.";

    fn permits(role: StaffRole) -> bool {
        matches!(role, StaffRole::Waiter)
    }
}

/// A chef, and nobody else.
#[derive(Debug, Clone, Copy)]
pub struct Chef;

impl RoleRequirement for Chef {
    const DESCRIPTION: &'static str = "Chefs only.";

    fn permits(role: StaffRole) -> bool {
        matches!(role, StaffRole::Chef)
    }
}

/// An admin or a chef.
///
/// Exactly one endpoint holds this, the dish availability switch. "We ran out"
/// is the chef's to say, and making them find an admin first would put a gap
/// between the kitchen running out and the waiters knowing, which is the whole
/// thing the switch exists to close.
#[derive(Debug, Clone, Copy)]
pub struct AdminOrChef;

impl RoleRequirement for AdminOrChef {
    const DESCRIPTION: &'static str = "Admins and chefs.";

    fn permits(role: StaffRole) -> bool {
        matches!(role, StaffRole::Admin | StaffRole::Chef)
    }
}

/// A waiter or a chef.
///
/// Held by the ordering menu, which the chef's Menu tab reads too. The admin
/// reads the fuller admin menu instead, which carries the versions, the diet
/// markers of archived dishes, and everything else an edit needs.
#[derive(Debug, Clone, Copy)]
pub struct WaiterOrChef;

impl RoleRequirement for WaiterOrChef {
    const DESCRIPTION: &'static str = "Waiters and chefs.";

    fn permits(role: StaffRole) -> bool {
        matches!(role, StaffRole::Waiter | StaffRole::Chef)
    }
}

/// Who is making this request, once their session has been resolved.
///
/// A handler taking `Actor<Admin>` is guaranteed two things by the time its
/// body runs: somebody is signed in, and they are an admin. A handler taking
/// `Actor` (which is `Actor<AnyRole>`) is guaranteed the first.
///
/// All four of restaurant, staff member, session, and role come out of one
/// lookup, so there is no way to hold a restaurant id without knowing who is
/// acting in it. That is what makes every audit row have a real actor.
#[derive(Debug, Clone)]
pub struct Actor<R: RoleRequirement = AnyRole> {
    session: ResolvedSession,
    // `fn() -> R` rather than `R`, so the marker carries no ownership and the
    // extractor stays `Send` whatever the marker type is.
    requirement: PhantomData<fn() -> R>,
}

impl<R: RoleRequirement> Actor<R> {
    /// Which restaurant this request may see. Goes straight into
    /// `Database::begin_scoped`.
    #[must_use]
    pub const fn restaurant_id(&self) -> RestaurantId {
        self.session.restaurant_id
    }

    /// Who is acting, for an audit row and for a write to their own account.
    #[must_use]
    pub const fn staff_id(&self) -> StaffId {
        self.session.staff_id
    }

    /// Which session made this request, for signing out and for deciding which
    /// sessions a password change keeps.
    #[must_use]
    pub const fn session_id(&self) -> SessionId {
        self.session.session_id
    }

    /// What they are. Rarely needed: the type already carried the requirement.
    #[must_use]
    pub const fn role(&self) -> StaffRole {
        self.session.role
    }
}

impl<R: RoleRequirement> FromRequestParts<AppState> for Actor<R> {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let session = resolve(parts, state).await?;

        // Step 2. Before any database work, and before the handler body.
        if !R::permits(session.role) {
            tracing::info!(
                staff_id = %session.staff_id,
                restaurant_id = %session.restaurant_id,
                role = session.role.as_label(),
                required = R::DESCRIPTION,
                "refusing a request from a role that does not hold this endpoint"
            );
            return Err(DomainError::Forbidden.into());
        }

        // Step 3. Its own transaction, committed on its own.
        slide_if_due(&session, state).await;

        Ok(Self {
            session,
            requirement: PhantomData,
        })
    }
}

/// The stored hash of whatever session cookie this request carried.
///
/// Public because the live stream needs it: it re resolves the session on every
/// heartbeat, and doing that by re reading the cookie is what keeps the one
/// cross tenant lookup the only cross tenant lookup. Looking a session up by its
/// id instead would be a second unscoped path into `sessions`.
///
/// [`None`] covers no cookie, a cookie that is not base64url, and one of the
/// wrong length. All three mean "not signed in".
#[must_use]
pub fn token_hash_from(headers: &HeaderMap) -> Option<Vec<u8>> {
    let jar = CookieJar::from_headers(headers);
    let raw = jar.get(SESSION_COOKIE)?.value().to_owned();

    hash_cookie_value(&raw)
}

/// Resolves a session and slides it if it is due, the way a request would.
///
/// Public for the live stream's heartbeat. A screen left open all shift makes
/// no other request, so without this its session would expire under the person
/// watching it; and a session revoked while the stream is open has to stop
/// resolving within one heartbeat.
pub async fn resolve_and_slide(state: &AppState, token_hash: &[u8]) -> Option<ResolvedSession> {
    let session = state.database.resolve_session(token_hash).await.ok()??;

    slide_if_due(&session, state).await;

    Some(session)
}

/// Step 1: the cookie, hashed, looked up, with no scope anywhere.
///
/// Every failure on this path is the same [`DomainError::Unauthenticated`]: no
/// cookie, a cookie that was never issued, a session that expired, one past its
/// ceiling, and one that was revoked all mean "not signed in", and telling them
/// apart would tell a caller which of their guesses was close.
async fn resolve(parts: &Parts, state: &AppState) -> Result<ResolvedSession, ApiError> {
    let token_hash = token_hash_from(&parts.headers).ok_or(DomainError::Unauthenticated)?;

    state
        .database
        .resolve_session(&token_hash)
        .await?
        .ok_or_else(|| DomainError::Unauthenticated.into())
}

/// Step 3: move the expiry forward, if it is stale enough to be worth writing.
///
/// Nothing here can fail the request. A session that did not slide is not a
/// reason to refuse somebody who is genuinely signed in; it is a reason for a
/// line in the log, which the repository writes.
async fn slide_if_due(session: &ResolvedSession, state: &AppState) {
    if !is_slide_due(session.last_seen_at, chrono::Utc::now()) {
        return;
    }

    let mut tx = match state.database.begin_scoped(session.restaurant_id).await {
        Ok(tx) => tx,
        Err(error) => {
            tracing::warn!(error = %error, "could not open a transaction to slide a session");
            return;
        }
    };

    if let Err(error) = sessions::slide(&mut tx, session.session_id).await {
        tracing::warn!(error = %error, "could not slide a session");
        return;
    }

    if let Err(error) = tx.commit().await {
        tracing::warn!(error = %error, "could not commit a session slide");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// covers: AC-8
    ///
    /// The table this whole mechanism exists to make true. Written out one pair
    /// at a time rather than derived, because a rule that generated it would
    /// change its own answer alongside any mistake in the markers.
    #[test]
    fn each_requirement_admits_exactly_the_roles_it_names() {
        assert!(AnyRole::permits(StaffRole::Admin));
        assert!(AnyRole::permits(StaffRole::Waiter));
        assert!(AnyRole::permits(StaffRole::Chef));

        assert!(Admin::permits(StaffRole::Admin));
        assert!(!Admin::permits(StaffRole::Waiter));
        assert!(!Admin::permits(StaffRole::Chef));

        assert!(!Waiter::permits(StaffRole::Admin));
        assert!(Waiter::permits(StaffRole::Waiter));
        assert!(!Waiter::permits(StaffRole::Chef));

        assert!(!Chef::permits(StaffRole::Admin));
        assert!(!Chef::permits(StaffRole::Waiter));
        assert!(Chef::permits(StaffRole::Chef));
    }

    /// covers: spec 0008 AC-16
    ///
    /// The two combined requirements, pair by pair, for the same reason as the
    /// table above: a rule that generated them would move with any mistake.
    #[test]
    fn each_combined_requirement_admits_exactly_its_two_roles() {
        assert!(AdminOrChef::permits(StaffRole::Admin));
        assert!(
            !AdminOrChef::permits(StaffRole::Waiter),
            "a waiter reached the availability switch, which is the kitchen's call"
        );
        assert!(AdminOrChef::permits(StaffRole::Chef));

        assert!(
            !WaiterOrChef::permits(StaffRole::Admin),
            "an admin reached the ordering menu rather than the admin menu"
        );
        assert!(WaiterOrChef::permits(StaffRole::Waiter));
        assert!(WaiterOrChef::permits(StaffRole::Chef));
    }

    /// covers: AC-8
    ///
    /// An admin is not a superuser here. There is no inheritance between the
    /// three roles, so an admin does not get the waiter's endpoints by being
    /// senior, and that is deliberate: an admin standing at a till is not the
    /// waiter who opened the bill, and the audit row would say the wrong thing.
    #[test]
    fn no_role_inherits_another_ones_endpoints() {
        assert!(
            !Waiter::permits(StaffRole::Admin),
            "an admin reached a waiter only endpoint, so the audit trail would name the \
             wrong person as having taken an order"
        );
        assert!(!Chef::permits(StaffRole::Admin));
    }

    /// Each description reaches the `OpenAPI` document and therefore the
    /// generated client, so an empty or duplicated one would make the document
    /// say nothing about who may call what.
    #[test]
    fn every_requirement_describes_itself_distinctly() {
        let descriptions = [
            AnyRole::DESCRIPTION,
            Admin::DESCRIPTION,
            Waiter::DESCRIPTION,
            Chef::DESCRIPTION,
            AdminOrChef::DESCRIPTION,
            WaiterOrChef::DESCRIPTION,
        ];

        for description in descriptions {
            assert!(!description.trim().is_empty());
        }

        for (index, description) in descriptions.iter().enumerate() {
            let clashes = descriptions
                .iter()
                .filter(|other| *other == description)
                .count();
            assert_eq!(clashes, 1, "requirement {index} shares its description");
        }
    }
}
