//! An admin putting real people on the floor, and everything that refuses them.
//!
//! Two halves, and they need different machinery.
//!
//! Most of it rolls back: a scoped transaction, a seeded restaurant, an
//! assertion, and a drop, so the database is left as it was found. That covers
//! the create path, the five refusals and the order they arrive in, the
//! revocations, and the audit rows.
//!
//! The last two commit, because a race is invisible between two transactions
//! that never do. Both delete their restaurant afterwards, which cascades.
//!
//! Every one of them connects as `app_api`, never as the schema owner, so the
//! row level security policies are the ones in force. That is what makes the
//! cross restaurant test mean anything: run as the owner it would pass while
//! proving nothing.
//!
//! Covers spec 0009's AC-1, AC-2, AC-6 through AC-14, AC-16, and AC-17.

mod common;

use api::domain::credentials::EmailAddress;
use api::domain::enums::StaffRole;
use api::domain::error::{ConflictKind, DomainError};
use api::domain::ids::{RestaurantId, StaffId};
use api::infrastructure::db::ScopedTx;
use api::infrastructure::db::repository::{sessions, staff};

/// A hash shaped like the real thing, without paying for `argon2` in a test.
///
/// Nothing here ever verifies a password: what these tests are about is which
/// row was written and what it did to that person's sessions. The one thing
/// that matters about this value is that it is not a password, so a test that
/// accidentally wrote it somewhere readable would be obvious.
const HASH: &str = "$argon2id$v=19$m=19456,t=2,p=1$c29tZXNhbHQ$notarealhashatall";

/// Adds somebody to the restaurant this transaction is scoped to.
async fn add(
    tx: &mut ScopedTx<'_>,
    admin: StaffId,
    name: &str,
    email: &str,
    role: StaffRole,
) -> api::domain::people::Staff {
    let address = EmailAddress::new(email).expect("a test address");

    staff::create(
        tx,
        &staff::NewStaff {
            display_name: name,
            email: &address,
            password_hash: HASH,
            role,
        },
        admin,
    )
    .await
    .expect("creating a member of staff")
}

/// Tries to create somebody at the scoped restaurant with this address.
///
/// The fallible twin of [`add`], for the refusals rather than the happy path.
async fn create_with(
    tx: &mut ScopedTx<'_>,
    admin: StaffId,
    email: &str,
) -> Result<api::domain::people::Staff, DomainError> {
    let address = EmailAddress::new(email).expect("a test address");

    staff::create(
        tx,
        &staff::NewStaff {
            display_name: "Someone Else",
            email: &address,
            password_hash: HASH,
            role: StaffRole::Waiter,
        },
        admin,
    )
    .await
}

/// How many live sessions this person holds.
async fn live_sessions(tx: &mut ScopedTx<'_>, staff_id: StaffId) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM sessions WHERE staff_id = $1 AND revoked_at IS NULL")
        .bind(staff_id.as_uuid())
        .fetch_one(tx.connection())
        .await
        .expect("counting live sessions")
}

/// Opens a session for somebody, so a revocation has something to revoke.
async fn sign_in(tx: &mut ScopedTx<'_>, staff_id: StaffId, token: u8) {
    sessions::open(tx, staff_id, &[token; 32])
        .await
        .expect("opening a session");
}

/// Whether an outcome is the "that account is switched off" refusal.
///
/// A function rather than a closure, because the five calls below pass five
/// different success types and a closure can only ever have one.
fn is_inactive<T>(outcome: &Result<T, DomainError>) -> bool {
    matches!(
        outcome,
        Err(DomainError::Conflict(ConflictKind::StaffInactive))
    )
}

/// Whether an outcome is the "not to your own row" refusal. Generic for the
/// same reason as [`is_inactive`].
fn is_self_action<T>(outcome: &Result<T, DomainError>) -> bool {
    matches!(
        outcome,
        Err(DomainError::Conflict(ConflictKind::CannotActOnSelf))
    )
}

/// Every audit action written for this staff member, oldest first.
async fn audit_actions(tx: &mut ScopedTx<'_>, staff_id: StaffId) -> Vec<String> {
    sqlx::query_scalar(
        "SELECT action FROM audit_log WHERE entity_id = $1 ORDER BY occurred_at, action",
    )
    .bind(staff_id.as_uuid())
    .fetch_all(tx.connection())
    .await
    .expect("reading the audit log")
}

/// covers: AC-1, AC-6
///
/// The whole of the create path in one pass: what the row holds, and where the
/// list puts it.
#[tokio::test]
async fn a_created_account_owes_a_password_and_has_never_signed_in() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut tx, restaurant_id).await;

    let suffix = restaurant_id.as_uuid().simple().to_string();
    let created = add(
        &mut tx,
        f.admin,
        "  Nina Newcomer  ",
        &format!("Nina-{suffix}@Example.Test"),
        StaffRole::Waiter,
    )
    .await;

    assert_eq!(created.role, StaffRole::Waiter);
    assert!(
        created.must_change_password,
        "a password an admin wrote is good for more than one sign in"
    );
    assert_eq!(created.version, 1);
    assert_eq!(created.deactivated_at, None);
    assert_eq!(created.last_sign_in_at, None);
    assert_eq!(
        created.display_name, "Nina Newcomer",
        "the padded name was stored rather than the trimmed one"
    );
    assert_eq!(
        created.email,
        format!("Nina-{suffix}@Example.Test"),
        "the address was flattened, so an admin sees something they did not type"
    );

    // AC-6: active first, then by display name ignoring case, then by id. The
    // fixture's three are Ada Admin, Cleo Chef, and Wes Waiter, so Nina lands
    // between Cleo and Wes.
    let listed = staff::list(&mut tx).await.expect("reading the list");
    let names: Vec<&str> = listed
        .iter()
        .map(|member| member.display_name.as_str())
        .collect();
    assert_eq!(
        names,
        vec!["Ada Admin", "Cleo Chef", "Nina Newcomer", "Wes Waiter"],
        "the list is not in the one order every client is meant to agree on"
    );
}

/// covers: AC-2
///
/// An address held anywhere on the platform is taken, and the two ways it can be
/// taken that an admin cannot see are refused exactly like the one they can.
#[tokio::test]
async fn an_address_already_in_use_is_refused_wherever_it_is_in_use() {
    let database = common::database().await;
    let alpha = RestaurantId::new();
    let beta = RestaurantId::new();

    let mut tx = database.begin_scoped(alpha).await.expect("opening scoped");
    let alpha_fixture = common::seed(&mut tx, alpha).await;

    // Beta is seeded with raw statements that name it explicitly, which is what
    // `common::seed` does. No repository call runs while re scoped: `rescope`
    // moves the policy and deliberately leaves the transaction handle's own
    // restaurant alone, so a repository write there would name one restaurant
    // while the policy checked the other.
    common::rescope(&mut tx, beta).await;
    common::seed(&mut tx, beta).await;
    common::rescope(&mut tx, alpha).await;

    // Somebody at alpha who has since been switched off. Created before any of
    // the three attempts below, because each of those raises a real unique
    // index violation, and one of those aborts the whole transaction.
    let suffix = alpha.as_uuid().simple().to_string();
    let leaver_address = format!("gone-{suffix}@example.test");
    let leaver = add(
        &mut tx,
        alpha_fixture.admin,
        "Ex Employee",
        &leaver_address,
        StaffRole::Chef,
    )
    .await;
    staff::deactivate(&mut tx, leaver.id, alpha_fixture.admin)
        .await
        .expect("switching the leaver off");

    // The three ways the address can already be taken, each inside its own
    // savepoint so the next one has a transaction left to run in.
    let beta_address = format!("waiter-{}@example.test", beta.as_uuid().simple());
    let attempts = [
        // Held at another restaurant, which alpha cannot see at all.
        ("across", beta_address),
        // Held by somebody here who has been switched off. Deactivating does
        // not free an address, because a reactivation has to be able to give it
        // back.
        ("reused", leaver_address.clone()),
        // And letter case is not a way around either of them.
        ("shouted", leaver_address.to_uppercase()),
    ];

    for (name, address) in attempts {
        common::savepoint(&mut tx, name).await;
        let refused = create_with(&mut tx, alpha_fixture.admin, &address).await;
        assert!(
            matches!(
                refused,
                Err(DomainError::Conflict(ConflictKind::EmailTaken))
            ),
            "the address {address} was accepted as a second account ({name}): {refused:?}"
        );
        common::rollback_to(&mut tx, name).await;
    }
}

/// covers: AC-7, AC-8, AC-14
///
/// A rename leaves every session alone, a role change ends all of them, and a
/// role change to the role somebody already holds does neither.
#[tokio::test]
async fn a_rename_keeps_sessions_a_role_change_ends_them_and_a_no_op_does_nothing() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut tx, restaurant_id).await;

    sign_in(&mut tx, f.waiter, 1).await;
    sign_in(&mut tx, f.waiter, 2).await;
    assert_eq!(live_sessions(&mut tx, f.waiter).await, 2);

    let before = staff::list(&mut tx)
        .await
        .expect("reading the list")
        .into_iter()
        .find(|member| member.id == f.waiter)
        .expect("the waiter works here");

    let renamed = staff::rename(&mut tx, f.waiter, "  Wes Renamed ", before.version, f.admin)
        .await
        .expect("renaming the waiter");

    assert_eq!(renamed.display_name, "Wes Renamed");
    assert_eq!(renamed.version, before.version + 1);
    assert_eq!(
        live_sessions(&mut tx, f.waiter).await,
        2,
        "a rename signed somebody out mid shift over a spelling"
    );

    // AC-14: the version the rename just consumed is stale now.
    let stale = staff::rename(&mut tx, f.waiter, "Wes Again", before.version, f.admin).await;
    assert!(
        matches!(
            stale,
            Err(DomainError::Conflict(ConflictKind::StaffChanged))
        ),
        "a stale rename was accepted: {stale:?}"
    );

    // AC-8: the no op writes nothing, revokes nothing, bumps nothing, succeeds.
    let unchanged = staff::change_role(
        &mut tx,
        f.waiter,
        StaffRole::Waiter,
        renamed.version,
        f.admin,
    )
    .await
    .expect("setting the role somebody already holds is not an error");

    assert_eq!(
        unchanged.version, renamed.version,
        "a role change that changed nothing still bumped the version, so every open form went \
         stale for nothing"
    );
    assert_eq!(
        live_sessions(&mut tx, f.waiter).await,
        2,
        "a role change that changed nothing signed somebody out"
    );

    // And the real one does end them.
    let promoted = staff::change_role(
        &mut tx,
        f.waiter,
        StaffRole::Chef,
        unchanged.version,
        f.admin,
    )
    .await
    .expect("changing the role");

    assert_eq!(promoted.role, StaffRole::Chef);
    assert_eq!(promoted.version, unchanged.version + 1);
    assert_eq!(
        live_sessions(&mut tx, f.waiter).await,
        0,
        "a role changed and the person kept working under the old one"
    );
}

/// covers: AC-9
///
/// A reset writes the hash, puts the flag back to owed, ends every session, and
/// takes no version.
#[tokio::test]
async fn an_admin_reset_owes_a_password_again_and_ends_every_session() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut tx, restaurant_id).await;

    sign_in(&mut tx, f.chef, 3).await;

    // The fixture's people were written straight into the table, so none of
    // them owes anything, which is the state a reset has to move.
    let before = staff::list(&mut tx)
        .await
        .expect("reading the list")
        .into_iter()
        .find(|member| member.id == f.chef)
        .expect("the chef works here");
    assert!(!before.must_change_password);

    staff::reset_password(&mut tx, f.chef, HASH, f.admin)
        .await
        .expect("resetting the chef's password");

    let after = staff::list(&mut tx)
        .await
        .expect("reading the list")
        .into_iter()
        .find(|member| member.id == f.chef)
        .expect("the chef still works here");

    assert!(
        after.must_change_password,
        "a password an admin wrote did not have to be replaced"
    );
    assert_eq!(after.version, before.version + 1);
    assert_eq!(live_sessions(&mut tx, f.chef).await, 0);

    // AC-17: no hash and no password on either side of the row it wrote.
    let values: Vec<String> = sqlx::query_scalar(
        "SELECT coalesce(before::text, '') || coalesce(after::text, '')
         FROM audit_log WHERE entity_id = $1 AND action = 'staff_password_reset'",
    )
    .bind(f.chef.as_uuid())
    .fetch_all(tx.connection())
    .await
    .expect("reading the audit row");

    assert_eq!(values.len(), 1, "a reset wrote {} audit rows", values.len());
    let written = values.concat();
    assert!(
        !written.contains("argon2") && !written.contains(HASH),
        "a hash reached the audit log in {written}"
    );
    assert!(
        written.contains("passwordOwed"),
        "the audit row does not record the owed flag moving, so a reset reads as nothing at all"
    );
}

/// covers: AC-10, AC-11
///
/// Switching somebody off never negotiates with the floor, and bringing them
/// back changes nothing else.
#[tokio::test]
async fn switching_an_account_off_leaves_the_floor_alone_and_bringing_it_back_changes_nothing_else()
{
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut tx, restaurant_id).await;

    // The waiter is mid service: a party at a table and a bill open on it.
    let (visit, _bill) =
        common::seed_visit_and_bill(&mut tx, restaurant_id, f.table_one, f.waiter).await;

    // They owe a password too, so bringing them back has something to leave
    // alone. Before the sign in, because a reset ends every session and would
    // otherwise be what the assertion below was measuring.
    staff::reset_password(&mut tx, f.waiter, HASH, f.admin)
        .await
        .expect("resetting the waiter's password");

    sign_in(&mut tx, f.waiter, 4).await;
    assert_eq!(live_sessions(&mut tx, f.waiter).await, 1);

    let switched_off = staff::deactivate(&mut tx, f.waiter, f.admin)
        .await
        .expect("switching the waiter off mid service");

    assert!(switched_off.deactivated_at.is_some());
    assert_eq!(live_sessions(&mut tx, f.waiter).await, 0);

    let still_open: i64 =
        sqlx::query_scalar("SELECT count(*) FROM visits WHERE id = $1 AND status = 'open'")
            .bind(visit)
            .fetch_one(tx.connection())
            .await
            .expect("counting the open visit");
    assert_eq!(
        still_open, 1,
        "switching a waiter off closed the party they were serving"
    );

    let brought_back = staff::reactivate(&mut tx, f.waiter, f.admin)
        .await
        .expect("bringing the waiter back");

    assert_eq!(brought_back.deactivated_at, None);
    assert_eq!(
        brought_back.role, switched_off.role,
        "bringing somebody back changed their role"
    );
    assert!(
        brought_back.must_change_password,
        "bringing somebody back quietly settled a password they still owed"
    );
}

/// covers: AC-13
///
/// Every write except reactivate needs the account switched on, and reactivate
/// needs it switched off.
#[tokio::test]
async fn a_switched_off_account_refuses_every_write_except_being_brought_back() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut tx, restaurant_id).await;

    let switched_off = staff::deactivate(&mut tx, f.chef, f.admin)
        .await
        .expect("switching the chef off");

    let renamed = staff::rename(&mut tx, f.chef, "Anything", switched_off.version, f.admin).await;
    assert!(
        is_inactive(&renamed),
        "a switched off row was renamed: {renamed:?}"
    );

    let re_roled = staff::change_role(
        &mut tx,
        f.chef,
        StaffRole::Waiter,
        switched_off.version,
        f.admin,
    )
    .await;
    assert!(
        is_inactive(&re_roled),
        "a switched off row was re roled: {re_roled:?}"
    );

    let reset = staff::reset_password(&mut tx, f.chef, HASH, f.admin).await;
    assert!(
        is_inactive(&reset),
        "a switched off row had a password written: {reset:?}"
    );

    let again = staff::deactivate(&mut tx, f.chef, f.admin).await;
    assert!(
        is_inactive(&again),
        "a switched off row was switched off twice: {again:?}"
    );

    // And the mirror: an active row cannot be brought back.
    let already_active = staff::reactivate(&mut tx, f.waiter, f.admin).await;
    assert!(
        is_inactive(&already_active),
        "an account that was already active was brought back: {already_active:?}"
    );
}

/// covers: AC-12, AC-14
///
/// The two guard rails, and the fixed order the five refusals arrive in.
#[tokio::test]
async fn the_guard_rails_hold_and_the_refusals_arrive_in_one_fixed_order() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut tx, restaurant_id).await;

    // AC-12: an admin's own row, through all three actions that change access.
    let own_role = staff::change_role(&mut tx, f.admin, StaffRole::Waiter, 1, f.admin).await;
    assert!(
        is_self_action(&own_role),
        "an admin demoted themselves: {own_role:?}"
    );

    let own_reset = staff::reset_password(&mut tx, f.admin, HASH, f.admin).await;
    assert!(
        is_self_action(&own_reset),
        "an admin reset their own password from the staff screen: {own_reset:?}"
    );

    let own_off = staff::deactivate(&mut tx, f.admin, f.admin).await;
    assert!(
        is_self_action(&own_off),
        "an admin switched themselves off: {own_off:?}"
    );

    // AC-12: the fixture has exactly one admin, so a second one has to exist
    // before the first can be demoted, and once it does the first can go.
    let suffix = restaurant_id.as_uuid().simple().to_string();
    let second = add(
        &mut tx,
        f.admin,
        "Bea Backup",
        &format!("bea-{suffix}@example.test"),
        StaffRole::Admin,
    )
    .await;

    let last_one = staff::change_role(
        &mut tx,
        second.id,
        StaffRole::Waiter,
        second.version,
        f.admin,
    )
    .await
    .expect("there are two admins, so one of them may be demoted");
    assert_eq!(last_one.role, StaffRole::Waiter);

    // Now there is one again, and it cannot be the caller's, so promote and
    // aim at the other.
    let promoted = staff::change_role(
        &mut tx,
        second.id,
        StaffRole::Admin,
        last_one.version,
        f.admin,
    )
    .await
    .expect("promoting them back");
    let deactivated_first = staff::deactivate(&mut tx, second.id, f.admin)
        .await
        .expect("two admins, so one may be switched off");
    assert!(deactivated_first.deactivated_at.is_some());
    assert_eq!(promoted.role, StaffRole::Admin);

    // AC-14: the fixed order, checked where two refusals apply at once. The
    // chef is switched off and the version is stale; `staff_inactive` wins,
    // because a reload does not cure it and a stale version does.
    let switched_off = staff::deactivate(&mut tx, f.chef, f.admin)
        .await
        .expect("switching the chef off");
    let both = staff::change_role(
        &mut tx,
        f.chef,
        StaffRole::Waiter,
        switched_off.version - 1,
        f.admin,
    )
    .await;
    assert!(
        matches!(
            both,
            Err(DomainError::Conflict(ConflictKind::StaffInactive))
        ),
        "a stale edit aimed at a switched off person reported the version rather than the \
         thing a reload cannot fix: {both:?}"
    );

    // And an id nobody has outranks all of them.
    let nobody = staff::rename(&mut tx, StaffId::new(), "Ghost", 1, f.admin).await;
    assert!(
        matches!(nobody, Err(DomainError::NotFound)),
        "an id nobody has read as something other than not found: {nobody:?}"
    );
}

/// covers: AC-17
///
/// Each of the six actions writes exactly one row, named the way the log is
/// read back.
#[tokio::test]
async fn each_of_the_six_actions_writes_exactly_one_audit_row() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut tx, restaurant_id).await;

    let suffix = restaurant_id.as_uuid().simple().to_string();
    let person = add(
        &mut tx,
        f.admin,
        "Rae Rounder",
        &format!("rae-{suffix}@example.test"),
        StaffRole::Waiter,
    )
    .await;

    let renamed = staff::rename(&mut tx, person.id, "Rae Renamed", person.version, f.admin)
        .await
        .expect("renaming");
    let re_roled = staff::change_role(
        &mut tx,
        person.id,
        StaffRole::Chef,
        renamed.version,
        f.admin,
    )
    .await
    .expect("changing the role");
    staff::reset_password(&mut tx, person.id, HASH, f.admin)
        .await
        .expect("resetting the password");
    let switched_off = staff::deactivate(&mut tx, person.id, f.admin)
        .await
        .expect("switching off");
    staff::reactivate(&mut tx, person.id, f.admin)
        .await
        .expect("bringing back");
    assert_eq!(re_roled.role, StaffRole::Chef);
    assert!(switched_off.deactivated_at.is_some());

    let mut written = audit_actions(&mut tx, person.id).await;
    written.sort();

    let mut expected = vec![
        "staff_created",
        "staff_edited",
        "staff_role_changed",
        "staff_password_reset",
        "staff_deactivated",
        "staff_reactivated",
    ];
    expected.sort_unstable();

    assert_eq!(
        written, expected,
        "the six actions did not write exactly one row each"
    );

    // Every one of them names the admin who did it, not the person it was done
    // to. A log that named the target would answer the wrong question.
    let actors: Vec<uuid::Uuid> =
        sqlx::query_scalar("SELECT actor_staff_id FROM audit_log WHERE entity_id = $1")
            .bind(person.id.as_uuid())
            .fetch_all(tx.connection())
            .await
            .expect("reading the actors");
    assert!(
        actors.iter().all(|actor| *actor == f.admin.as_uuid()),
        "an audit row named somebody other than the acting admin"
    );
}

/// covers: AC-16
///
/// An admin of one restaurant can neither read nor write another's staff, run
/// as `app_api` so the policies are the ones in force.
#[tokio::test]
async fn an_admin_can_neither_read_nor_write_another_restaurants_staff() {
    let database = common::database().await;
    let alpha = RestaurantId::new();
    let beta = RestaurantId::new();

    let mut tx = database.begin_scoped(alpha).await.expect("opening scoped");
    let alpha_fixture = common::seed(&mut tx, alpha).await;

    common::rescope(&mut tx, beta).await;
    let beta_fixture = common::seed(&mut tx, beta).await;

    common::rescope(&mut tx, alpha).await;

    // The read shows alpha's own three and nobody else's.
    let listed = staff::list(&mut tx).await.expect("reading the list");
    assert_eq!(
        listed.len(),
        3,
        "alpha saw {} staff, which is not its own three",
        listed.len()
    );
    assert!(
        listed.iter().all(|member| member.id != beta_fixture.waiter),
        "alpha read a member of beta's staff"
    );

    // And every write aimed at beta's waiter reads as not found, which is the
    // same answer an id nobody has gets.
    let renamed = staff::rename(
        &mut tx,
        beta_fixture.waiter,
        "Taken Over",
        1,
        alpha_fixture.admin,
    )
    .await;
    assert!(matches!(renamed, Err(DomainError::NotFound)));

    let re_roled = staff::change_role(
        &mut tx,
        beta_fixture.waiter,
        StaffRole::Admin,
        1,
        alpha_fixture.admin,
    )
    .await;
    assert!(matches!(re_roled, Err(DomainError::NotFound)));

    let reset =
        staff::reset_password(&mut tx, beta_fixture.waiter, HASH, alpha_fixture.admin).await;
    assert!(matches!(reset, Err(DomainError::NotFound)));

    let switched_off = staff::deactivate(&mut tx, beta_fixture.waiter, alpha_fixture.admin).await;
    assert!(matches!(switched_off, Err(DomainError::NotFound)));

    let brought_back = staff::reactivate(&mut tx, beta_fixture.waiter, alpha_fixture.admin).await;
    assert!(matches!(brought_back, Err(DomainError::NotFound)));

    // Beta's waiter really is untouched, checked from beta's own scope rather
    // than inferred from the five refusals.
    common::rescope(&mut tx, beta).await;
    let beta_staff = staff::list(&mut tx).await.expect("reading beta's list");
    let waiter = beta_staff
        .iter()
        .find(|member| member.id == beta_fixture.waiter)
        .expect("beta's waiter still works there");
    assert_eq!(waiter.role, StaffRole::Waiter);
    assert_eq!(waiter.deactivated_at, None);
    assert_eq!(waiter.version, 1, "somebody else's admin bumped beta's row");
}

/// covers: AC-12
///
/// Two admins demoting each other at the same instant. This is the one the
/// advisory lock exists for, and it commits, because a race between two
/// transactions that never commit is not a race.
#[tokio::test]
async fn two_admins_demoting_each_other_at_once_leave_exactly_one_standing() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();

    let mut setup = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut setup, restaurant_id).await;

    let suffix = restaurant_id.as_uuid().simple().to_string();
    let second = add(
        &mut setup,
        f.admin,
        "Bea Backup",
        &format!("bea-{suffix}@example.test"),
        StaffRole::Admin,
    )
    .await;
    setup.commit().await.expect("committing the fixture");

    let first_admin = f.admin;
    let second_admin = second.id;
    let second_version = second.version;

    // The first admin demotes the second, and holds the restaurant's lock.
    let mut first = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening the first transaction");
    staff::change_role(
        &mut first,
        second_admin,
        StaffRole::Waiter,
        second_version,
        first_admin,
    )
    .await
    .expect("the first admin demotes the second");

    // The second admin demotes the first, and is now blocked behind that lock.
    let other = database.clone();
    let racing = tokio::spawn(async move {
        let mut tx = other
            .begin_scoped(restaurant_id)
            .await
            .expect("opening the second transaction");
        let outcome = staff::change_role(&mut tx, first_admin, StaffRole::Waiter, 1, second_admin)
            .await
            .map(|member| member.role);
        // Committing whatever it managed, which is the point: if the lock did
        // not hold, this write lands and the restaurant has no admin left.
        let _ = tx.commit().await;
        outcome
    });

    first.commit().await.expect("the first admin's work lands");

    let outcome = racing.await.expect("the second admin's task ran");
    assert!(
        matches!(outcome, Err(DomainError::Conflict(ConflictKind::LastAdmin))),
        "both admins were demoted, so the restaurant is locked out of its own account: {outcome:?}"
    );

    let mut check = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let admins: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM staff WHERE role = 'admin' AND deactivated_at IS NULL",
    )
    .fetch_one(check.connection())
    .await
    .expect("counting the admins");
    assert_eq!(
        admins, 1,
        "the restaurant was left with {admins} active admins"
    );
    drop(check);

    common::drop_restaurant(&database, restaurant_id).await;
}

/// covers: AC-13
///
/// A deactivate and a reactivate issued at the same instant. The conditional
/// update naming the state it expects is what decides it, and the outcome is
/// always one of the two intents rather than a blend.
#[tokio::test]
async fn a_deactivate_and_a_reactivate_at_once_resolve_to_exactly_one_winner() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();

    let mut setup = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut setup, restaurant_id).await;
    let admin = f.admin;
    let chef = f.chef;
    setup.commit().await.expect("committing the fixture");

    // The first caller switches the chef off, and holds the lock.
    let mut first = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening the first transaction");
    staff::deactivate(&mut first, chef, admin)
        .await
        .expect("the first admin switches the chef off");

    // The second tries to bring them back, at the same instant.
    let other = database.clone();
    let racing = tokio::spawn(async move {
        let mut tx = other
            .begin_scoped(restaurant_id)
            .await
            .expect("opening the second transaction");
        let outcome = staff::reactivate(&mut tx, chef, admin)
            .await
            .map(|member| member.deactivated_at.is_none());
        let _ = tx.commit().await;
        outcome
    });

    first.commit().await.expect("the first admin's work lands");

    let outcome = racing.await.expect("the second admin's task ran");

    // Either the reactivate lost the race and read `staff_inactive`, or it
    // arrived after the deactivate committed and genuinely brought them back.
    // Both are one of the two intents. What must not happen is a blend, and the
    // check below is what says which of the two the row actually holds.
    let mut check = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let active: bool = sqlx::query_scalar("SELECT deactivated_at IS NULL FROM staff WHERE id = $1")
        .bind(chef.as_uuid())
        .fetch_one(check.connection())
        .await
        .expect("reading the chef");
    drop(check);

    match outcome {
        Ok(true) => assert!(
            active,
            "the reactivate said it worked and the row is switched off"
        ),
        Err(DomainError::Conflict(ConflictKind::StaffInactive)) => assert!(
            !active,
            "the reactivate was refused as a loser and the row is active anyway"
        ),
        other => panic!("the race ended in neither of the two intents: {other:?}"),
    }

    common::drop_restaurant(&database, restaurant_id).await;
}
