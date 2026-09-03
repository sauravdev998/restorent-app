//! Keeping one restaurant's data away from another's.
//!
//! This is the most important file in the suite. Everything else in the product
//! is a feature; this is the promise that signing up costs a restaurant nothing
//! in confidentiality, and it is guarded three times over: a scoped transaction,
//! a row level security policy, and foreign keys that carry the restaurant id.
//!
//! Covers AC-2, AC-3, AC-4, AC-11, and AC-12.

mod common;

use api::domain::ids::{RestaurantId, SessionId};
use api::domain::service::NewOrderLine;
use api::infrastructure::db::repository::{billing, service};

/// AC-3: a transaction scoped to one restaurant reads none of another's rows,
/// on every tenant scoped table.
#[tokio::test]
async fn a_scoped_transaction_reads_none_of_another_restaurants_rows() {
    let database = common::database().await;
    let alpha = RestaurantId::new();
    let beta = RestaurantId::new();

    let mut tx = database.begin_scoped(alpha).await.expect("opening scoped");

    let alpha_fixture = common::seed(&mut tx, alpha).await;

    common::rescope(&mut tx, beta).await;
    common::seed(&mut tx, beta).await;

    common::rescope(&mut tx, alpha).await;

    // Alpha can see exactly one restaurant, and it is its own.
    assert_eq!(
        common::visible_rows(&mut tx, "restaurants").await,
        1,
        "alpha saw more than one restaurant, so the policy is not filtering"
    );
    assert_eq!(
        common::visible_rows(&mut tx, "staff").await,
        3,
        "alpha saw a staff count that is not its own three"
    );
    assert_eq!(
        common::visible_rows(&mut tx, "dishes").await,
        2,
        "alpha saw a dish count that is not its own two"
    );

    // And beta's rows are genuinely there, on the other side of the policy.
    common::rescope(&mut tx, beta).await;
    assert_eq!(
        common::visible_rows(&mut tx, "dishes").await,
        2,
        "beta could not see its own dishes, so the test proved nothing"
    );

    common::rescope(&mut tx, alpha).await;
    assert_eq!(
        alpha_fixture.restaurant_id, alpha,
        "the fixture reported a restaurant it did not seed"
    );
}

/// AC-4: with nothing scoped, every tenant table reads empty.
///
/// The failure direction that matters. `current_restaurant_id()` returns NULL
/// when nothing set it, and `restaurant_id = NULL` matches no rows, so a
/// transaction that forgot to scope itself sees nothing rather than seeing
/// everything.
#[tokio::test]
async fn an_unscoped_transaction_reads_nothing_at_all() {
    let database = common::database().await;
    let alpha = RestaurantId::new();

    let mut tx = database.begin_scoped(alpha).await.expect("opening scoped");
    common::seed(&mut tx, alpha).await;

    common::unscope(&mut tx).await;

    for table in common::TENANT_TABLES {
        assert_eq!(
            common::visible_rows(&mut tx, table).await,
            0,
            "an unscoped transaction could read rows from {table}"
        );
    }
}

/// AC-2 and AC-13: a cross restaurant reference cannot be written even by a
/// correctly scoped transaction.
///
/// This is the guard the other two structurally cannot provide. A scoped
/// transaction is allowed to write its own restaurant's rows, so nothing about
/// scoping stops it writing one that points at somebody else's row. The
/// composite foreign key does.
#[tokio::test]
async fn a_correctly_scoped_transaction_still_cannot_point_at_another_restaurant() {
    let database = common::database().await;
    let alpha = RestaurantId::new();
    let beta = RestaurantId::new();

    let mut tx = database.begin_scoped(alpha).await.expect("opening scoped");
    let a = common::seed(&mut tx, alpha).await;

    common::rescope(&mut tx, beta).await;
    let b = common::seed(&mut tx, beta).await;
    let (_, beta_bill_id) = common::seed_visit_and_bill(&mut tx, beta, b.table_one, b.waiter).await;

    common::rescope(&mut tx, alpha).await;
    let alpha_visit = service::open_visit(&mut tx, a.table_one, a.waiter, Some(2))
        .await
        .expect("opening alpha's visit");
    let (_, alpha_lines) = service::send_round(
        &mut tx,
        alpha_visit.id,
        a.waiter,
        &[NewOrderLine {
            dish_id: a.soup,
            quantity: 1,
            note: None,
        }],
    )
    .await
    .expect("sending alpha's round");

    let alpha_line = alpha_lines.first().expect("the round has one line");

    // Pointing one of alpha's lines at beta's dish. A failed statement aborts
    // the transaction, so each attempt sits inside its own savepoint.
    common::savepoint(&mut tx, "stolen_dish").await;
    let stolen_dish = sqlx::query("UPDATE order_lines SET dish_id = $2 WHERE id = $1")
        .bind(alpha_line.id.as_uuid())
        .bind(b.steak.as_uuid())
        .execute(tx.connection())
        .await;

    assert!(
        stolen_dish.is_err(),
        "alpha attached beta's dish to its own order line"
    );
    common::rollback_to(&mut tx, "stolen_dish").await;

    // Pointing one of alpha's lines at a bill of beta's, which is AC-13's
    // refusal. Beta's bill genuinely exists, so this proves the composite key is
    // doing the work rather than the row simply being absent.
    common::savepoint(&mut tx, "stolen_bill").await;
    let stolen_bill = sqlx::query("UPDATE order_lines SET bill_id = $2 WHERE id = $1")
        .bind(alpha_line.id.as_uuid())
        .bind(beta_bill_id)
        .execute(tx.connection())
        .await;

    assert!(
        stolen_bill.is_err(),
        "alpha put its own order line on beta's bill"
    );
    common::rollback_to(&mut tx, "stolen_bill").await;

    // The transaction is still usable, which confirms both failures were the
    // constraint refusing a write rather than the connection giving up.
    assert_eq!(
        common::visible_rows(&mut tx, "order_lines").await,
        1,
        "alpha lost its own order line somewhere in the refusals"
    );
}

/// AC-11: deleting a restaurant removes everything of its own and nothing of
/// anybody else's, and deactivating one removes nothing at all.
#[tokio::test]
async fn deleting_a_restaurant_is_complete_and_deactivating_one_removes_nothing() {
    let database = common::database().await;
    let alpha = RestaurantId::new();
    let beta = RestaurantId::new();

    let mut tx = database.begin_scoped(alpha).await.expect("opening scoped");
    let a = common::seed(&mut tx, alpha).await;

    // Give alpha a full trail of rows, so the cascade has something to miss.
    let visit = service::open_visit(&mut tx, a.table_one, a.waiter, Some(2))
        .await
        .expect("opening a visit");
    let (_, lines) = service::send_round(
        &mut tx,
        visit.id,
        a.waiter,
        &[NewOrderLine {
            dish_id: a.soup,
            quantity: 2,
            note: Some("no salt".to_owned()),
        }],
    )
    .await
    .expect("sending a round");
    let line = lines.first().expect("the round has one line");
    service::mark_line_ready(&mut tx, line.id, a.chef)
        .await
        .expect("marking ready");
    service::mark_line_served(&mut tx, line.id)
        .await
        .expect("marking served");
    let bill = billing::open_bill(&mut tx, visit.id, a.waiter)
        .await
        .expect("opening a bill");
    billing::assign_lines_to_bill(&mut tx, bill.id, &[line.id])
        .await
        .expect("assigning the line");
    billing::close_bill(&mut tx, bill.id, a.waiter)
        .await
        .expect("closing the bill");

    common::rescope(&mut tx, beta).await;
    common::seed(&mut tx, beta).await;
    common::rescope(&mut tx, alpha).await;

    // Deactivating changes nothing about what is there.
    sqlx::query("UPDATE restaurants SET deactivated_at = now(), updated_at = now()")
        .execute(tx.connection())
        .await
        .expect("deactivating");

    assert_eq!(
        common::visible_rows(&mut tx, "bills").await,
        1,
        "deactivating a restaurant removed one of its bills"
    );
    assert_eq!(
        common::visible_rows(&mut tx, "order_lines").await,
        1,
        "deactivating a restaurant removed one of its order lines"
    );

    // Deleting removes everything of alpha's.
    sqlx::query("DELETE FROM restaurants WHERE id = $1")
        .bind(alpha.as_uuid())
        .execute(tx.connection())
        .await
        .expect("deleting the restaurant");

    for table in common::TENANT_TABLES {
        assert_eq!(
            common::visible_rows(&mut tx, table).await,
            0,
            "deleting the restaurant left rows behind in {table}"
        );
    }

    // And nothing of beta's.
    common::rescope(&mut tx, beta).await;
    assert_eq!(
        common::visible_rows(&mut tx, "restaurants").await,
        1,
        "deleting alpha took beta with it"
    );
    assert_eq!(
        common::visible_rows(&mut tx, "dishes").await,
        2,
        "deleting alpha took beta's dishes with it"
    );
}

/// AC-12: the two sign in lookups work with nothing scoped, and a direct read of
/// the same tables with nothing scoped returns nothing.
///
/// This is the test that would have caught the mistake the design calls out: a
/// `SECURITY DEFINER` function owned by the schema owner is still filtered under
/// `FORCE ROW LEVEL SECURITY`, finds no restaurant set, and returns no rows, so
/// nobody could ever sign in. It is not optional.
///
/// It has to commit, because the lookups run on their own connection outside any
/// transaction a test could hold open, so it deletes the restaurant afterwards.
#[tokio::test]
async fn the_two_sign_in_lookups_read_across_restaurants_and_nothing_else_does() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let token = format!("token-{}", restaurant_id.as_uuid().simple());
    let token_hash = token.as_bytes().to_vec();

    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let fixture = common::seed(&mut tx, restaurant_id).await;

    sqlx::query(
        "INSERT INTO sessions
             (id, restaurant_id, staff_id, token_hash, expires_at, last_seen_at,
              absolute_expires_at, updated_at)
         VALUES ($1, $2, $3, $4, now() + interval '1 hour', now(),
                 now() + interval '90 days', now())",
    )
    .bind(SessionId::new().as_uuid())
    .bind(restaurant_id.as_uuid())
    .bind(fixture.admin.as_uuid())
    .bind(&token_hash)
    .execute(tx.connection())
    .await
    .expect("seeding a session");

    tx.commit().await.expect("committing the fixture");

    // The lookup finds the account with no scope set anywhere, and is not
    // case sensitive about the address.
    let found = database
        .find_staff_for_login(&fixture.admin_email.to_uppercase())
        .await
        .expect("the login lookup ran");
    let found = found.expect(
        "the login lookup found nobody. If the two functions are owned by the schema owner \
         rather than by auth_lookup, FORCE ROW LEVEL SECURITY filters them and nobody can sign in.",
    );
    assert_eq!(found.staff_id, fixture.admin);
    assert_eq!(found.restaurant_id, restaurant_id);

    // An address nobody has is simply absent, not an error.
    let missing = database
        .find_staff_for_login("nobody@example.test")
        .await
        .expect("the login lookup ran");
    assert!(missing.is_none(), "an unknown address matched an account");

    // The session resolves, again with no scope.
    let session = database
        .resolve_session(&token_hash)
        .await
        .expect("the session lookup ran")
        .expect("the session lookup found nothing");
    assert_eq!(session.staff_id, fixture.admin);
    assert_eq!(session.restaurant_id, restaurant_id);

    // A token nobody issued resolves to nothing.
    let unknown = database
        .resolve_session(b"not-a-real-token")
        .await
        .expect("the session lookup ran");
    assert!(unknown.is_none(), "an unknown token resolved to a session");

    // Meanwhile a direct read of those very tables, with no scope, sees nothing.
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    common::unscope(&mut tx).await;
    assert_eq!(
        common::visible_rows(&mut tx, "staff").await,
        0,
        "an unscoped select read the staff table directly"
    );
    assert_eq!(
        common::visible_rows(&mut tx, "sessions").await,
        0,
        "an unscoped select read the sessions table directly"
    );
    drop(tx);

    common::drop_restaurant(&database, restaurant_id).await;
}

/// A revoked or expired session resolves to nothing, which is the other half of
/// what the session lookup promises.
#[tokio::test]
async fn an_expired_or_revoked_session_resolves_to_nothing() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let stem = restaurant_id.as_uuid().simple().to_string();
    let expired = format!("expired-{stem}").into_bytes();
    let revoked = format!("revoked-{stem}").into_bytes();

    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let fixture = common::seed(&mut tx, restaurant_id).await;

    sqlx::query(
        "INSERT INTO sessions
             (id, restaurant_id, staff_id, token_hash, expires_at, last_seen_at,
              absolute_expires_at, updated_at)
         VALUES ($1, $2, $3, $4, now() - interval '1 hour', now(),
                 now() + interval '90 days', now())",
    )
    .bind(SessionId::new().as_uuid())
    .bind(restaurant_id.as_uuid())
    .bind(fixture.admin.as_uuid())
    .bind(&expired)
    .execute(tx.connection())
    .await
    .expect("seeding an expired session");

    sqlx::query(
        "INSERT INTO sessions
             (id, restaurant_id, staff_id, token_hash, expires_at, last_seen_at,
              absolute_expires_at, revoked_at, updated_at)
         VALUES ($1, $2, $3, $4, now() + interval '1 hour', now(),
                 now() + interval '90 days', now(), now())",
    )
    .bind(SessionId::new().as_uuid())
    .bind(restaurant_id.as_uuid())
    .bind(fixture.admin.as_uuid())
    .bind(&revoked)
    .execute(tx.connection())
    .await
    .expect("seeding a revoked session");

    tx.commit().await.expect("committing the fixture");

    assert!(
        database
            .resolve_session(&expired)
            .await
            .expect("the session lookup ran")
            .is_none(),
        "an expired session still resolved"
    );
    assert!(
        database
            .resolve_session(&revoked)
            .await
            .expect("the session lookup ran")
            .is_none(),
        "a revoked session still resolved"
    );

    common::drop_restaurant(&database, restaurant_id).await;
}
