//! The language and formatting settings, against a real database.
//!
//! This feature ships no endpoint, which is a decision rather than an omission:
//! there is no session yet to hang a "save my preference" request on, so feature
//! 7 owns the write path. That makes the repository functions the outermost
//! thing there is to test, and these call them directly.
//!
//! Every test runs inside a transaction that is never committed, as `app_api`
//! rather than as the schema owner, so the row level security policies actually
//! apply. See `common/mod.rs`.
//!
//! Covers AC-15, and the storage half of AC-6 and AC-12.

mod common;

use api::domain::ids::RestaurantId;
use api::domain::language::{FormattingLocale, LanguageCode, catalogue};
use api::infrastructure::db::repository::catalog;

/// AC-15: the columns exist with the defaults the catalogue names, so a
/// restaurant nobody has configured is still readable and still valid.
///
/// The migration cannot read `locales/catalogue.json`, so its column defaults
/// are written by hand. This is what stops the two drifting: a fresh row has to
/// come back as values the catalogue actually offers.
#[tokio::test]
async fn a_fresh_restaurant_takes_the_catalogue_defaults() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();

    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    common::seed(&mut tx, restaurant_id).await;

    let restaurant = catalog::restaurant(&mut tx)
        .await
        .expect("reading a seeded restaurant");
    let defaults = &catalogue().expect("the catalogue parses").defaults;

    assert_eq!(
        restaurant.default_language.as_str(),
        defaults.language,
        "migration 0003's default_language does not match the catalogue's default"
    );
    assert_eq!(
        restaurant.formatting_locale.as_str(),
        defaults.formatting_locale,
        "migration 0003's formatting_locale does not match the catalogue's default"
    );
}

/// AC-6, AC-12: the two settings are stored and read back independently.
///
/// Deliberately a language and a formatting locale that do not match each other.
/// An owner reading Hindi whose figures are written the way an accountant in
/// India expects is the exact case these two columns exist to allow, and a
/// system that quietly derived one from the other would fail this.
#[tokio::test]
async fn the_language_and_the_formatting_locale_are_stored_separately() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();

    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    common::seed(&mut tx, restaurant_id).await;

    let language = LanguageCode::new("hi").expect("hi is in the catalogue");
    let locale = FormattingLocale::new("en-IN").expect("en-IN is in the catalogue");

    catalog::set_restaurant_languages(&mut tx, &language, &locale)
        .await
        .expect("setting the restaurant's languages");

    let restaurant = catalog::restaurant(&mut tx).await.expect("reading it back");

    assert_eq!(restaurant.default_language.as_str(), "hi");
    assert_eq!(restaurant.formatting_locale.as_str(), "en-IN");
}

/// AC-6: a staff member's personal language is stored, read back, and can be
/// cleared back to "whatever the restaurant uses".
///
/// Clearing matters as much as setting. It is a real choice somebody makes, and
/// it has to be expressible rather than only reachable by never having chosen.
#[tokio::test]
async fn a_staff_members_own_language_is_stored_and_can_be_cleared() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();

    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let fixture = common::seed(&mut tx, restaurant_id).await;

    // Nobody starts with one. A staff member created by feature 10 needs no
    // value, which is why the column is nullable.
    let before = catalog::active_staff(&mut tx).await.expect("reading staff");
    assert!(
        before.iter().all(|member| member.language.is_none()),
        "a seeded staff member should have no personal language"
    );

    let hindi = LanguageCode::new("hi").expect("hi is in the catalogue");
    catalog::set_staff_language(&mut tx, fixture.waiter, Some(&hindi))
        .await
        .expect("setting the waiter's language");

    let after = catalog::active_staff(&mut tx).await.expect("reading staff");
    let waiter = after
        .iter()
        .find(|member| member.id == fixture.waiter)
        .expect("the waiter is still active");
    assert_eq!(
        waiter.language.as_ref().map(LanguageCode::as_str),
        Some("hi")
    );

    // And nobody else was touched by a write naming one row.
    assert_eq!(
        after
            .iter()
            .filter(|member| member.language.is_some())
            .count(),
        1,
        "setting one person's language changed somebody else's"
    );

    catalog::set_staff_language(&mut tx, fixture.waiter, None)
        .await
        .expect("clearing the waiter's language");

    let cleared = catalog::active_staff(&mut tx).await.expect("reading staff");
    let waiter = cleared
        .iter()
        .find(|member| member.id == fixture.waiter)
        .expect("the waiter is still active");
    assert!(
        waiter.language.is_none(),
        "clearing should put them back on the restaurant's default"
    );
}

/// AC-15: a code outside the catalogue is refused, and is refused before any
/// statement is prepared.
///
/// The refusal lives in the constructor rather than in the repository, and that
/// is the design: the repository takes the validated types, so there is no way
/// to reach a statement with an unknown code at all. It is a `DomainError` that
/// `presentation/error.rs` maps to a `400`, so feature 7's endpoint inherits the
/// refusal without writing it again.
#[tokio::test]
async fn a_code_outside_the_catalogue_never_reaches_a_statement() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();

    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    common::seed(&mut tx, restaurant_id).await;

    // There is no way to build the argument, so there is no call to make.
    LanguageCode::new("xx").expect_err("an uncatalogued language should be refused");
    LanguageCode::new("en-US").expect_err("a formatting locale is not a language");
    FormattingLocale::new("fr-FR").expect_err("an unlisted formatting locale should be refused");
    FormattingLocale::new("hi").expect_err("a bare language is not a formatting locale");

    // And the row is untouched, because none of the above could be called.
    let restaurant = catalog::restaurant(&mut tx).await.expect("reading it back");
    assert_eq!(restaurant.default_language.as_str(), "en");
}

/// AC-15 with the tenant rule from spec 0003: a language write cannot reach
/// another restaurant's staff row.
///
/// Nothing new was written to make this true. The scoped transaction and the
/// policy on `staff` already say it, and adding a column to a table changes
/// neither. This is the test that says so out loud, so a later change that
/// weakens it fails here.
#[tokio::test]
async fn a_language_write_cannot_cross_a_restaurant_boundary() {
    let database = common::database().await;
    let alpha = RestaurantId::new();
    let beta = RestaurantId::new();

    let mut tx = database.begin_scoped(alpha).await.expect("opening scoped");
    common::seed(&mut tx, alpha).await;

    common::rescope(&mut tx, beta).await;
    let beta_fixture = common::seed(&mut tx, beta).await;

    // Back in alpha's scope, holding a real staff id that belongs to beta.
    common::rescope(&mut tx, alpha).await;

    let hindi = LanguageCode::new("hi").expect("hi is in the catalogue");
    let refused = catalog::set_staff_language(&mut tx, beta_fixture.waiter, Some(&hindi)).await;

    assert!(
        refused.is_err(),
        "alpha wrote a language onto beta's staff row"
    );

    // Beta's row really is unchanged, checked from beta's own scope rather than
    // inferred from the error.
    common::rescope(&mut tx, beta).await;
    let beta_staff = catalog::active_staff(&mut tx).await.expect("reading staff");
    assert!(
        beta_staff.iter().all(|member| member.language.is_none()),
        "beta's staff language was changed from outside beta"
    );
}
