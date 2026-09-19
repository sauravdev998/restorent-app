//! Everything an admin does to somebody else's account.
//!
//! Six writes and one read, all scoped, and all of them about a row an admin
//! may not own. That is what makes this module different from
//! [`accounts`](super::accounts), which holds the writes somebody makes to their
//! own row: here the caller and the target are two different people, so every
//! write has to answer "may this admin do this, to this person, right now"
//! before it touches anything.
//!
//! # The two guard rails, and why they need a lock
//!
//! A restaurant must always keep at least one active admin, and no admin may
//! act on their own row. The second is a comparison and needs nothing. The
//! first is a count, and a count read outside a lock is a count two callers can
//! both pass: two admins demoting each other at the same instant each read
//! "two admins, one to spare" and each write, and the restaurant is left with
//! none and no way back in without a database console.
//!
//! So every targeted write takes [`lock_restaurant`] first, a transaction level
//! advisory lock keyed on the restaurant. Two staff writes in one restaurant
//! queue; two in different restaurants never meet. The lock is released by the
//! commit, so there is nothing to unlock and nothing to leak on a failure.
//!
//! It also makes the conditional updates below honest in a second way. Inside
//! the lock, nothing else can move the row between the read that decides the
//! refusals and the update that acts on them, so an update matching zero rows
//! means exactly one thing: the version the form carried is not the version the
//! row has.
//!
//! # One fixed refusal order
//!
//! More than one refusal can apply to the same request, and every targeted
//! action reports them in the same order, decided in [`refuse_before_writing`]
//! before any write:
//!
//! 1. `404`, an id this restaurant does not have
//! 2. `409 cannot_act_on_self`
//! 3. `409 staff_inactive`
//! 4. `409 last_admin`
//! 5. `409 staff_changed`
//!
//! The stale version is last because it is the only one of the five that a
//! reload cures. Telling an admin "somebody changed this, reload" about a
//! person who has been switched off sends them round a loop that ends in the
//! same place.

use serde_json::{Value, json};

use crate::domain::audit::AuditAction;
use crate::domain::credentials::EmailAddress;
use crate::domain::enums::StaffRole;
use crate::domain::error::{ConflictKind, DomainError, DomainResult};
use crate::domain::event::EntityKind;
use crate::domain::ids::StaffId;
use crate::domain::language::LanguageCode;
use crate::domain::people::Staff;

use super::super::{Database, ScopedTx};
use super::{audit, sessions};

/// The name of the platform wide unique index on the lowered address.
const EMAIL_KEY: &str = "staff_email_key";

/// What kind of thing `staff` is, in an audit row's `entity_type`.
const ENTITY: &str = "staff";

/// Separates this feature's advisory locks from every other user of the same
/// space.
///
/// `login_attempts` already takes one keyed on a lowered email address. Both
/// hash into one global namespace, so without a salt of its own a restaurant
/// whose id happened to hash the same as some address would make a sign in
/// attempt wait on a staff edit. Harmless if it happened, and free to rule out.
const LOCK_SALT: i64 = 9;

/// Builds a [`Staff`] from any row that selected the nine columns.
///
/// A macro rather than a function because every `query!` returns its own
/// anonymous record type, and this is the one place the nine fields are named.
macro_rules! staff_from {
    ($row:expr) => {
        Staff {
            id: StaffId::from_uuid($row.id),
            email: $row.email,
            display_name: $row.display_name,
            role: $row.role,
            language: $row
                .language
                .as_deref()
                .map(LanguageCode::new)
                .transpose()?,
            deactivated_at: $row.deactivated_at,
            last_sign_in_at: $row.last_sign_in_at,
            must_change_password: $row.must_change_password,
            version: $row.version,
        }
    };
}

/// Everybody who works here, active first.
///
/// No pagination, on purpose: a restaurant has tens of staff and the screen is
/// one scannable list. Deactivated people are included, because the screen has
/// a section for them and bringing somebody back is one of the six actions.
///
/// The order is computed in SQL rather than in the browser, so every client
/// agrees on it: active people first, then by display name ignoring letter
/// case, then by id, which is what makes the order total rather than merely
/// mostly decided.
///
/// # Errors
///
/// Returns [`DomainError::Unavailable`] if the read fails, and
/// [`DomainError::Invalid`] if somebody's stored language is no longer in the
/// catalogue.
pub async fn list(tx: &mut ScopedTx<'_>) -> DomainResult<Vec<Staff>> {
    let rows = sqlx::query!(
        r#"
        SELECT id, email, display_name, role AS "role: StaffRole", language,
               deactivated_at, last_sign_in_at, must_change_password, version
        FROM staff
        ORDER BY (deactivated_at IS NULL) DESC, lower(display_name), id
        "#
    )
    .fetch_all(tx.connection())
    .await?;

    // Not `.map(...).collect()` into a `Vec`: building a `LanguageCode`
    // validates against the catalogue and so can fail, and a `?` inside a
    // closure would only return from the closure.
    rows.into_iter()
        .map(|row| Ok(staff_from!(row)))
        .collect::<DomainResult<Vec<_>>>()
}

/// What creating a member of staff asks for.
///
/// No restaurant, because the transaction already names one, and no role
/// default, because "which role" is the whole decision an admin is making. No
/// flag either: the password is owed because an admin wrote it, which is a fact
/// about how this row came to exist rather than a choice the caller makes.
#[derive(Debug, Clone)]
pub struct NewStaff<'a> {
    /// What to call them on screen. Trimmed by the caller, and trimmed again
    /// here, because the trimmed value is the one that must be stored.
    pub display_name: &'a str,
    /// Their address, exactly as it was typed.
    pub email: &'a EmailAddress,
    /// The `argon2id` hash of the password the admin chose. Never the password.
    pub password_hash: &'a str,
    /// What they are allowed to be.
    pub role: StaffRole,
}

/// Adds a member of staff to the restaurant this transaction is scoped to.
///
/// The new row owes a password change from the moment it exists, because an
/// admin wrote the password and is about to read it out loud. That is set here
/// rather than passed in, so no caller can create somebody who quietly does not
/// owe one.
///
/// Whether the address is free is decided by the unique index, not by a read
/// before the insert. An application pre check would race, and it would race in
/// the direction of two accounts on one address.
///
/// # Errors
///
/// Returns [`DomainError::Conflict`] carrying [`ConflictKind::EmailTaken`] if
/// that address already belongs to an account anywhere on the platform, which
/// the caller turns into `fields.email=already_taken`, and
/// [`DomainError::Unavailable`] if a statement fails.
pub async fn create(
    tx: &mut ScopedTx<'_>,
    new: &NewStaff<'_>,
    actor: StaffId,
) -> DomainResult<Staff> {
    let display_name = new.display_name.trim();
    let id = StaffId::new();

    let row = sqlx::query!(
        r#"
        INSERT INTO staff
            (id, restaurant_id, email, password_hash, display_name, role,
             must_change_password, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, true, now())
        RETURNING id, email, display_name, role AS "role: StaffRole", language,
                  deactivated_at, last_sign_in_at, must_change_password, version
        "#,
        id.as_uuid(),
        tx.restaurant_id().as_uuid(),
        new.email.as_str(),
        new.password_hash,
        display_name,
        new.role as StaffRole,
    )
    .fetch_one(tx.connection())
    .await
    .map_err(|error| super::conflict_on(error, EMAIL_KEY, ConflictKind::EmailTaken))?;

    // A null `before`, because there was nothing before. The `after` names the
    // three things that identify the account and not one thing more: no hash,
    // and no password.
    audit::record(
        tx,
        Some(actor),
        AuditAction::StaffCreated,
        ENTITY,
        id.as_uuid(),
        None,
        Some(json!({
            "displayName": display_name,
            "email": new.email.as_str(),
            "role": new.role.as_label(),
        })),
    )
    .await?;

    Database::notify_entity_change(tx, EntityKind::Staff, id.as_uuid()).await?;

    Ok(staff_from!(row))
}

/// Changes somebody's display name, provided nobody changed them since the form
/// loaded `version`.
///
/// Touches no session. A person whose name was corrected is the same person
/// with the same access, and signing them out mid shift over a spelling would
/// be a worse product than the one that leaves them alone.
///
/// # Errors
///
/// Returns the five refusals in [`refuse_before_writing`]'s order, and
/// [`DomainError::Unavailable`] if a statement fails.
pub async fn rename(
    tx: &mut ScopedTx<'_>,
    staff_id: StaffId,
    display_name: &str,
    version: i32,
    actor: StaffId,
) -> DomainResult<Staff> {
    let display_name = display_name.trim();
    let before = refuse_before_writing(tx, staff_id, actor, Action::Rename).await?;

    let updated = sqlx::query!(
        r#"
        UPDATE staff
           SET display_name = $2, version = version + 1, updated_at = now()
         WHERE id = $1 AND version = $3 AND deactivated_at IS NULL
        RETURNING id, email, display_name, role AS "role: StaffRole", language,
                  deactivated_at, last_sign_in_at, must_change_password, version
        "#,
        staff_id.as_uuid(),
        display_name,
        version,
    )
    .fetch_optional(tx.connection())
    .await?;

    // Inside the lock nothing else could have moved the row since the read
    // above, and that read already proved it exists and is active. So zero rows
    // means exactly one thing.
    let Some(row) = updated else {
        return Err(DomainError::Conflict(ConflictKind::StaffChanged));
    };

    let after = Snapshot {
        display_name: display_name.to_owned(),
        ..before.clone()
    };
    write_audit(
        tx,
        actor,
        AuditAction::StaffEdited,
        staff_id,
        &before,
        &after,
    )
    .await?;

    // The floor, the Orders list, and every table screen name the responsible
    // waiter, so a new name has to reach them live.
    Database::notify_entity_change(tx, EntityKind::Staff, staff_id.as_uuid()).await?;

    Ok(staff_from!(row))
}

/// Changes what somebody is allowed to be, and turns every device of theirs out.
///
/// The revocation is the point of the action rather than a side effect: a
/// waiter promoted to admin, or an admin demoted to waiter, must be working
/// under the new role on their very next request and not whenever their session
/// happens to end.
///
/// Setting the role somebody already holds writes nothing, revokes nothing,
/// bumps nothing, and still succeeds. The admin asked for a state, the row is
/// already in it, and signing somebody out to change nothing would be a
/// surprising thing for a no op to do. It is checked after the four refusals,
/// so an admin cannot use it to act on their own row or on a switched off one,
/// and before the version check, because a write that changes nothing has
/// nothing to write over.
///
/// # Errors
///
/// Returns the five refusals in [`refuse_before_writing`]'s order, and
/// [`DomainError::Unavailable`] if a statement fails.
pub async fn change_role(
    tx: &mut ScopedTx<'_>,
    staff_id: StaffId,
    role: StaffRole,
    version: i32,
    actor: StaffId,
) -> DomainResult<Staff> {
    let before = refuse_before_writing(tx, staff_id, actor, Action::RoleChange(role)).await?;

    if before.role == role {
        return read_one(tx, staff_id).await;
    }

    let updated = sqlx::query!(
        r#"
        UPDATE staff
           SET role = $2, version = version + 1, updated_at = now()
         WHERE id = $1 AND version = $3 AND deactivated_at IS NULL
        RETURNING id, email, display_name, role AS "role: StaffRole", language,
                  deactivated_at, last_sign_in_at, must_change_password, version
        "#,
        staff_id.as_uuid(),
        role as StaffRole,
        version,
    )
    .fetch_optional(tx.connection())
    .await?;

    let Some(row) = updated else {
        return Err(DomainError::Conflict(ConflictKind::StaffChanged));
    };

    let after = Snapshot {
        role,
        ..before.clone()
    };
    write_audit(
        tx,
        actor,
        AuditAction::StaffRoleChanged,
        staff_id,
        &before,
        &after,
    )
    .await?;

    revoke_and_log(tx, staff_id, "a role changed").await?;

    Ok(staff_from!(row))
}

/// Writes a new password onto somebody else's row, and turns their devices out.
///
/// It takes no version. An admin resetting a password has decided that this
/// person needs a new one, and whether somebody renamed them meanwhile does not
/// change that; refusing it as stale would leave a locked out chef waiting
/// through a service while two admins sorted out a form.
///
/// The new password is owed from the moment it is written, so the admin who
/// reads it out is handing over something good for exactly one sign in.
///
/// # Errors
///
/// Returns the four refusals that apply to it in [`refuse_before_writing`]'s
/// order, and [`DomainError::Unavailable`] if a statement fails.
pub async fn reset_password(
    tx: &mut ScopedTx<'_>,
    staff_id: StaffId,
    password_hash: &str,
    actor: StaffId,
) -> DomainResult<()> {
    let before = refuse_before_writing(tx, staff_id, actor, Action::PasswordReset).await?;

    let updated = sqlx::query!(
        r#"
        UPDATE staff
           SET password_hash = $2, must_change_password = true,
               version = version + 1, updated_at = now()
         WHERE id = $1 AND deactivated_at IS NULL
        "#,
        staff_id.as_uuid(),
        password_hash,
    )
    .execute(tx.connection())
    .await?
    .rows_affected();

    if updated == 0 {
        // The row was proved live under the lock a moment ago, so this cannot
        // happen through any path in the product. Saying so loudly beats
        // reporting success for a password that was never written.
        return Err(DomainError::Conflict(ConflictKind::StaffInactive));
    }

    // The owed flag moving false to true is what a reset reads as in the log.
    // Neither side carries the password or its hash, on this path or any other.
    let after = Snapshot {
        password_owed: true,
        ..before.clone()
    };
    write_audit(
        tx,
        actor,
        AuditAction::StaffPasswordReset,
        staff_id,
        &before,
        &after,
    )
    .await?;

    revoke_and_log(tx, staff_id, "a password was reset by an admin").await?;

    Ok(())
}

/// Switches an account off, whatever that person has open on the floor.
///
/// Never refused because of the service. Open visits, unserved rounds, and open
/// bills all keep pointing at the row and the action succeeds regardless,
/// because "this person may not work here from now on" is not a thing to
/// negotiate with the dinner rush. The row is never deleted, so every bill they
/// opened still names them.
///
/// A single conditional update naming the state it expects, so a deactivate and
/// a reactivate issued at the same instant resolve to exactly one winner.
///
/// # Errors
///
/// Returns the four refusals that apply to it in [`refuse_before_writing`]'s
/// order, and [`DomainError::Unavailable`] if a statement fails.
pub async fn deactivate(
    tx: &mut ScopedTx<'_>,
    staff_id: StaffId,
    actor: StaffId,
) -> DomainResult<Staff> {
    let before = refuse_before_writing(tx, staff_id, actor, Action::Deactivate).await?;

    let updated = sqlx::query!(
        r#"
        UPDATE staff
           SET deactivated_at = now(), version = version + 1, updated_at = now()
         WHERE id = $1 AND deactivated_at IS NULL
        RETURNING id, email, display_name, role AS "role: StaffRole", language,
                  deactivated_at, last_sign_in_at, must_change_password, version
        "#,
        staff_id.as_uuid(),
    )
    .fetch_optional(tx.connection())
    .await?;

    let Some(row) = updated else {
        return Err(DomainError::Conflict(ConflictKind::StaffInactive));
    };

    let after = Snapshot {
        active: false,
        ..before.clone()
    };
    write_audit(
        tx,
        actor,
        AuditAction::StaffDeactivated,
        staff_id,
        &before,
        &after,
    )
    .await?;

    revoke_and_log(tx, staff_id, "an account was switched off").await?;

    Ok(staff_from!(row))
}

/// Brings a switched off account back, exactly as it was.
///
/// It clears the one column and nothing else. The role, the password, and
/// whether a password is owed are all left where they were, because
/// deactivation did not touch them either: that is what makes this the reverse
/// of an action rather than a second creation.
///
/// No session is revoked, because a deactivated account holds none: switching
/// it off ended every one of them, and nothing since could have opened another.
///
/// # Errors
///
/// Returns [`DomainError::NotFound`] for an id this restaurant does not have,
/// [`ConflictKind::StaffInactive`] if the account is already active, and
/// [`DomainError::Unavailable`] if a statement fails.
pub async fn reactivate(
    tx: &mut ScopedTx<'_>,
    staff_id: StaffId,
    actor: StaffId,
) -> DomainResult<Staff> {
    let before = refuse_before_writing(tx, staff_id, actor, Action::Reactivate).await?;

    let updated = sqlx::query!(
        r#"
        UPDATE staff
           SET deactivated_at = NULL, version = version + 1, updated_at = now()
         WHERE id = $1 AND deactivated_at IS NOT NULL
        RETURNING id, email, display_name, role AS "role: StaffRole", language,
                  deactivated_at, last_sign_in_at, must_change_password, version
        "#,
        staff_id.as_uuid(),
    )
    .fetch_optional(tx.connection())
    .await?;

    let Some(row) = updated else {
        return Err(DomainError::Conflict(ConflictKind::StaffInactive));
    };

    let after = Snapshot {
        active: true,
        ..before.clone()
    };
    write_audit(
        tx,
        actor,
        AuditAction::StaffReactivated,
        staff_id,
        &before,
        &after,
    )
    .await?;

    Database::notify_entity_change(tx, EntityKind::Staff, staff_id.as_uuid()).await?;

    Ok(staff_from!(row))
}

// ===========================================================================
// The shared parts: the lock, the refusal order, and the audit shape
// ===========================================================================

/// Which action is being attempted, for the refusals that differ between them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    /// A display name edit. Carries a version, and is the one targeted action
    /// an admin may aim at their own row without being refused, because it
    /// changes nothing about access.
    Rename,
    /// A role change, to the role it carries. Carries a version.
    RoleChange(StaffRole),
    /// An admin writing somebody else's password. No version.
    PasswordReset,
    /// Switching an account off. No version.
    Deactivate,
    /// Bringing one back. No version, and the one action that wants the row
    /// switched off rather than on.
    Reactivate,
}

impl Action {
    /// Whether an admin aiming this at their own row is refused.
    ///
    /// The three that change access are. A rename is not: correcting your own
    /// spelling is not a way to escalate anything, and the account screen would
    /// let you do it anyway.
    const fn refuses_self(self) -> bool {
        matches!(
            self,
            Self::RoleChange(_) | Self::PasswordReset | Self::Deactivate
        )
    }

    /// Whether this action could leave the restaurant with no active admin.
    fn could_remove_the_last_admin(self, target_is_an_active_admin: bool) -> bool {
        if !target_is_an_active_admin {
            return false;
        }

        match self {
            Self::RoleChange(role) => role != StaffRole::Admin,
            Self::Deactivate => true,
            Self::Rename | Self::PasswordReset | Self::Reactivate => false,
        }
    }
}

/// The four mutable fields, as they were before a write and as they are after.
///
/// One shape shared by all six actions, so the log reads the same whichever one
/// wrote the row: a password reset is the owed flag moving false to true, and a
/// deactivation is active moving true to false. Reading the log back is then a
/// comparison rather than five different comparisons.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Snapshot {
    /// What they are called.
    display_name: String,
    /// What they are allowed to be.
    role: StaffRole,
    /// Whether the account is switched on.
    active: bool,
    /// Whether they still owe their own password.
    password_owed: bool,
}

impl Snapshot {
    /// The four fields as an audit value. Never anything else: no hash, no
    /// password, and no session token appears on either side of any row here.
    fn as_value(&self) -> Value {
        json!({
            "displayName": self.display_name,
            "role": self.role.as_label(),
            "active": self.active,
            "passwordOwed": self.password_owed,
        })
    }
}

/// Takes the restaurant's advisory lock for the rest of this transaction.
///
/// Transaction level, so the commit releases it and there is nothing to unlock
/// on any path, including a failure. Keyed on the restaurant, so two admins of
/// one restaurant queue and two admins of different restaurants never wait on
/// each other.
async fn lock_restaurant(tx: &mut ScopedTx<'_>) -> DomainResult<()> {
    let restaurant_id = tx.restaurant_id().as_uuid();

    sqlx::query!(
        "SELECT pg_advisory_xact_lock(hashtextextended($1::uuid::text, $2))",
        restaurant_id,
        LOCK_SALT,
    )
    .execute(tx.connection())
    .await?;

    Ok(())
}

/// Everything that can refuse a targeted action, in one fixed order.
///
/// Takes the lock, reads the target once, and decides all four of the refusals
/// that are decided before a write. The caller then makes its one conditional
/// update, and a zero row result there is the fifth refusal.
///
/// Returns the target's four mutable fields as they are now, which is the
/// `before` side of the audit row and saves the caller a second read.
async fn refuse_before_writing(
    tx: &mut ScopedTx<'_>,
    staff_id: StaffId,
    actor: StaffId,
    action: Action,
) -> DomainResult<Snapshot> {
    lock_restaurant(tx).await?;

    let found = sqlx::query!(
        r#"
        SELECT display_name, role AS "role: StaffRole", deactivated_at, must_change_password
        FROM staff
        WHERE id = $1
        "#,
        staff_id.as_uuid(),
    )
    .fetch_optional(tx.connection())
    .await?;

    // An id in another restaurant reads exactly like an id nobody has, because
    // the transaction is scoped and row level security filtered it out before
    // this code ever saw it. A caller cannot tell the two apart, which is the
    // point.
    let Some(row) = found else {
        return Err(DomainError::NotFound);
    };

    if action.refuses_self() && staff_id == actor {
        return Err(DomainError::Conflict(ConflictKind::CannotActOnSelf));
    }

    let active = row.deactivated_at.is_none();

    // Reactivate is the one action that wants the row switched off. Every other
    // one wants it on, and reads the same refusal when it is not.
    if active == matches!(action, Action::Reactivate) {
        return Err(DomainError::Conflict(ConflictKind::StaffInactive));
    }

    if action.could_remove_the_last_admin(active && row.role == StaffRole::Admin) {
        let remaining = sqlx::query!(
            r#"
            SELECT count(*) AS "count!"
            FROM staff
            WHERE role = 'admin' AND deactivated_at IS NULL
            "#
        )
        .fetch_one(tx.connection())
        .await?
        .count;

        // Counted inside the lock, so the answer cannot change between here and
        // the write. Without the lock, two admins demoting each other would
        // both read two and both write.
        if remaining <= 1 {
            return Err(DomainError::Conflict(ConflictKind::LastAdmin));
        }
    }

    Ok(Snapshot {
        display_name: row.display_name,
        role: row.role,
        active,
        password_owed: row.must_change_password,
    })
}

/// Writes one audit row in the one shape all six actions share.
async fn write_audit(
    tx: &mut ScopedTx<'_>,
    actor: StaffId,
    action: AuditAction,
    staff_id: StaffId,
    before: &Snapshot,
    after: &Snapshot,
) -> DomainResult<()> {
    audit::record(
        tx,
        Some(actor),
        action,
        ENTITY,
        staff_id.as_uuid(),
        Some(before.as_value()),
        Some(after.as_value()),
    )
    .await
}

/// Ends every session this person holds, and says so, then notifies.
///
/// In the same transaction as the change that caused it, so the two commit
/// together or neither does. A revocation that landed without its change would
/// sign somebody out for nothing; a change that landed without its revocation
/// would leave a demoted admin working as an admin until their session expired.
async fn revoke_and_log(tx: &mut ScopedTx<'_>, staff_id: StaffId, what: &str) -> DomainResult<()> {
    let revoked = sessions::revoke_all(tx, staff_id).await?;

    tracing::info!(
        staff_id = %staff_id,
        revoked_sessions = revoked,
        "{what}, so every session they held was ended"
    );

    Database::notify_entity_change(tx, EntityKind::Staff, staff_id.as_uuid()).await
}

/// Reads one staff member back, for the no op role change that wrote nothing.
async fn read_one(tx: &mut ScopedTx<'_>, staff_id: StaffId) -> DomainResult<Staff> {
    let row = sqlx::query!(
        r#"
        SELECT id, email, display_name, role AS "role: StaffRole", language,
               deactivated_at, last_sign_in_at, must_change_password, version
        FROM staff
        WHERE id = $1
        "#,
        staff_id.as_uuid(),
    )
    .fetch_optional(tx.connection())
    .await?
    .ok_or(DomainError::NotFound)?;

    Ok(staff_from!(row))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// covers: AC-12
    ///
    /// Which of the five actions refuse an admin aiming at their own row.
    /// Written out one at a time rather than derived, because a rule that
    /// generated the answer would move with any mistake in it.
    #[test]
    fn the_three_actions_that_change_access_refuse_an_admins_own_row() {
        assert!(Action::RoleChange(StaffRole::Waiter).refuses_self());
        assert!(Action::PasswordReset.refuses_self());
        assert!(Action::Deactivate.refuses_self());

        assert!(
            !Action::Rename.refuses_self(),
            "correcting your own name was refused, which is not an access change and is \
             something the account screen already allows"
        );
        assert!(!Action::Reactivate.refuses_self());
    }

    /// covers: AC-12
    ///
    /// Which actions have to count the admins first. Getting this wrong in the
    /// permissive direction is a restaurant locked out of its own account.
    #[test]
    fn only_demoting_or_switching_off_an_active_admin_can_reach_the_last_one() {
        assert!(Action::Deactivate.could_remove_the_last_admin(true));
        assert!(Action::RoleChange(StaffRole::Waiter).could_remove_the_last_admin(true));
        assert!(Action::RoleChange(StaffRole::Chef).could_remove_the_last_admin(true));

        assert!(
            !Action::RoleChange(StaffRole::Admin).could_remove_the_last_admin(true),
            "making an admin an admin was treated as removing one, so the last admin could \
             not be left alone"
        );
        assert!(!Action::Rename.could_remove_the_last_admin(true));
        assert!(!Action::PasswordReset.could_remove_the_last_admin(true));
        assert!(!Action::Reactivate.could_remove_the_last_admin(true));

        // Nothing aimed at somebody who is not an active admin can reach it.
        for action in [
            Action::Deactivate,
            Action::RoleChange(StaffRole::Waiter),
            Action::Rename,
            Action::PasswordReset,
            Action::Reactivate,
        ] {
            assert!(!action.could_remove_the_last_admin(false));
        }
    }

    /// covers: AC-17
    ///
    /// The one shape all six actions share, and the one thing that must never
    /// appear in it.
    #[test]
    fn an_audit_value_carries_the_four_mutable_fields_and_nothing_else() {
        let snapshot = Snapshot {
            display_name: "Ada".to_owned(),
            role: StaffRole::Waiter,
            active: true,
            password_owed: false,
        };

        let value = snapshot.as_value();

        assert_eq!(value["displayName"], "Ada");
        assert_eq!(value["role"], "waiter");
        assert_eq!(value["active"], true);
        assert_eq!(value["passwordOwed"], false);

        let object = value.as_object().expect("an audit value is an object");
        assert_eq!(
            object.len(),
            4,
            "the audit shape grew a field: {value}. Every one of the six actions writes this \
             shape, so a fifth key is a fifth key on all of them."
        );
    }

    /// covers: AC-9, AC-17
    ///
    /// A password reset reads as the owed flag moving, and the plain password
    /// and its hash appear on neither side. This is the one audit row written
    /// on a path that has a password in scope, so it is the one worth pinning.
    #[test]
    fn a_password_reset_puts_no_password_and_no_hash_in_the_log() {
        let before = Snapshot {
            display_name: "Ada".to_owned(),
            role: StaffRole::Chef,
            active: true,
            password_owed: false,
        };
        let after = Snapshot {
            password_owed: true,
            ..before.clone()
        };

        let written = format!("{} {}", before.as_value(), after.as_value());

        assert_eq!(before.as_value()["passwordOwed"], false);
        assert_eq!(after.as_value()["passwordOwed"], true);
        assert!(
            !written.contains("password_hash") && !written.contains("$argon2"),
            "a hash reached the audit log in {written}"
        );
    }
}
