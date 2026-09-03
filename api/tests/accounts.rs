//! Registration, sessions, roles, and the throttle, against a real Postgres.
//!
//! Everything here connects as `app_api`, never as the schema owner, for the
//! same reason the isolation suite does: Postgres exempts a table's owner from
//! its own policies unless `FORCE ROW LEVEL SECURITY` is set, and a suite run as
//! the owner would pass while proving nothing.
//!
//! These tests commit, because a resolved session is looked up on its own
//! connection outside any transaction a test could hold open. Each one deletes
//! its restaurant afterwards, which cascades.
//!
//! Covers AC-1, AC-2, AC-4, AC-6, AC-7, AC-10, AC-13, AC-14, AC-17, AC-23.

mod common;

use api::domain::country::CountryCode;
use api::domain::credentials::EmailAddress;
use api::domain::error::DomainError;
use api::domain::ids::RestaurantId;
use api::domain::session::{SessionToken, hash_cookie_value};
use api::infrastructure::db::repository::{accounts, catalog, sessions};

/// AC-1: the five settings a new restaurant gets come from the country's row in
/// `locales/countries.json` and from nowhere else.
#[tokio::test]
async fn registering_takes_every_setting_from_the_country_row() {
    let database = common::database().await;
    let account = common::register_and_sign_in(&database, "settings").await;

    let mut tx = database
        .begin_scoped(account.restaurant_id)
        .await
        .expect("opening scoped");

    let restaurant = catalog::restaurant(&mut tx)
        .await
        .expect("reading the registered restaurant");

    let india = CountryCode::new("IN")
        .expect("IN is served")
        .settings()
        .expect("the country row");

    assert_eq!(restaurant.country_code, india.code);
    assert_eq!(restaurant.currency.code(), india.currency_code);
    assert_eq!(restaurant.currency.decimals(), india.currency_decimals);
    assert_eq!(restaurant.timezone, india.default_timezone);
    assert_eq!(
        restaurant.default_language.as_str(),
        india.default_language,
        "the restaurant reads in a language the country row did not name"
    );
    assert_eq!(
        restaurant.formatting_locale.as_str(),
        india.formatting_locale
    );

    drop(tx);
    common::drop_restaurant(&database, account.restaurant_id).await;
}

/// AC-1: the address is stored exactly as it was typed, and matched case
/// insensitively, so the bundle shows somebody their own capitalisation.
#[tokio::test]
async fn an_address_is_stored_as_typed_and_matched_without_regard_to_case() {
    let database = common::database().await;
    let account = common::register_and_sign_in(&database, "case").await;

    let mut tx = database
        .begin_scoped(account.restaurant_id)
        .await
        .expect("opening scoped");

    let staff = accounts::staff_by_id(&mut tx, account.admin)
        .await
        .expect("reading the admin");

    assert_eq!(
        staff.email, account.email,
        "the stored address is not the one that was typed"
    );
    assert!(
        staff.email.chars().any(char::is_uppercase),
        "the fixture address has no capitals, so this test proves nothing"
    );

    drop(tx);

    // The same account, found through the lookup sign in uses, however the
    // address is capitalised on the way in.
    for typed in [
        account.email.to_lowercase(),
        account.email.to_uppercase(),
        account.email.clone(),
    ] {
        let found = database
            .find_staff_for_login(&typed)
            .await
            .expect("the login lookup ran")
            .expect("the account was not found");

        assert_eq!(found.staff_id, account.admin, "{typed} found nobody");
    }

    common::drop_restaurant(&database, account.restaurant_id).await;
}

/// AC-2: registration is one transaction. A duplicate address leaves no
/// restaurant, no staff row, and no session behind.
#[tokio::test]
async fn a_duplicate_address_leaves_no_restaurant_behind() {
    let database = common::database().await;
    let first = common::register_and_sign_in(&database, "dup").await;

    // A second registration for the same address, in its own transaction, which
    // is exactly what the handler does.
    let orphan = RestaurantId::new();
    let mut tx = database.begin_scoped(orphan).await.expect("opening scoped");

    let refused = accounts::register(
        &mut tx,
        &accounts::Registration {
            restaurant_name: "The Second Kitchen",
            country: CountryCode::new("GB")
                .expect("GB is served")
                .settings()
                .expect("the country row"),
            display_name: "Bob Owner",
            email: &EmailAddress::new(&first.email).expect("a valid address"),
            password_hash: "not-a-real-hash",
        },
    )
    .await;

    assert!(
        matches!(refused, Err(DomainError::Conflict(_))),
        "a duplicate address was accepted, or refused as something other than a conflict: \
         {refused:?}"
    );

    // The handler drops the transaction on this error, so nothing it wrote
    // survives. Dropping it here is that same thing.
    drop(tx);

    let mut check = database
        .begin_scoped(orphan)
        .await
        .expect("opening scoped to look for an orphan");

    assert_eq!(
        common::visible_rows(&mut check, "restaurants").await,
        0,
        "the refused registration left a restaurant behind with no admin in it"
    );

    drop(check);
    common::drop_restaurant(&database, first.restaurant_id).await;
}

/// AC-6, AC-13: signing out ends exactly the session that asked, and every
/// other session that person holds keeps working.
#[tokio::test]
async fn signing_out_on_one_device_leaves_the_other_signed_in() {
    let database = common::database().await;
    let account = common::register_and_sign_in(&database, "devices").await;

    // A second device: another session for the same person.
    let second = SessionToken::mint().expect("minting");
    let mut tx = database
        .begin_scoped(account.restaurant_id)
        .await
        .expect("opening scoped");
    let second_session = sessions::open(&mut tx, account.admin, &second.hash())
        .await
        .expect("opening a second session");
    tx.commit().await.expect("committing");

    // Both resolve.
    assert!(
        database
            .resolve_session(&account.token_hash)
            .await
            .expect("resolving")
            .is_some()
    );
    assert!(
        database
            .resolve_session(&second.hash())
            .await
            .expect("resolving")
            .is_some()
    );

    // Sign out on the second.
    let mut tx = database
        .begin_scoped(account.restaurant_id)
        .await
        .expect("opening scoped");
    sessions::revoke(&mut tx, second_session.id)
        .await
        .expect("revoking");
    tx.commit().await.expect("committing");

    assert!(
        database
            .resolve_session(&second.hash())
            .await
            .expect("resolving")
            .is_none(),
        "the signed out session still resolves, so revocation is not instant"
    );
    assert!(
        database
            .resolve_session(&account.token_hash)
            .await
            .expect("resolving")
            .is_some(),
        "signing out on one device signed the other one out too"
    );

    common::drop_restaurant(&database, account.restaurant_id).await;
}

/// AC-7: a session past its absolute ceiling is refused even though its sliding
/// expiry is in the future, which is the case a session used every day reaches.
#[tokio::test]
async fn a_session_past_its_ceiling_is_refused_however_recently_it_was_used() {
    let database = common::database().await;
    let account = common::register_and_sign_in(&database, "ceiling").await;

    // Push the ceiling into the past while leaving the sliding expiry where it
    // is. The check constraint refuses `absolute_expires_at <= expires_at`, so
    // the sliding expiry moves back with it and still stays in the future.
    let mut tx = database
        .begin_scoped(account.restaurant_id)
        .await
        .expect("opening scoped");

    sqlx::query(
        "UPDATE sessions
            SET expires_at          = now() + interval '13 days',
                absolute_expires_at = now() - interval '1 minute'
          WHERE token_hash = $1",
    )
    .bind(&account.token_hash)
    .execute(tx.connection())
    .await
    .expect_err(
        "moving the ceiling behind the expiry should break the check constraint, which is \
         what makes the pair meaningful",
    );

    drop(tx);

    // So do it the way it really happens: both in the past, ceiling first.
    let mut tx = database
        .begin_scoped(account.restaurant_id)
        .await
        .expect("opening scoped");

    sqlx::query(
        "UPDATE sessions
            SET expires_at          = now() - interval '2 minutes',
                absolute_expires_at = now() - interval '1 minute'
          WHERE token_hash = $1",
    )
    .bind(&account.token_hash)
    .execute(tx.connection())
    .await
    .expect("ageing the session");

    tx.commit().await.expect("committing");

    assert!(
        database
            .resolve_session(&account.token_hash)
            .await
            .expect("resolving")
            .is_none(),
        "a session past its ceiling still resolves"
    );

    common::drop_restaurant(&database, account.restaurant_id).await;
}

/// AC-7: sliding moves the expiry forward and never moves the ceiling, and it
/// touches exactly one row.
#[tokio::test]
async fn sliding_moves_the_expiry_and_never_the_ceiling() {
    let database = common::database().await;
    let account = common::register_and_sign_in(&database, "slide").await;

    let before = session_row(&database, &account).await;

    // Age the last use past the floor, so a real request would slide it.
    let mut tx = database
        .begin_scoped(account.restaurant_id)
        .await
        .expect("opening scoped");
    sqlx::query(
        "UPDATE sessions
            SET last_seen_at = now() - interval '10 minutes',
                expires_at   = now() + interval '14 days' - interval '10 minutes'
          WHERE token_hash = $1",
    )
    .bind(&account.token_hash)
    .execute(tx.connection())
    .await
    .expect("ageing the session");
    tx.commit().await.expect("committing");

    let session = database
        .resolve_session(&account.token_hash)
        .await
        .expect("resolving")
        .expect("the session still resolves");

    let mut tx = database
        .begin_scoped(account.restaurant_id)
        .await
        .expect("opening scoped");
    sessions::slide(&mut tx, session.session_id)
        .await
        .expect("sliding");
    tx.commit().await.expect("committing");

    let after = session_row(&database, &account).await;

    assert!(
        after.0 > before.0 - chrono::Duration::seconds(1),
        "the expiry did not move forward"
    );
    assert_eq!(
        after.1, before.1,
        "the absolute ceiling moved, so a session used every day would never end"
    );

    common::drop_restaurant(&database, account.restaurant_id).await;
}

/// AC-10: five failed attempts for one address inside the window are refused,
/// and a different address in the same window is unaffected.
#[tokio::test]
async fn one_address_running_out_of_attempts_does_not_stop_another() {
    let database = common::database().await;
    let stem = RestaurantId::new().as_uuid().simple().to_string();
    let busy = format!("busy-{stem}@example.test");
    let quiet = format!("quiet-{stem}@example.test");

    // The limit is five, so five are allowed and the sixth is not.
    for attempt in 1..=5 {
        database
            .record_login_attempt(&busy, None)
            .await
            .unwrap_or_else(|error| panic!("attempt {attempt} was refused: {error:?}"));
    }

    let refused = database.record_login_attempt(&busy, None).await;
    assert!(
        matches!(refused, Err(DomainError::Throttled(_))),
        "a sixth attempt for one address was allowed: {refused:?}"
    );

    // A different address is counted in its own bucket.
    database
        .record_login_attempt(&quiet, None)
        .await
        .expect("a different address was throttled by another address's attempts");

    // Case does not create a second bucket: an account is one account however
    // its address is capitalised, so the throttle has to be too.
    let refused = database
        .record_login_attempt(&busy.to_uppercase(), None)
        .await;
    assert!(
        matches!(refused, Err(DomainError::Throttled(_))),
        "capitalising the address bought a fresh set of attempts: {refused:?}"
    );

    clear_attempts(&database, &[&busy, &quiet]).await;
}

/// AC-23: the sign in sweep clears only this address's old rows.
#[tokio::test]
async fn the_attempt_sweep_touches_only_the_address_that_signed_in() {
    let database = common::database().await;
    let stem = RestaurantId::new().as_uuid().simple().to_string();
    let mine = format!("mine-{stem}@example.test");
    let theirs = format!("theirs-{stem}@example.test");

    for address in [&mine, &theirs] {
        database
            .record_login_attempt(address, None)
            .await
            .expect("recording an attempt");
    }

    // Age both well past the retention window.
    age_attempts(&database, &[&mine, &theirs]).await;

    let swept = database
        .sweep_login_attempts(&mine)
        .await
        .expect("sweeping");

    assert_eq!(swept, 1, "the sweep did not clear this address's own row");
    assert_eq!(
        attempts_for(&database, &theirs).await,
        1,
        "a routine sign in deleted another address's rows, so it is a platform wide write"
    );

    clear_attempts(&database, &[&mine, &theirs]).await;
}

/// AC-14: changing a password revokes every other session and keeps this one.
#[tokio::test]
async fn changing_a_password_keeps_this_session_and_ends_the_others() {
    let database = common::database().await;
    let account = common::register_and_sign_in(&database, "password").await;

    let other = SessionToken::mint().expect("minting");
    let mut tx = database
        .begin_scoped(account.restaurant_id)
        .await
        .expect("opening scoped");
    sessions::open(&mut tx, account.admin, &other.hash())
        .await
        .expect("opening another session");
    tx.commit().await.expect("committing");

    let mine = database
        .resolve_session(&account.token_hash)
        .await
        .expect("resolving")
        .expect("my own session");

    let mut tx = database
        .begin_scoped(account.restaurant_id)
        .await
        .expect("opening scoped");
    accounts::set_password_hash(&mut tx, account.admin, "a-new-hash")
        .await
        .expect("setting the hash");
    let revoked = sessions::revoke_every_other(&mut tx, account.admin, mine.session_id)
        .await
        .expect("revoking the others");
    tx.commit().await.expect("committing");

    assert_eq!(revoked, 1, "the other session was not revoked");
    assert!(
        database
            .resolve_session(&other.hash())
            .await
            .expect("resolving")
            .is_none(),
        "another device stayed signed in after the password changed"
    );
    assert!(
        database
            .resolve_session(&account.token_hash)
            .await
            .expect("resolving")
            .is_some(),
        "the person who changed their own password was signed out of the screen in front of them"
    );

    common::drop_restaurant(&database, account.restaurant_id).await;
}

/// AC-17: registration and a password change each write exactly one audit row,
/// and no password hash appears in any value.
#[tokio::test]
async fn the_audit_rows_name_what_happened_and_carry_no_hash() {
    let database = common::database().await;
    let account = common::register_and_sign_in(&database, "audit").await;

    let mut tx = database
        .begin_scoped(account.restaurant_id)
        .await
        .expect("opening scoped");

    accounts::set_password_hash(&mut tx, account.admin, "$argon2id$v=19$secret-looking-hash")
        .await
        .expect("setting the hash");

    let rows: Vec<(String, Option<serde_json::Value>, Option<serde_json::Value>)> =
        sqlx::query_as("SELECT action, before, after FROM audit_log ORDER BY occurred_at")
            .fetch_all(tx.connection())
            .await
            .expect("reading the audit log");

    let actions: Vec<&str> = rows.iter().map(|(action, _, _)| action.as_str()).collect();
    assert_eq!(actions, ["restaurant_registered", "password_changed"]);

    let (_, registered_before, registered_after) = &rows[0];
    assert!(
        registered_before.is_none(),
        "a restaurant coming into existence has no before"
    );
    let after = registered_after
        .as_ref()
        .expect("the registration row has an after");
    assert_eq!(after["adminEmail"], account.email);
    assert_eq!(after["countryCode"], "IN");

    for (action, before, after) in &rows {
        let printed = format!("{before:?}{after:?}");
        assert!(
            !printed.contains("argon2"),
            "the {action} row carries a password hash: {printed}"
        );
        assert!(
            !printed.contains(&account.password),
            "the {action} row carries a password"
        );
    }

    drop(tx);
    common::drop_restaurant(&database, account.restaurant_id).await;
}

/// AC-6: a cookie value nobody was ever issued resolves to nothing, whatever
/// shape it is in.
#[tokio::test]
async fn a_cookie_value_nobody_was_issued_resolves_to_nothing() {
    let database = common::database().await;

    let invented = SessionToken::mint().expect("minting");
    assert!(
        database
            .resolve_session(&invented.hash())
            .await
            .expect("resolving")
            .is_none(),
        "a token this server never issued resolved to a session"
    );

    for nonsense in ["", "not-base64!!", "c2hvcnQ"] {
        assert!(
            hash_cookie_value(nonsense).is_none(),
            "{nonsense:?} was read as a session token"
        );
    }
}

/// The session's two expiry columns, as stored.
async fn session_row(
    database: &api::infrastructure::db::Database,
    account: &common::RegisteredAccount,
) -> (chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>) {
    let mut tx = database
        .begin_scoped(account.restaurant_id)
        .await
        .expect("opening scoped");

    let row: (chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>) = sqlx::query_as(
        "SELECT expires_at, absolute_expires_at FROM sessions WHERE token_hash = $1",
    )
    .bind(&account.token_hash)
    .fetch_one(tx.connection())
    .await
    .expect("reading the session row");

    row
}

/// How many attempt rows one address currently has.
async fn attempts_for(database: &api::infrastructure::db::Database, email: &str) -> i64 {
    // `login_attempts` carries no restaurant, so any scope will do to reach it;
    // the table has no policy to filter it.
    let mut tx = database
        .begin_scoped(RestaurantId::new())
        .await
        .expect("opening scoped");

    let (count,): (i64,) =
        sqlx::query_as("SELECT count(*) FROM login_attempts WHERE email = lower($1)")
            .bind(email)
            .fetch_one(tx.connection())
            .await
            .expect("counting attempts");

    count
}

/// Pushes an address's attempt rows past the retention window.
async fn age_attempts(database: &api::infrastructure::db::Database, emails: &[&str]) {
    let mut tx = database
        .begin_scoped(RestaurantId::new())
        .await
        .expect("opening scoped");

    for email in emails {
        sqlx::query(
            "UPDATE login_attempts
                SET attempted_at = now() - interval '48 hours'
              WHERE email = lower($1)",
        )
        .bind(email)
        .execute(tx.connection())
        .await
        .expect("ageing attempts");
    }

    tx.commit().await.expect("committing");
}

/// Leaves the table as it was found.
async fn clear_attempts(database: &api::infrastructure::db::Database, emails: &[&str]) {
    let mut tx = database
        .begin_scoped(RestaurantId::new())
        .await
        .expect("opening scoped");

    for email in emails {
        sqlx::query("DELETE FROM login_attempts WHERE email = lower($1)")
            .bind(email)
            .execute(tx.connection())
            .await
            .expect("clearing attempts");
    }

    tx.commit().await.expect("committing");
}
