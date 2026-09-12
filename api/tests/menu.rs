//! Menu management, spec 0008, at the layer the handlers sit on.
//!
//! Every test runs against a real Postgres as `app_api`, so row level security
//! applies exactly as it does to the API. All but the race roll back; the race
//! has to commit, because two transactions only see each other's work once it
//! has, and it deletes its restaurant at the end.
//!
//! What these prove is the rules rather than the plumbing: a stale edit is
//! refused and the kitchen's switch survives it, a reorder is all or nothing, no
//! live dish ever sits under an archived heading, names are unique the way a
//! reader of a menu means it, a line already sent never changes, and every
//! consequential change leaves exactly one audit row.
//!
//! Covers AC-1, AC-2, AC-3, AC-4, AC-5, AC-6, AC-7, AC-8, AC-9, AC-11, AC-12,
//! AC-14, AC-15, AC-16, and AC-17.

mod common;

use std::collections::BTreeMap;
use std::time::Duration;

use uuid::Uuid;

use api::domain::enums::Diet;
use api::domain::error::{ConflictKind, DomainError};
use api::domain::ids::{DishId, MenuCategoryId, RestaurantId, StaffId};
use api::domain::service::NewOrderLine;
use api::infrastructure::db::ScopedTx;
use api::infrastructure::db::repository::catalog::{self, DishEdit, NewDish};
use api::infrastructure::db::repository::{billing, service};

/// The conflict a refusal carries, or a panic naming what came back instead.
fn conflict_of<T: std::fmt::Debug>(outcome: Result<T, DomainError>, what: &str) -> ConflictKind {
    match outcome {
        Err(DomainError::Conflict(kind)) => kind,
        other => panic!("{what} came back as {other:?} rather than a conflict"),
    }
}

/// Adds a dish the way the admin screen does.
async fn add_dish(
    tx: &mut ScopedTx<'_>,
    category_id: MenuCategoryId,
    name: &str,
    price: &str,
    actor: StaffId,
) -> api::domain::catalog::Dish {
    catalog::create_dish(
        tx,
        &NewDish {
            category_id,
            name,
            description: None,
            price: common::money(price),
            diet: Diet::Veg,
        },
        actor,
    )
    .await
    .unwrap_or_else(|error| panic!("adding {name}: {error:?}"))
}

/// An edit that keeps everything about the dish but what the caller changes.
fn edit_of(dish: &api::domain::catalog::Dish) -> DishEdit {
    DishEdit {
        category_id: dish.category_id,
        name: dish.name.clone(),
        description: dish.description.clone(),
        price: dish.price,
        diet: dish.diet,
        version: dish.version,
    }
}

/// How many audit rows of each action this restaurant has.
async fn audit_counts(tx: &mut ScopedTx<'_>) -> BTreeMap<String, i64> {
    let rows: Vec<(String, i64)> =
        sqlx::query_as("SELECT action, count(*) FROM audit_log GROUP BY action")
            .fetch_all(tx.connection())
            .await
            .expect("counting audit rows");

    rows.into_iter().collect()
}

/// The live dishes of one category, in printed order, by name.
async fn names_in(tx: &mut ScopedTx<'_>, category_id: MenuCategoryId) -> Vec<String> {
    catalog::live_dishes(tx)
        .await
        .expect("reading the menu")
        .into_iter()
        .filter(|dish| dish.category_id == category_id)
        .map(|dish| dish.name)
        .collect()
}

/// Turns a list of expected audit counts into the shape [`audit_counts`] reads.
fn counts(pairs: &[(&str, i64)]) -> BTreeMap<String, i64> {
    pairs
        .iter()
        .map(|(action, count)| ((*action).to_owned(), *count))
        .collect()
}

/// AC-1, AC-2, AC-3, AC-17: categories and dishes land at the end of their
/// lists, an edit keeps a dish in place, and a move lands at the end of the
/// new category, each with its version bumped and one audit row.
#[tokio::test]
async fn a_menu_is_built_edited_and_moved() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut tx, restaurant_id).await;

    // Two categories, each at the end of the list after the fixture's Mains.
    let starters = catalog::create_menu_category(&mut tx, "  Starters ", f.admin)
        .await
        .expect("adding Starters");
    let breads = catalog::create_menu_category(&mut tx, "Breads", f.admin)
        .await
        .expect("adding Breads");

    assert_eq!(starters.name, "Starters", "the name was not trimmed");
    assert_eq!((starters.position, breads.position), (2, 3));
    assert_eq!(starters.version, 1);

    // Three dishes, each at the end of its category and each available.
    let pakora = add_dish(&mut tx, starters.id, "Pakora", "90", f.admin).await;
    let tikka = add_dish(&mut tx, starters.id, "Paneer tikka", "320", f.admin).await;
    let naan = add_dish(&mut tx, breads.id, "Naan", "40", f.admin).await;

    assert_eq!((pakora.position, tikka.position, naan.position), (1, 2, 1));
    assert!(pakora.is_available && tikka.is_available && naan.is_available);
    assert_eq!(pakora.version, 1);

    // A price edit keeps the dish where it is and bumps its version.
    let repriced = catalog::update_dish(
        &mut tx,
        tikka.id,
        &DishEdit {
            price: common::money("340"),
            ..edit_of(&tikka)
        },
        f.admin,
    )
    .await
    .expect("repricing the tikka");
    assert_eq!(repriced.position, tikka.position);
    assert_eq!(repriced.version, 2);

    // A move lands at the end of the new category.
    let moved = catalog::update_dish(
        &mut tx,
        tikka.id,
        &DishEdit {
            category_id: breads.id,
            ..edit_of(&repriced)
        },
        f.admin,
    )
    .await
    .expect("moving the tikka");
    assert_eq!(moved.category_id, breads.id);
    assert_eq!(moved.position, 2, "a moved dish did not land at the end");
    assert_eq!(moved.version, 3);
    assert_eq!(names_in(&mut tx, breads.id).await, ["Naan", "Paneer tikka"]);

    assert_eq!(
        audit_counts(&mut tx).await,
        counts(&[
            ("menu_category_created", 2),
            ("dish_created", 3),
            ("dish_edited", 2)
        ]),
        "the audit log does not match the changes"
    );
}

/// AC-4, AC-6, AC-8, AC-17: a reorder rewrites positions and nothing else, and
/// a removed dish and category come back at the end of their lists as they
/// were. A reorder writes no audit row; everything else writes one.
#[tokio::test]
async fn a_menu_is_ordered_removed_and_restored() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut tx, restaurant_id).await;

    let starters = catalog::create_menu_category(&mut tx, "Starters", f.admin)
        .await
        .expect("adding Starters");
    let pakora = add_dish(&mut tx, starters.id, "Pakora", "90", f.admin).await;

    let categories =
        catalog::reorder_menu_categories(&mut tx, &[starters.id.as_uuid(), f.category.as_uuid()])
            .await
            .expect("reordering the categories");
    assert_eq!(
        categories
            .iter()
            .map(|category| (category.name.as_str(), category.position))
            .collect::<Vec<_>>(),
        [("Starters", 1), ("Mains", 2)]
    );
    assert!(
        categories.iter().all(|category| category.version == 1),
        "a reorder bumped a version, so it would make an open rename form stale"
    );

    let dishes =
        catalog::reorder_dishes(&mut tx, f.category, &[f.steak.as_uuid(), f.soup.as_uuid()])
            .await
            .expect("reordering the mains");
    assert_eq!(
        dishes
            .iter()
            .map(|dish| (dish.name.as_str(), dish.position, dish.version))
            .collect::<Vec<_>>(),
        [("Steak", 1, 1), ("Soup", 2, 1)]
    );

    // Removing the pakora empties Starters, which can then go too.
    let archived = catalog::archive_dish(&mut tx, pakora.id, f.admin)
        .await
        .expect("removing the pakora");
    assert!(archived.archived_at.is_some());
    assert_eq!(archived.version, 2);
    catalog::archive_menu_category(&mut tx, starters.id, f.admin)
        .await
        .expect("removing Starters");

    // Both come back, each at the end of its list, as they were.
    let restored = catalog::restore_menu_category(&mut tx, starters.id, f.admin)
        .await
        .expect("putting Starters back");
    assert_eq!(
        restored.position, 3,
        "a restored category did not land at the end"
    );

    let pakora_back = catalog::restore_dish(&mut tx, pakora.id, starters.id, f.admin)
        .await
        .expect("putting the pakora back");
    assert_eq!(pakora_back.price, common::money("90"));
    assert!(
        pakora_back.is_available,
        "a restore changed the availability"
    );
    assert!(pakora_back.archived_at.is_none());

    assert_eq!(
        audit_counts(&mut tx).await,
        counts(&[
            ("menu_category_created", 1),
            ("dish_created", 1),
            ("dish_archived", 1),
            ("menu_category_archived", 1),
            ("menu_category_restored", 1),
            ("dish_restored", 1),
        ]),
        "the audit log does not match the changes"
    );
}

/// AC-17: an edit records the category and the diet in its before and after,
/// and says who made it.
#[tokio::test]
async fn an_edit_records_what_the_dish_was_and_what_it_became() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut tx, restaurant_id).await;

    let soup = catalog::dish(&mut tx, f.soup)
        .await
        .expect("reading the soup");
    catalog::update_dish(
        &mut tx,
        f.soup,
        &DishEdit {
            diet: Diet::Veg,
            price: common::money("11"),
            ..edit_of(&soup)
        },
        f.admin,
    )
    .await
    .expect("editing the soup");

    let (actor, before, after): (Option<Uuid>, serde_json::Value, serde_json::Value) =
        sqlx::query_as(
            "SELECT actor_staff_id, before, after FROM audit_log WHERE action = 'dish_edited'",
        )
        .fetch_one(tx.connection())
        .await
        .expect("reading the edit row");

    assert_eq!(actor, Some(f.admin.as_uuid()));
    assert_eq!(before["diet"], "non_veg");
    assert_eq!(after["diet"], "veg");
    assert_eq!(before["category_id"], after["category_id"]);
    assert_eq!(before["price"], "9.5000");
    assert_eq!(after["price"], "11");
}

/// AC-9, AC-15: the kitchen switches a dish off while an admin has its edit
/// form open, and the form's save is refused rather than switching it back on.
#[tokio::test]
async fn an_edit_from_before_the_kitchen_switched_a_dish_off_is_refused() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut tx, restaurant_id).await;

    // The admin's form loads the soup at version 1.
    let loaded = catalog::dish(&mut tx, f.soup)
        .await
        .expect("loading the form");

    let switched = catalog::set_dish_availability(&mut tx, f.soup, false, f.chef)
        .await
        .expect("the chef switches the soup off");
    assert_eq!(switched.version, loaded.version + 1);

    let stale = catalog::update_dish(
        &mut tx,
        f.soup,
        &DishEdit {
            price: common::money("12"),
            ..edit_of(&loaded)
        },
        f.admin,
    )
    .await;
    assert_eq!(
        conflict_of(stale, "a stale edit"),
        ConflictKind::DishChanged
    );

    let now = catalog::dish(&mut tx, f.soup)
        .await
        .expect("rereading the soup");
    assert!(!now.is_available, "a stale edit switched the soup back on");
    assert_eq!(
        now.price,
        common::money("9.5000"),
        "a stale edit was written"
    );
}

/// AC-15: a rename made from a form older than the stored category is refused.
#[tokio::test]
async fn a_rename_from_a_stale_form_is_refused() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut tx, restaurant_id).await;

    let first = catalog::rename_menu_category(&mut tx, f.category, "Main courses", 1, f.admin)
        .await
        .expect("the first rename");
    assert_eq!(first.version, 2);

    let second = catalog::rename_menu_category(&mut tx, f.category, "Big plates", 1, f.admin).await;
    assert_eq!(
        conflict_of(second, "a stale rename"),
        ConflictKind::CategoryChanged
    );

    let unknown =
        catalog::rename_menu_category(&mut tx, MenuCategoryId::new(), "Anything", 1, f.admin).await;
    assert!(matches!(unknown, Err(DomainError::NotFound)));
}

/// AC-9, AC-17: switching a dish to the value it already has succeeds and
/// changes nothing, and a dish removed a moment ago cannot be switched at all.
#[tokio::test]
async fn switching_a_dish_to_what_it_already_is_changes_nothing() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut tx, restaurant_id).await;

    let same = catalog::set_dish_availability(&mut tx, f.soup, true, f.chef)
        .await
        .expect("switching an available dish on");
    assert!(same.is_available);
    assert_eq!(
        same.version, 1,
        "a switch that changed nothing bumped the version"
    );
    assert!(
        audit_counts(&mut tx).await.is_empty(),
        "a switch that changed nothing wrote an audit row"
    );

    catalog::set_dish_availability(&mut tx, f.soup, false, f.chef)
        .await
        .expect("switching it off");
    assert_eq!(
        audit_counts(&mut tx).await.get("dish_availability_changed"),
        Some(&1)
    );

    catalog::archive_dish(&mut tx, f.steak, f.admin)
        .await
        .expect("removing the steak");
    let gone = catalog::set_dish_availability(&mut tx, f.steak, false, f.chef).await;
    assert!(
        matches!(gone, Err(DomainError::NotFound)),
        "a removed dish was switched: {gone:?}"
    );
}

/// AC-15: an edit that is both stale and aimed at a removed category reports
/// the category, which is what the admin has to change first.
#[tokio::test]
async fn an_edit_both_stale_and_aimed_at_a_removed_category_reports_the_category() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut tx, restaurant_id).await;

    let spare = catalog::create_menu_category(&mut tx, "Specials", f.admin)
        .await
        .expect("adding a category");
    catalog::archive_menu_category(&mut tx, spare.id, f.admin)
        .await
        .expect("removing it again");

    let loaded = catalog::dish(&mut tx, f.soup)
        .await
        .expect("loading the form");
    catalog::set_dish_availability(&mut tx, f.soup, false, f.chef)
        .await
        .expect("making the form stale");

    let refused = catalog::update_dish(
        &mut tx,
        f.soup,
        &DishEdit {
            category_id: spare.id,
            ..edit_of(&loaded)
        },
        f.admin,
    )
    .await;
    assert_eq!(
        conflict_of(refused, "a stale move into a removed category"),
        ConflictKind::CategoryArchived
    );
}

/// AC-7: a category that still holds a live dish cannot be removed.
#[tokio::test]
async fn a_category_holding_a_live_dish_cannot_be_removed() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut tx, restaurant_id).await;

    // Unavailable still counts: switched off is not off the menu.
    catalog::set_dish_availability(&mut tx, f.soup, false, f.chef)
        .await
        .expect("switching the soup off");

    let refused = catalog::archive_menu_category(&mut tx, f.category, f.admin).await;
    assert_eq!(
        conflict_of(refused, "removing a category with dishes"),
        ConflictKind::CategoryNotEmpty
    );

    // A dish cannot be created in, or restored into, a removed category either.
    let spare = catalog::create_menu_category(&mut tx, "Specials", f.admin)
        .await
        .expect("adding a category");
    catalog::archive_menu_category(&mut tx, spare.id, f.admin)
        .await
        .expect("removing the empty one");

    let into_removed = catalog::create_dish(
        &mut tx,
        &NewDish {
            category_id: spare.id,
            name: "Special",
            description: None,
            price: common::money("5"),
            diet: Diet::Egg,
        },
        f.admin,
    )
    .await;
    assert_eq!(
        conflict_of(into_removed, "a dish created in a removed category"),
        ConflictKind::CategoryArchived
    );

    catalog::archive_dish(&mut tx, f.steak, f.admin)
        .await
        .expect("removing the steak");
    let restored_into_removed = catalog::restore_dish(&mut tx, f.steak, spare.id, f.admin).await;
    assert_eq!(
        conflict_of(
            restored_into_removed,
            "a dish restored into a removed category"
        ),
        ConflictKind::CategoryArchived
    );
}

/// AC-7: removing a category while a dish is being created in it, in both
/// orders. Exactly one wins, and the database never holds a live dish under an
/// archived category.
#[tokio::test]
async fn a_dish_created_while_its_category_is_removed_never_leaves_a_live_dish_in_it() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();

    let mut setup = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut setup, restaurant_id).await;
    let first_target = catalog::create_menu_category(&mut setup, "Specials", f.admin)
        .await
        .expect("adding the first category");
    let second_target = catalog::create_menu_category(&mut setup, "Desserts", f.admin)
        .await
        .expect("adding the second category");
    setup.commit().await.expect("committing the fixture");

    // The archive holds its lock first; the create queues behind it and finds
    // the category gone.
    {
        let mut archiving = database
            .begin_scoped(restaurant_id)
            .await
            .expect("opening the archive");
        catalog::archive_menu_category(&mut archiving, first_target.id, f.admin)
            .await
            .expect("the archive takes the category");

        let other = database.clone();
        let target = first_target.id;
        let admin = f.admin;
        let creating = tokio::spawn(async move {
            let mut tx = other
                .begin_scoped(restaurant_id)
                .await
                .expect("opening the create");
            let outcome = catalog::create_dish(
                &mut tx,
                &NewDish {
                    category_id: target,
                    name: "Late special",
                    description: None,
                    price: common::money("5"),
                    diet: Diet::Veg,
                },
                admin,
            )
            .await;
            if outcome.is_ok() {
                tx.commit().await.expect("committing the create");
            }
            outcome
        });

        tokio::time::sleep(Duration::from_millis(300)).await;
        archiving.commit().await.expect("the archive lands");

        let outcome = creating.await.expect("the create ran");
        assert_eq!(
            conflict_of(outcome, "a create behind an archive"),
            ConflictKind::CategoryArchived
        );
    }

    // The create holds its lock first; the archive queues behind it and finds
    // the category no longer empty.
    {
        let mut creating = database
            .begin_scoped(restaurant_id)
            .await
            .expect("opening the create");
        add_dish(&mut creating, second_target.id, "Kulfi", "120", f.admin).await;

        let other = database.clone();
        let target = second_target.id;
        let admin = f.admin;
        let archiving = tokio::spawn(async move {
            let mut tx = other
                .begin_scoped(restaurant_id)
                .await
                .expect("opening the archive");
            let outcome = catalog::archive_menu_category(&mut tx, target, admin).await;
            if outcome.is_ok() {
                tx.commit().await.expect("committing the archive");
            }
            outcome
        });

        tokio::time::sleep(Duration::from_millis(300)).await;
        creating.commit().await.expect("the create lands");

        let outcome = archiving.await.expect("the archive ran");
        assert_eq!(
            conflict_of(outcome, "an archive behind a create"),
            ConflictKind::CategoryNotEmpty
        );
    }

    let mut check = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let stranded: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM dishes AS d
         JOIN menu_categories AS c ON c.id = d.category_id
         WHERE d.archived_at IS NULL AND c.archived_at IS NOT NULL",
    )
    .fetch_one(check.connection())
    .await
    .expect("looking for stranded dishes");
    assert_eq!(stranded, 0, "a live dish sits under an archived category");
    drop(check);

    common::drop_restaurant(&database, restaurant_id).await;
}

/// AC-5: a reorder naming anything but the live list is refused whole.
#[tokio::test]
async fn a_reorder_that_is_not_the_live_list_is_refused_and_moves_nothing() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut tx, restaurant_id).await;

    // The admin's screen was drawn before this dish arrived.
    let latest = add_dish(&mut tx, f.category, "Latest", "3", f.admin).await;

    let missing =
        catalog::reorder_dishes(&mut tx, f.category, &[f.steak.as_uuid(), f.soup.as_uuid()]).await;
    assert_eq!(
        conflict_of(missing, "a reorder missing a new dish"),
        ConflictKind::MenuChanged
    );

    let repeated = catalog::reorder_dishes(
        &mut tx,
        f.category,
        &[f.steak.as_uuid(), f.steak.as_uuid(), f.soup.as_uuid()],
    )
    .await;
    assert_eq!(
        conflict_of(repeated, "a reorder repeating a dish"),
        ConflictKind::MenuChanged
    );

    assert_eq!(
        names_in(&mut tx, f.category).await,
        ["Soup", "Steak", "Latest"],
        "a refused reorder moved something"
    );
    assert_eq!(latest.position, 3);

    let categories = catalog::reorder_menu_categories(&mut tx, &[Uuid::now_v7()]).await;
    assert_eq!(
        conflict_of(categories, "a category reorder naming a stranger"),
        ConflictKind::MenuChanged
    );

    let unknown = catalog::reorder_dishes(&mut tx, MenuCategoryId::new(), &[]).await;
    assert!(matches!(unknown, Err(DomainError::NotFound)));
}

/// AC-14: names are unique among live rows, ignoring letter case and edge
/// spaces, and a removed name is free again.
#[tokio::test]
async fn names_are_unique_among_live_rows_the_way_a_reader_means_it() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut tx, restaurant_id).await;

    common::savepoint(&mut tx, "clash").await;
    let clash = catalog::create_dish(
        &mut tx,
        &NewDish {
            category_id: f.category,
            name: " sOUP  ",
            description: None,
            price: common::money("1"),
            diet: Diet::Veg,
        },
        f.admin,
    )
    .await;
    assert_eq!(
        conflict_of(clash, "a second live Soup"),
        ConflictKind::NameTaken
    );
    common::rollback_to(&mut tx, "clash").await;

    common::savepoint(&mut tx, "category_clash").await;
    let category_clash = catalog::create_menu_category(&mut tx, "MAINS", f.admin).await;
    assert_eq!(
        conflict_of(category_clash, "a second live Mains"),
        ConflictKind::NameTaken
    );
    common::rollback_to(&mut tx, "category_clash").await;

    catalog::archive_dish(&mut tx, f.soup, f.admin)
        .await
        .expect("removing the soup");
    let reused = add_dish(&mut tx, f.category, "SOUP", "2", f.admin).await;
    assert_eq!(reused.name, "SOUP", "a removed name was not free again");
}

/// AC-8: restoring a dish whose name a live dish has taken meanwhile is
/// refused.
#[tokio::test]
async fn restoring_a_dish_whose_name_is_now_taken_is_refused() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut tx, restaurant_id).await;

    let dal = add_dish(&mut tx, f.category, "Dal makhani", "340", f.admin).await;
    catalog::archive_dish(&mut tx, dal.id, f.admin)
        .await
        .expect("removing the dal");
    add_dish(&mut tx, f.category, "Dal Makhani", "360", f.admin).await;

    let refused = catalog::restore_dish(&mut tx, dal.id, f.category, f.admin).await;
    assert_eq!(
        conflict_of(refused, "restoring into a name clash"),
        ConflictKind::NameTaken
    );
}

/// AC-12: a ticket carrying a dish that went off is refused whole, before
/// anything is written.
#[tokio::test]
async fn a_ticket_carrying_a_dish_that_went_off_is_refused_whole() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut tx, restaurant_id).await;

    let visit = service::open_visit(&mut tx, f.table_one, f.waiter, Some(2))
        .await
        .expect("seating a party");

    // The basket was built with both; the kitchen runs out of soup before the
    // send lands.
    catalog::set_dish_availability(&mut tx, f.soup, false, f.chef)
        .await
        .expect("the soup goes off");

    let refused = service::send_round(
        &mut tx,
        visit.id,
        f.waiter,
        &[
            NewOrderLine {
                dish_id: f.steak,
                quantity: 1,
                note: None,
            },
            NewOrderLine {
                dish_id: f.soup,
                quantity: 2,
                note: None,
            },
        ],
    )
    .await;
    assert_eq!(
        conflict_of(refused, "a ticket with a dish that went off"),
        ConflictKind::DishNotOrderable
    );

    let rounds: i64 = sqlx::query_scalar("SELECT count(*) FROM order_rounds")
        .fetch_one(tx.connection())
        .await
        .expect("counting rounds");
    let lines: i64 = sqlx::query_scalar("SELECT count(*) FROM order_lines")
        .fetch_one(tx.connection())
        .await
        .expect("counting lines");
    assert_eq!((rounds, lines), (0, 0), "half a ticket reached the kitchen");
}

/// AC-11: nothing done to a dish reaches a line already sent, or the bill it
/// is on.
#[tokio::test]
async fn no_menu_change_touches_a_line_already_sent() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut tx, restaurant_id).await;

    let visit = service::open_visit(&mut tx, f.table_one, f.waiter, Some(2))
        .await
        .expect("seating a party");
    let bill = billing::open_bill(&mut tx, visit.id, f.waiter)
        .await
        .expect("opening the bill");
    let (_, sent) = service::send_round(
        &mut tx,
        visit.id,
        f.waiter,
        &[NewOrderLine {
            dish_id: f.soup,
            quantity: 2,
            note: None,
        }],
    )
    .await
    .expect("sending the soup");
    let line_ids: Vec<_> = sent.iter().map(|line| line.id).collect();
    billing::assign_lines_to_bill(&mut tx, bill.id, &line_ids)
        .await
        .expect("putting the soup on the bill");

    let before_line = service::line(&mut tx, line_ids[0])
        .await
        .expect("reading the line");
    let before_bill = billing::bill(&mut tx, bill.id)
        .await
        .expect("reading the bill");

    // Reprice and rename, move, switch off, and remove.
    let spare = catalog::create_menu_category(&mut tx, "Soups", f.admin)
        .await
        .expect("adding a category");
    let soup = catalog::dish(&mut tx, f.soup)
        .await
        .expect("reading the soup");
    let edited = catalog::update_dish(
        &mut tx,
        f.soup,
        &DishEdit {
            name: "Consomme".to_owned(),
            price: common::money("99"),
            category_id: spare.id,
            ..edit_of(&soup)
        },
        f.admin,
    )
    .await
    .expect("repricing, renaming, and moving the soup");
    assert_eq!(edited.category_id, spare.id);
    catalog::set_dish_availability(&mut tx, f.soup, false, f.chef)
        .await
        .expect("switching it off");
    catalog::archive_dish(&mut tx, f.soup, f.admin)
        .await
        .expect("removing it");

    let after_line = service::line(&mut tx, line_ids[0])
        .await
        .expect("the line stopped resolving");
    let after_bill = billing::bill(&mut tx, bill.id)
        .await
        .expect("the bill stopped resolving");

    assert_eq!(after_line.dish_name, before_line.dish_name);
    assert_eq!(after_line.unit_price, before_line.unit_price);
    assert_eq!(after_line.line_total, before_line.line_total);
    assert_eq!(after_line.status, before_line.status);
    assert_eq!(
        after_bill.subtotal, before_bill.subtotal,
        "a menu change moved an open bill's subtotal"
    );
}

/// AC-16: another restaurant's dish and category read as not found, whatever
/// is attempted on them.
#[tokio::test]
async fn another_restaurants_menu_reads_as_not_found() {
    let database = common::database().await;
    let alpha = RestaurantId::new();
    let beta = RestaurantId::new();
    let mut tx = database.begin_scoped(alpha).await.expect("opening scoped");

    let a = common::seed(&mut tx, alpha).await;
    common::rescope(&mut tx, beta).await;
    let b = common::seed(&mut tx, beta).await;
    common::rescope(&mut tx, alpha).await;

    let stranger: DishId = b.soup;

    assert!(matches!(
        catalog::set_dish_availability(&mut tx, stranger, false, a.chef).await,
        Err(DomainError::NotFound)
    ));
    assert!(matches!(
        catalog::archive_dish(&mut tx, stranger, a.admin).await,
        Err(DomainError::NotFound)
    ));
    assert!(matches!(
        catalog::rename_menu_category(&mut tx, b.category, "Mine now", 1, a.admin).await,
        Err(DomainError::NotFound)
    ));
    assert!(matches!(
        catalog::archive_menu_category(&mut tx, b.category, a.admin).await,
        Err(DomainError::NotFound)
    ));

    // Moving one of alpha's own dishes into beta's category is refused too.
    let soup = catalog::dish(&mut tx, a.soup)
        .await
        .expect("reading alpha's soup");
    let moved = catalog::update_dish(
        &mut tx,
        a.soup,
        &DishEdit {
            category_id: b.category,
            ..edit_of(&soup)
        },
        a.admin,
    )
    .await;
    assert!(
        matches!(moved, Err(DomainError::NotFound)),
        "a dish was moved into another restaurant's category: {moved:?}"
    );

    // And beta's rows are exactly as they were.
    common::rescope(&mut tx, beta).await;
    let theirs = catalog::dish(&mut tx, b.soup)
        .await
        .expect("reading beta's soup");
    assert!(theirs.is_available && theirs.archived_at.is_none());
}
