//! Tables and floor plan, spec 0010, at the layer the handlers sit on.
//!
//! Every test runs against a real Postgres as `app_api`, so row level security
//! applies exactly as it does to the API. All but the races roll back; a race
//! has to commit, because two transactions only see each other's work once it
//! has, and each one deletes its restaurant at the end.
//!
//! The fixture already has a section called `Terrace` holding `T1` and `T2`,
//! each seating four, which several tests below lean on.
//!
//! Covers AC-2, AC-3, AC-4, AC-5, AC-6, AC-7, AC-8, AC-9, AC-10, AC-11, AC-12,
//! AC-13, AC-16, and AC-17.

mod common;

use std::collections::BTreeMap;
use std::time::Duration;

use api::domain::catalog::DiningTable;
use api::domain::error::{ConflictKind, DomainError};
use api::domain::ids::{DiningTableId, RestaurantId, StaffId, TableSectionId};
use api::domain::service::NewOrderLine;
use api::infrastructure::db::repository::floor::{self, NewTable, NewTableRange, TableEdit};
use api::infrastructure::db::repository::service;
use api::infrastructure::db::{Database, ScopedTx};

/// The conflict a refusal carries, or a panic naming what came back instead.
fn conflict_of<T: std::fmt::Debug>(outcome: Result<T, DomainError>, what: &str) -> ConflictKind {
    match outcome {
        Err(DomainError::Conflict(kind)) => kind,
        other => panic!("{what} came back as {other:?} rather than a conflict"),
    }
}

/// Adds one table the way the admin screen does.
async fn add_table(
    tx: &mut ScopedTx<'_>,
    section_id: Option<TableSectionId>,
    label: &str,
    actor: StaffId,
) -> DiningTable {
    floor::create_dining_table(
        tx,
        &NewTable {
            section_id,
            label,
            seats: Some(2),
        },
        actor,
    )
    .await
    .unwrap_or_else(|error| panic!("adding {label}: {error:?}"))
}

/// An edit that keeps everything about the table but what the caller changes.
fn edit_of(table: &DiningTable) -> TableEdit {
    TableEdit {
        section_id: table.section_id,
        label: table.label.clone(),
        seats: table.seats,
        version: table.version,
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

fn counts(pairs: &[(&str, i64)]) -> BTreeMap<String, i64> {
    pairs
        .iter()
        .map(|(action, count)| ((*action).to_owned(), *count))
        .collect()
}

/// The live labels of one group, in displayed order.
async fn labels_in(tx: &mut ScopedTx<'_>, section_id: Option<TableSectionId>) -> Vec<String> {
    floor::live_dining_tables(tx)
        .await
        .expect("reading the floor")
        .into_iter()
        .filter(|table| table.section_id == section_id)
        .map(|table| table.label)
        .collect()
}

fn labels(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

/// AC-2, AC-3, AC-4, AC-5, AC-17: sections and tables land at the end of
/// their lists, a range lands in number order, an edit keeps a table in place,
/// and a move lands at the end of the new group, each write with its version
/// and its one audit row.
#[tokio::test]
async fn a_floor_is_built_edited_and_moved() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut tx, restaurant_id).await;

    let garden = floor::create_table_section(&mut tx, "  Garden ", f.admin)
        .await
        .expect("adding Garden");
    let bar = floor::create_table_section(&mut tx, "Bar", f.admin)
        .await
        .expect("adding Bar");
    assert_eq!(garden.name, "Garden", "the name was not trimmed");
    assert_eq!((garden.position, bar.position), (2, 3));
    assert_eq!(garden.version, 1);

    // One loose table, then a range in the garden.
    let counter = add_table(&mut tx, None, "Counter", f.admin).await;
    assert_eq!((counter.section_id, counter.position), (None, 1));

    let range = floor::create_table_range(
        &mut tx,
        &NewTableRange {
            section_id: Some(garden.id),
            labels: &labels(&["G1", "G2", "G3"]),
            seats: Some(6),
        },
        f.admin,
    )
    .await
    .expect("adding G1 to G3");
    assert_eq!(
        range
            .iter()
            .map(|table| (table.label.as_str(), table.position, table.seats))
            .collect::<Vec<_>>(),
        [("G1", 1, Some(6)), ("G2", 2, Some(6)), ("G3", 3, Some(6))]
    );

    // A seats edit keeps the table where it is and bumps its version.
    let g2 = range[1].clone();
    let reseated = floor::edit_dining_table(
        &mut tx,
        g2.id,
        &TableEdit {
            seats: Some(8),
            ..edit_of(&g2)
        },
        f.admin,
    )
    .await
    .expect("reseating G2");
    assert_eq!((reseated.position, reseated.version), (2, 2));

    // A move lands at the end of the new group; so does a move to no section.
    let moved = floor::edit_dining_table(
        &mut tx,
        g2.id,
        &TableEdit {
            section_id: Some(f.section),
            ..edit_of(&reseated)
        },
        f.admin,
    )
    .await
    .expect("moving G2 to the terrace");
    assert_eq!((moved.position, moved.version), (3, 3));
    assert_eq!(
        labels_in(&mut tx, Some(f.section)).await,
        ["T1", "T2", "G2"]
    );

    let loose = floor::edit_dining_table(
        &mut tx,
        g2.id,
        &TableEdit {
            section_id: None,
            ..edit_of(&moved)
        },
        f.admin,
    )
    .await
    .expect("moving G2 to no section");
    assert_eq!(loose.position, 2);
    assert_eq!(labels_in(&mut tx, None).await, ["Counter", "G2"]);

    let renamed = floor::rename_table_section(&mut tx, bar.id, "Bar counter", 1, f.admin)
        .await
        .expect("renaming Bar");
    assert_eq!(renamed.version, 2);

    assert_eq!(
        audit_counts(&mut tx).await,
        counts(&[
            ("table_section_created", 2),
            ("table_section_renamed", 1),
            ("dining_table_created", 4),
            ("dining_table_edited", 3),
        ]),
        "the audit log does not match the changes"
    );
}

/// AC-6, AC-10, AC-17: both reorders rewrite positions and nothing else, and a
/// removed table comes back at the end of the group it is put into, keeping
/// its label and seats. A reorder writes no audit row.
#[tokio::test]
async fn a_floor_is_ordered_removed_and_restored() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut tx, restaurant_id).await;

    let garden = floor::create_table_section(&mut tx, "Garden", f.admin)
        .await
        .expect("adding Garden");
    let t3 = add_table(&mut tx, Some(f.section), "T3", f.admin).await;

    let sections =
        floor::reorder_table_sections(&mut tx, &[garden.id.as_uuid(), f.section.as_uuid()])
            .await
            .expect("reordering the sections");
    assert_eq!(
        sections.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
        ["Garden", "Terrace"]
    );
    assert!(
        sections.iter().all(|s| s.version == 1),
        "a reorder bumped a version"
    );

    let tables = floor::reorder_table_group(
        &mut tx,
        Some(f.section),
        &[
            t3.id.as_uuid(),
            f.table_two.as_uuid(),
            f.table_one.as_uuid(),
        ],
    )
    .await
    .expect("reordering the terrace");
    assert_eq!(
        tables.iter().map(|t| t.label.as_str()).collect::<Vec<_>>(),
        ["T3", "T2", "T1"]
    );
    assert_eq!(t3.version, tables[0].version, "a reorder bumped a version");

    // Removed, then put back into the garden with its label and seats.
    let archived = floor::archive_dining_table(&mut tx, t3.id, f.admin)
        .await
        .expect("removing T3");
    assert!(archived.archived_at.is_some());
    assert_eq!(archived.version, 2);
    assert_eq!(labels_in(&mut tx, Some(f.section)).await, ["T2", "T1"]);

    let listed = floor::archived_dining_tables(&mut tx)
        .await
        .expect("reading the archived tables");
    let entry = listed
        .iter()
        .find(|entry| entry.table.id == t3.id)
        .expect("T3 is in the archived list");
    assert_eq!(entry.section_name.as_deref(), Some("Terrace"));
    assert!(entry.section_live);

    let restored = floor::restore_dining_table(&mut tx, t3.id, Some(garden.id), f.admin)
        .await
        .expect("putting T3 back into the garden");
    assert_eq!(
        (
            restored.section_id,
            restored.label.as_str(),
            restored.seats,
            restored.position,
            restored.version
        ),
        (Some(garden.id), "T3", Some(2), 1, 3)
    );

    assert_eq!(
        audit_counts(&mut tx).await,
        counts(&[
            ("table_section_created", 1),
            ("dining_table_created", 1),
            ("dining_table_archived", 1),
            ("dining_table_restored", 1),
        ]),
        "a reorder wrote an audit row, or a change did not"
    );
}

/// AC-8, AC-5: a table with a party at it cannot be removed, and can be
/// renamed; the new label is what the table screen and the kitchen read.
#[tokio::test]
async fn a_busy_table_stays_and_can_still_be_renamed() {
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
    common::send_round(
        &mut tx,
        visit.id,
        f.waiter,
        &[NewOrderLine {
            dish_id: f.soup,
            quantity: 1,
            note: None,
        }],
    )
    .await
    .expect("sending a ticket");

    assert_eq!(
        conflict_of(
            floor::archive_dining_table(&mut tx, f.table_one, f.admin).await,
            "removing a busy table"
        ),
        ConflictKind::TableInUse
    );

    let occupancy = floor::live_tables_with_occupancy(&mut tx)
        .await
        .expect("reading the admin floor");
    let busy = occupancy
        .iter()
        .find(|entry| entry.table.id == f.table_one)
        .expect("T1 is still live");
    assert!(busy.occupied);
    assert!(
        occupancy
            .iter()
            .filter(|entry| entry.table.id != f.table_one)
            .all(|entry| !entry.occupied)
    );

    let t1 = floor::dining_table(&mut tx, f.table_one)
        .await
        .expect("reading T1");
    floor::edit_dining_table(
        &mut tx,
        f.table_one,
        &TableEdit {
            label: "Window 1".to_owned(),
            ..edit_of(&t1)
        },
        f.admin,
    )
    .await
    .expect("renaming a busy table");

    assert_eq!(
        service::table_label(&mut tx, f.table_one)
            .await
            .expect("reading the label"),
        "Window 1"
    );
    let tickets = service::kitchen_queue(&mut tx)
        .await
        .expect("reading the kitchen queue")
        .tickets;
    assert_eq!(
        tickets
            .iter()
            .map(|ticket| ticket.table_label.as_str())
            .collect::<Vec<_>>(),
        ["Window 1"]
    );

    // Once the party leaves, the table can go, and its visit still names it.
    let bare = service::open_visit(&mut tx, f.table_two, f.waiter, None)
        .await
        .expect("seating a party that orders nothing");
    service::close_visit(&mut tx, bare.id)
        .await
        .expect("they leave");
    floor::archive_dining_table(&mut tx, f.table_two, f.admin)
        .await
        .expect("removing a table whose party has left");
    assert_eq!(
        service::table_label(&mut tx, f.table_two)
            .await
            .expect("an archived table's label still resolves"),
        "T2"
    );
}

/// AC-8: removing a table while a waiter opens it, in both orders. Exactly one
/// wins, and the database never holds an open visit on an archived table.
#[tokio::test]
async fn a_table_opened_while_it_is_removed_never_leaves_a_party_on_a_removed_table() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();

    let mut setup = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut setup, restaurant_id).await;
    setup.commit().await.expect("committing the fixture");

    // The removal holds its lock first; the opening queues and finds it gone.
    {
        let mut archiving = database
            .begin_scoped(restaurant_id)
            .await
            .expect("opening the removal");
        floor::archive_dining_table(&mut archiving, f.table_one, f.admin)
            .await
            .expect("the removal takes the table");

        let other = database.clone();
        let (table, waiter) = (f.table_one, f.waiter);
        let opening = tokio::spawn(async move {
            let mut tx = other
                .begin_scoped(restaurant_id)
                .await
                .expect("opening the visit transaction");
            common::name_racer(&mut tx).await;
            let outcome = service::open_visit(&mut tx, table, waiter, None).await;
            if outcome.is_ok() {
                tx.commit().await.expect("committing the visit");
            }
            outcome.map(|visit| visit.id)
        });

        common::until_blocked(&database, restaurant_id, &opening).await;
        archiving.commit().await.expect("the removal lands");

        let outcome = opening.await.expect("the opening ran");
        assert!(
            matches!(outcome, Err(DomainError::Invalid(_))),
            "a party was seated at a table removed first: {outcome:?}"
        );
    }

    // The opening holds its lock first; the removal queues and finds a party.
    {
        let mut opening = database
            .begin_scoped(restaurant_id)
            .await
            .expect("opening the visit transaction");
        service::open_visit(&mut opening, f.table_two, f.waiter, None)
            .await
            .expect("the party sits down");

        let other = database.clone();
        let (table, admin) = (f.table_two, f.admin);
        let archiving = tokio::spawn(async move {
            let mut tx = other
                .begin_scoped(restaurant_id)
                .await
                .expect("opening the removal");
            common::name_racer(&mut tx).await;
            let outcome = floor::archive_dining_table(&mut tx, table, admin).await;
            if outcome.is_ok() {
                tx.commit().await.expect("committing the removal");
            }
            outcome.map(|archived| archived.id)
        });

        common::until_blocked(&database, restaurant_id, &archiving).await;
        opening.commit().await.expect("the party is seated");

        let outcome = archiving.await.expect("the removal ran");
        assert_eq!(
            conflict_of(outcome, "a removal behind a seating"),
            ConflictKind::TableInUse
        );
    }

    let mut check = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let stranded: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM visits AS v
         JOIN dining_tables AS t ON t.id = v.table_id
         WHERE v.status = 'open' AND t.archived_at IS NOT NULL",
    )
    .fetch_one(check.connection())
    .await
    .expect("looking for stranded parties");
    assert_eq!(stranded, 0, "an open visit sits on a removed table");
    drop(check);

    common::drop_restaurant(&database, restaurant_id).await;
}

/// A committed restaurant with one extra live section, for the section races.
async fn committed_with_section(
    database: &Database,
    restaurant_id: RestaurantId,
) -> (common::Fixture, TableSectionId) {
    let mut setup = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut setup, restaurant_id).await;
    let garden = floor::create_table_section(&mut setup, "Garden", f.admin)
        .await
        .expect("adding Garden");
    setup.commit().await.expect("committing the fixture");

    (f, garden.id)
}

/// Starts removing a section on another connection, where it queues behind
/// whatever lock the caller's transaction holds.
fn archive_section_behind(
    database: &Database,
    restaurant_id: RestaurantId,
    section: TableSectionId,
    admin: StaffId,
) -> tokio::task::JoinHandle<Result<TableSectionId, DomainError>> {
    let other = database.clone();

    tokio::spawn(async move {
        let mut tx = other
            .begin_scoped(restaurant_id)
            .await
            .expect("opening the removal");
        common::name_racer(&mut tx).await;
        let outcome = floor::archive_table_section(&mut tx, section, admin).await;
        if outcome.is_ok() {
            tx.commit().await.expect("committing the removal");
        }
        outcome.map(|archived| archived.id)
    })
}

/// No live table sits in a removed section; then the restaurant goes.
async fn assert_no_stranded_table(database: &Database, restaurant_id: RestaurantId) {
    let mut check = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let stranded: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM dining_tables AS t
         JOIN table_sections AS s ON s.id = t.section_id
         WHERE t.archived_at IS NULL AND s.archived_at IS NOT NULL",
    )
    .fetch_one(check.connection())
    .await
    .expect("looking for stranded tables");
    assert_eq!(stranded, 0, "a live table sits in a removed section");
    drop(check);

    common::drop_restaurant(database, restaurant_id).await;
}

/// AC-9: removing a section while a table is created in it. The removal holds
/// its lock first; the create queues and finds the section gone.
#[tokio::test]
async fn a_table_created_while_its_section_is_removed_is_refused() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let (f, garden) = committed_with_section(&database, restaurant_id).await;

    let mut archiving = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening the removal");
    floor::archive_table_section(&mut archiving, garden, f.admin)
        .await
        .expect("the removal takes the section");

    let other = database.clone();
    let admin = f.admin;
    let creating = tokio::spawn(async move {
        let mut tx = other
            .begin_scoped(restaurant_id)
            .await
            .expect("opening the create");
        common::name_racer(&mut tx).await;
        let outcome = floor::create_dining_table(
            &mut tx,
            &NewTable {
                section_id: Some(garden),
                label: "Late",
                seats: None,
            },
            admin,
        )
        .await;
        if outcome.is_ok() {
            tx.commit().await.expect("committing the create");
        }
        outcome.map(|table| table.id)
    });

    common::until_blocked(&database, restaurant_id, &creating).await;
    archiving.commit().await.expect("the removal lands");

    assert_eq!(
        conflict_of(
            creating.await.expect("the create ran"),
            "a create behind a removal"
        ),
        ConflictKind::SectionArchived
    );

    assert_no_stranded_table(&database, restaurant_id).await;
}

/// AC-9: a table moved into a section while it is removed. The move holds its
/// lock first; the removal queues and finds a table.
#[tokio::test]
async fn a_section_removed_while_a_table_moves_into_it_is_refused() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let (f, garden) = committed_with_section(&database, restaurant_id).await;

    let mut moving = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening the move");
    let t1 = floor::dining_table(&mut moving, f.table_one)
        .await
        .expect("reading T1");
    floor::edit_dining_table(
        &mut moving,
        f.table_one,
        &TableEdit {
            section_id: Some(garden),
            ..edit_of(&t1)
        },
        f.admin,
    )
    .await
    .expect("the move takes the section");

    let archiving = archive_section_behind(&database, restaurant_id, garden, f.admin);
    common::until_blocked(&database, restaurant_id, &archiving).await;
    moving.commit().await.expect("the move lands");

    assert_eq!(
        conflict_of(
            archiving.await.expect("the removal ran"),
            "a removal behind a move"
        ),
        ConflictKind::SectionNotEmpty
    );

    assert_no_stranded_table(&database, restaurant_id).await;
}

/// AC-9: a table restored into a section while it is removed. The restore
/// holds its lock first; the removal queues and finds a table.
#[tokio::test]
async fn a_section_removed_while_a_table_is_restored_into_it_is_refused() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let (f, garden) = committed_with_section(&database, restaurant_id).await;

    let mut setup = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    floor::archive_dining_table(&mut setup, f.table_two, f.admin)
        .await
        .expect("removing T2");
    setup.commit().await.expect("committing the removal");

    let mut restoring = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening the restore");
    floor::restore_dining_table(&mut restoring, f.table_two, Some(garden), f.admin)
        .await
        .expect("the restore takes the section");

    let archiving = archive_section_behind(&database, restaurant_id, garden, f.admin);
    common::until_blocked(&database, restaurant_id, &archiving).await;
    restoring.commit().await.expect("the restore lands");

    assert_eq!(
        conflict_of(
            archiving.await.expect("the removal ran"),
            "a removal behind a restore"
        ),
        ConflictKind::SectionNotEmpty
    );

    assert_no_stranded_table(&database, restaurant_id).await;
}

/// AC-4, AC-12: a range whose labels clash with live tables, ignoring case, is
/// refused whole, naming each clash as the range spelled it.
#[tokio::test]
async fn a_range_that_clashes_is_refused_whole_and_names_every_clash() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut tx, restaurant_id).await;
    add_table(&mut tx, None, "T3", f.admin).await;

    let before = floor::live_dining_tables(&mut tx)
        .await
        .expect("reading")
        .len();
    let audited = audit_counts(&mut tx).await;

    let refused = floor::create_table_range(
        &mut tx,
        &NewTableRange {
            section_id: None,
            labels: &labels(&["t1", "t2", "t3", "t4", "t5"]),
            seats: None,
        },
        f.admin,
    )
    .await;
    assert_eq!(
        conflict_of(refused, "a clashing range"),
        ConflictKind::LabelsTaken(labels(&["t1", "t2", "t3"]))
    );

    assert_eq!(
        floor::live_dining_tables(&mut tx)
            .await
            .expect("reading")
            .len(),
        before,
        "a refused range created a table"
    );
    assert_eq!(
        audit_counts(&mut tx).await,
        audited,
        "a refused range wrote an audit row"
    );

    // The same rule for a single table: a clash in another letter case.
    assert_eq!(
        conflict_of(
            floor::create_dining_table(
                &mut tx,
                &NewTable {
                    section_id: None,
                    label: "t1",
                    seats: None,
                },
                f.admin,
            )
            .await,
            "a clashing single table"
        ),
        ConflictKind::NameTaken
    );
}

/// AC-4: a label a colleague takes between the check and the insert fails the
/// insert, which says so with an empty list; a fresh read then names it.
#[tokio::test]
async fn a_label_taken_between_the_check_and_the_insert_is_found_afterwards() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();

    let mut setup = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut setup, restaurant_id).await;
    setup.commit().await.expect("committing the fixture");

    // A colleague's table, inserted and not yet committed: invisible to the
    // range's check, and holding the index entry the range's insert needs.
    let mut colleague = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening the colleague");
    add_table(&mut colleague, None, "Z2", f.admin).await;

    let other = database.clone();
    let admin = f.admin;
    let ranging = tokio::spawn(async move {
        let mut tx = other
            .begin_scoped(restaurant_id)
            .await
            .expect("opening the range");
        common::name_racer(&mut tx).await;
        floor::create_table_range(
            &mut tx,
            &NewTableRange {
                section_id: None,
                labels: &labels(&["Z1", "Z2", "Z3"]),
                seats: None,
            },
            admin,
        )
        .await
        .map(|tables| tables.len())
    });

    common::until_blocked(&database, restaurant_id, &ranging).await;
    colleague
        .commit()
        .await
        .expect("the colleague's table lands");

    assert_eq!(
        conflict_of(ranging.await.expect("the range ran"), "a late clash"),
        ConflictKind::LabelsTaken(Vec::new())
    );

    let mut fresh = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening the fresh read");
    assert_eq!(
        floor::clashing_labels(&mut fresh, &labels(&["Z1", "Z2", "Z3"]))
            .await
            .expect("reading the clash again"),
        ["Z2"]
    );
    assert_eq!(
        labels_in(&mut fresh, None).await,
        ["Z2"],
        "the range wrote a row"
    );
    drop(fresh);

    common::drop_restaurant(&database, restaurant_id).await;
}

/// AC-13: a stale edit or rename is refused and writes nothing; an edit aimed
/// at a removed section reports the section first.
#[tokio::test]
async fn a_stale_edit_or_rename_is_refused() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut tx, restaurant_id).await;

    let t1 = floor::dining_table(&mut tx, f.table_one)
        .await
        .expect("reading T1");
    floor::edit_dining_table(
        &mut tx,
        f.table_one,
        &TableEdit {
            seats: Some(3),
            ..edit_of(&t1)
        },
        f.admin,
    )
    .await
    .expect("the first tab saves");

    assert_eq!(
        conflict_of(
            floor::edit_dining_table(
                &mut tx,
                f.table_one,
                &TableEdit {
                    seats: Some(9),
                    ..edit_of(&t1)
                },
                f.admin,
            )
            .await,
            "the second tab's save"
        ),
        ConflictKind::TableChanged
    );
    assert_eq!(
        floor::dining_table(&mut tx, f.table_one)
            .await
            .expect("reading T1")
            .seats,
        Some(3)
    );

    assert_eq!(
        conflict_of(
            floor::rename_table_section(&mut tx, f.section, "Porch", 7, f.admin).await,
            "a stale rename"
        ),
        ConflictKind::SectionChanged
    );

    let garden = floor::create_table_section(&mut tx, "Garden", f.admin)
        .await
        .expect("adding Garden");
    floor::archive_table_section(&mut tx, garden.id, f.admin)
        .await
        .expect("removing Garden");
    assert_eq!(
        conflict_of(
            floor::edit_dining_table(
                &mut tx,
                f.table_two,
                &TableEdit {
                    section_id: Some(garden.id),
                    label: "T2".to_owned(),
                    seats: None,
                    version: 99,
                },
                f.admin,
            )
            .await,
            "a stale move into a removed section"
        ),
        ConflictKind::SectionArchived
    );
}

/// AC-7: a reorder naming anything but the live group is refused whole.
#[tokio::test]
async fn a_reorder_that_is_not_the_live_group_is_refused_and_moves_nothing() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut tx, restaurant_id).await;

    // A table created a moment ago is missing from the list.
    add_table(&mut tx, Some(f.section), "T3", f.admin).await;
    assert_eq!(
        conflict_of(
            floor::reorder_table_group(
                &mut tx,
                Some(f.section),
                &[f.table_two.as_uuid(), f.table_one.as_uuid()],
            )
            .await,
            "a reorder missing a table"
        ),
        ConflictKind::FloorChanged
    );
    assert_eq!(
        labels_in(&mut tx, Some(f.section)).await,
        ["T1", "T2", "T3"]
    );

    // An id repeated in place of another.
    let t2 = f.table_two.as_uuid();
    assert_eq!(
        conflict_of(
            floor::reorder_table_group(&mut tx, Some(f.section), &[t2, t2, t2]).await,
            "a reorder with a repeat"
        ),
        ConflictKind::FloorChanged
    );
    assert_eq!(
        conflict_of(
            floor::reorder_table_sections(&mut tx, &[]).await,
            "a section reorder missing the terrace"
        ),
        ConflictKind::FloorChanged
    );

    // A group that is not a live section is not found.
    assert!(matches!(
        floor::reorder_table_group(&mut tx, Some(TableSectionId::new()), &[]).await,
        Err(DomainError::NotFound)
    ));
}

/// AC-6: the no section group also holds a live table whose section was
/// archived before this feature existed, which is where both floors show it.
#[tokio::test]
async fn the_no_section_group_holds_a_table_left_in_an_old_archived_section() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut tx, restaurant_id).await;

    // The shape a pre 0010 archive could leave: a section archived under a
    // live table, written by hand because nothing can make it any more.
    sqlx::query("UPDATE table_sections SET archived_at = now() WHERE id = $1")
        .bind(f.section.as_uuid())
        .execute(tx.connection())
        .await
        .expect("archiving the terrace by hand");
    let counter = add_table(&mut tx, None, "Counter", f.admin).await;

    let tables = floor::reorder_table_group(
        &mut tx,
        None,
        &[
            counter.id.as_uuid(),
            f.table_two.as_uuid(),
            f.table_one.as_uuid(),
        ],
    )
    .await
    .expect("reordering the no section group");
    assert_eq!(
        tables.iter().map(|t| t.label.as_str()).collect::<Vec<_>>(),
        ["Counter", "T2", "T1"]
    );
}

/// AC-11, AC-17: a section restore with a clashing ticked table restores
/// nothing; unticking it restores the section and the rest, in their old order,
/// with one audit row each.
#[tokio::test]
async fn a_section_comes_back_with_its_tables_or_not_at_all() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut tx, restaurant_id).await;

    let patio = floor::create_table_section(&mut tx, "Patio", f.admin)
        .await
        .expect("adding Patio");
    let p1 = add_table(&mut tx, Some(patio.id), "P1", f.admin).await;
    let p2 = add_table(&mut tx, Some(patio.id), "P2", f.admin).await;
    floor::archive_dining_table(&mut tx, p1.id, f.admin)
        .await
        .expect("removing P1");
    floor::archive_dining_table(&mut tx, p2.id, f.admin)
        .await
        .expect("removing P2");
    floor::archive_table_section(&mut tx, patio.id, f.admin)
        .await
        .expect("removing Patio");
    add_table(&mut tx, None, "p2", f.admin).await;

    let before = audit_counts(&mut tx).await;

    common::savepoint(&mut tx, "clash").await;
    assert_eq!(
        conflict_of(
            floor::restore_table_section(
                &mut tx,
                patio.id,
                &[p2.id.as_uuid(), p1.id.as_uuid()],
                f.admin,
            )
            .await,
            "a restore with a clashing table"
        ),
        ConflictKind::LabelsTaken(labels(&["P2"]))
    );
    // The handler drops the transaction on a refusal; here a savepoint stands
    // in for that, and nothing it wrote survives.
    common::rollback_to(&mut tx, "clash").await;
    assert!(
        floor::live_table_sections(&mut tx)
            .await
            .expect("reading")
            .iter()
            .all(|section| section.id != patio.id),
        "a refused restore brought the section back"
    );

    // An id that is not an archived table of this section is refused.
    assert!(matches!(
        floor::restore_table_section(&mut tx, patio.id, &[f.table_one.as_uuid()], f.admin).await,
        Err(DomainError::Invalid(_))
    ));

    let (section, tables) =
        floor::restore_table_section(&mut tx, patio.id, &[p1.id.as_uuid()], f.admin)
            .await
            .expect("restoring Patio with P1");
    assert!(section.archived_at.is_none());
    assert_eq!(section.position, 2);
    assert_eq!(
        tables
            .iter()
            .map(|t| (t.label.as_str(), t.position, t.section_id))
            .collect::<Vec<_>>(),
        [("P1", 1, Some(patio.id))]
    );

    let after = audit_counts(&mut tx).await;
    assert_eq!(
        after.get("table_section_restored").copied().unwrap_or(0)
            - before.get("table_section_restored").copied().unwrap_or(0),
        1
    );
    assert_eq!(
        after.get("dining_table_restored").copied().unwrap_or(0)
            - before.get("dining_table_restored").copied().unwrap_or(0),
        1
    );

    // A section whose name was taken meanwhile is refused as a name clash.
    floor::archive_dining_table(&mut tx, p1.id, f.admin)
        .await
        .expect("removing P1 again");
    floor::archive_table_section(&mut tx, patio.id, f.admin)
        .await
        .expect("removing Patio again");
    floor::create_table_section(&mut tx, "PATIO", f.admin)
        .await
        .expect("a new PATIO");
    assert_eq!(
        conflict_of(
            floor::restore_table_section(&mut tx, patio.id, &[], f.admin).await,
            "a restore whose name is taken"
        ),
        ConflictKind::NameTaken
    );
}

/// AC-10: a restore into a removed section, or of a label now taken, is
/// refused.
#[tokio::test]
async fn a_table_restore_is_refused_when_its_section_or_label_is_gone() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();
    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut tx, restaurant_id).await;

    let garden = floor::create_table_section(&mut tx, "Garden", f.admin)
        .await
        .expect("adding Garden");
    floor::archive_table_section(&mut tx, garden.id, f.admin)
        .await
        .expect("removing Garden");
    floor::archive_dining_table(&mut tx, f.table_two, f.admin)
        .await
        .expect("removing T2");

    assert_eq!(
        conflict_of(
            floor::restore_dining_table(&mut tx, f.table_two, Some(garden.id), f.admin).await,
            "a restore into a removed section"
        ),
        ConflictKind::SectionArchived
    );

    add_table(&mut tx, None, "t2", f.admin).await;
    common::savepoint(&mut tx, "taken").await;
    assert_eq!(
        conflict_of(
            floor::restore_dining_table(&mut tx, f.table_two, None, f.admin).await,
            "a restore whose label is taken"
        ),
        ConflictKind::NameTaken
    );
    common::rollback_to(&mut tx, "taken").await;

    // A live table is not an archived one.
    assert!(matches!(
        floor::restore_dining_table(&mut tx, f.table_one, None, f.admin).await,
        Err(DomainError::NotFound)
    ));
}

/// AC-16: another restaurant's tables and sections read as not found, whatever
/// is attempted on them.
#[tokio::test]
async fn another_restaurants_floor_reads_as_not_found() {
    let database = common::database().await;
    let alpha = RestaurantId::new();
    let beta = RestaurantId::new();
    let mut tx = database.begin_scoped(alpha).await.expect("opening scoped");

    let a = common::seed(&mut tx, alpha).await;
    common::rescope(&mut tx, beta).await;
    let b = common::seed(&mut tx, beta).await;
    common::rescope(&mut tx, alpha).await;

    let stranger: DiningTableId = b.table_one;

    assert!(matches!(
        floor::archive_dining_table(&mut tx, stranger, a.admin).await,
        Err(DomainError::NotFound)
    ));
    assert!(matches!(
        floor::edit_dining_table(
            &mut tx,
            stranger,
            &TableEdit {
                section_id: None,
                label: "Mine".to_owned(),
                seats: None,
                version: 1,
            },
            a.admin,
        )
        .await,
        Err(DomainError::NotFound)
    ));
    assert!(matches!(
        floor::rename_table_section(&mut tx, b.section, "Mine now", 1, a.admin).await,
        Err(DomainError::NotFound)
    ));
    assert!(matches!(
        floor::archive_table_section(&mut tx, b.section, a.admin).await,
        Err(DomainError::NotFound)
    ));
    assert!(matches!(
        floor::reorder_table_group(&mut tx, Some(b.section), &[]).await,
        Err(DomainError::NotFound)
    ));

    // Putting one of alpha's own tables into beta's section is refused too.
    let mine = floor::dining_table(&mut tx, a.table_one)
        .await
        .expect("reading alpha's table");
    assert!(matches!(
        floor::edit_dining_table(
            &mut tx,
            a.table_one,
            &TableEdit {
                section_id: Some(b.section),
                ..edit_of(&mine)
            },
            a.admin,
        )
        .await,
        Err(DomainError::NotFound)
    ));
    common::rescope(&mut tx, beta).await;
    let theirs = floor::dining_table(&mut tx, b.table_one)
        .await
        .expect("reading beta's table");
    assert!(theirs.archived_at.is_none());
    assert_eq!(theirs.label, "T1");
}

/// AC-17, AC-18: putting a section back with two tables in it writes three
/// audit rows and sends three notifications, one for the section and one per
/// table, all inside the transaction that made the change.
///
/// This one commits, because a `NOTIFY` is delivered only when its transaction
/// does. A listen connection is opened after the fixture has landed, so the
/// only traffic it can see from this restaurant is the restore itself, and the
/// payloads are read back through the same `DomainEvent` the real listener
/// uses, so a wire format that drifted would fail here too.
#[tokio::test]
async fn putting_a_section_back_tells_every_screen_about_it_and_its_tables() {
    let database = common::database().await;
    let restaurant_id = RestaurantId::new();

    let mut setup = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening scoped");
    let f = common::seed(&mut setup, restaurant_id).await;

    let patio = floor::create_table_section(&mut setup, "Patio", f.admin)
        .await
        .expect("adding Patio");
    let p1 = add_table(&mut setup, Some(patio.id), "P1", f.admin).await;
    let p2 = add_table(&mut setup, Some(patio.id), "P2", f.admin).await;
    for table in [p1.id, p2.id] {
        floor::archive_dining_table(&mut setup, table, f.admin)
            .await
            .expect("removing a Patio table");
    }
    floor::archive_table_section(&mut setup, patio.id, f.admin)
        .await
        .expect("removing Patio");
    setup.commit().await.expect("committing the fixture");

    // Listening starts now, so nothing the fixture sent can be counted here.
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL");
    let mut listener = sqlx::postgres::PgListener::connect(&database_url)
        .await
        .expect("opening a listen connection");
    listener
        .listen("entity_changed")
        .await
        .expect("listening on entity_changed");

    let mut tx = database
        .begin_scoped(restaurant_id)
        .await
        .expect("opening the restore");
    let before = audit_counts(&mut tx).await;
    let (section, tables) = floor::restore_table_section(
        &mut tx,
        patio.id,
        &[p1.id.as_uuid(), p2.id.as_uuid()],
        f.admin,
    )
    .await
    .expect("restoring Patio with both tables");
    assert!(section.archived_at.is_none());
    assert_eq!(tables.len(), 2);

    let after = audit_counts(&mut tx).await;
    let written = |action: &str| {
        after.get(action).copied().unwrap_or(0) - before.get(action).copied().unwrap_or(0)
    };
    assert_eq!(
        (
            written("table_section_restored"),
            written("dining_table_restored")
        ),
        (1, 2),
        "a section restore with two tables did not write three audit rows"
    );

    tx.commit().await.expect("the restore lands");

    // Three notifications, and no fourth. Other tests share this channel, so
    // only this restaurant's are counted.
    let mut seen = Vec::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while tokio::time::Instant::now() < deadline {
        let Ok(received) = tokio::time::timeout_at(deadline, listener.recv()).await else {
            break;
        };
        let notification = received.expect("the listen connection stayed up");
        let event: api::domain::event::DomainEvent =
            serde_json::from_str(notification.payload()).expect("a payload the listener can read");
        if event.restaurant_id == restaurant_id {
            seen.push(event);
            if seen.len() == 3 {
                break;
            }
        }
    }

    let mut kinds: Vec<String> = seen
        .iter()
        .map(|event| format!("{:?}", event.entity))
        .collect();
    kinds.sort();
    assert_eq!(
        kinds,
        ["DiningTable", "DiningTable", "TableSection"],
        "the restore did not tell the screens about the section and both tables"
    );
    assert!(
        seen.iter()
            .any(|event| event.entity_id == patio.id.as_uuid()),
        "no notification named the section that came back"
    );
    for table in [p1.id, p2.id] {
        assert!(
            seen.iter().any(|event| event.entity_id == table.as_uuid()),
            "no notification named a table that came back"
        );
    }

    common::drop_restaurant(&database, restaurant_id).await;
}
