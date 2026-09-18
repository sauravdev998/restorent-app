//! The floor: table sections and the tables in them, and every change an admin
//! makes to them.
//!
//! Spec 0010. Every write here notifies inside its own transaction
//! (`dining_table` or `table_section`), and every one except a reorder writes
//! its audit row there too, so a change and its record commit together or not
//! at all. Three rules are worth knowing before reading it:
//!
//! * **No open visit ever sits on an archived table.** Archiving a table locks
//!   its row `FOR UPDATE` before looking for an open visit; opening or moving a
//!   visit locks the table `FOR SHARE` (`service::lock_live_table`). The two
//!   lock modes conflict, so whichever runs second sees what the first did.
//! * **No live table ever sits in an archived section.** Archiving a section
//!   locks its row `FOR UPDATE` before counting live tables; creating, moving,
//!   and restoring a table lock the target section `FOR SHARE` and require it
//!   live. The same pairing as a menu category and its dishes.
//! * **A range and a section restore write every row or none.** Both check the
//!   labels first, then write all the rows in one statement. A label a
//!   colleague takes between the check and the write fails that statement, and
//!   the caller is told with an empty [`ConflictKind::LabelsTaken`], so it can
//!   roll back and look the clash up again in a fresh transaction (Postgres
//!   refuses every further statement in a transaction after a failed one).
//!
//! A live table whose section was archived before this feature existed belongs
//! to the no section group, on both floors. Such rows cannot be made any more,
//! but nothing here assumes they are gone.

use chrono::{DateTime, Utc};
use serde_json::json;
use uuid::Uuid;

use crate::domain::audit::AuditAction;
use crate::domain::catalog::{AdminFloorTable, ArchivedTable, DiningTable, TableSection};
use crate::domain::error::{ConflictKind, DomainError, DomainResult};
use crate::domain::event::EntityKind;
use crate::domain::floor;
use crate::domain::ids::{DiningTableId, StaffId, TableSectionId};
use crate::domain::menu;

use super::super::{Database, ScopedTx};
use super::{audit, conflict_on, violated_constraint};

/// The name of the partial unique index that keeps live section names apart.
const SECTION_NAME_KEY: &str = "table_sections_live_name_key";

/// The name of the partial unique index that keeps live table labels apart.
const TABLE_LABEL_KEY: &str = "dining_tables_live_label_key";

/// Builds a [`TableSection`] from any row that selected the section's own
/// columns.
///
/// A macro rather than a function because every `query!` returns its own
/// anonymous record type.
macro_rules! section_from {
    ($row:expr) => {
        TableSection {
            id: TableSectionId::from_uuid($row.id),
            name: $row.name,
            position: $row.position,
            version: $row.version,
            archived_at: $row.archived_at,
        }
    };
}

/// Builds a [`DiningTable`] the same way.
macro_rules! table_from {
    ($row:expr) => {
        DiningTable {
            id: DiningTableId::from_uuid($row.id),
            section_id: $row.section_id.map(TableSectionId::from_uuid),
            label: $row.label,
            seats: $row.seats,
            position: $row.position,
            version: $row.version,
            archived_at: $row.archived_at,
        }
    };
}

/// Trims a name and refuses an empty one.
///
/// The handlers check every name against the full rule as a field error first.
/// This is the backstop for a caller that did not, such as the seed, so a blank
/// name is refused by name rather than by a constraint that reads as the
/// database being unavailable.
fn require_name<'a>(value: &'a str, complaint: &'static str) -> DomainResult<&'a str> {
    let trimmed = value.trim();

    if trimmed.is_empty() {
        return Err(DomainError::Invalid(complaint.to_owned()));
    }

    Ok(trimmed)
}

// ===========================================================================
// Reads
// ===========================================================================

/// The live sections, in displayed order.
///
/// Position first, then name, then id, so two sections that share a position
/// still come back in the same order on every screen and every refetch.
///
/// # Errors
///
/// Returns [`DomainError::Unavailable`] if the read fails.
pub async fn live_table_sections(tx: &mut ScopedTx<'_>) -> DomainResult<Vec<TableSection>> {
    let rows = sqlx::query!(
        r#"
        SELECT id, name, position, version, archived_at
        FROM table_sections
        WHERE archived_at IS NULL
        ORDER BY position, name, id
        "#
    )
    .fetch_all(tx.connection())
    .await?;

    Ok(rows.into_iter().map(|row| section_from!(row)).collect())
}

/// The live tables, flat, in displayed order within their groups.
///
/// # Errors
///
/// Returns [`DomainError::Unavailable`] if the read fails.
pub async fn live_dining_tables(tx: &mut ScopedTx<'_>) -> DomainResult<Vec<DiningTable>> {
    let rows = sqlx::query!(
        r#"
        SELECT id, section_id, label, seats, position, version, archived_at
        FROM dining_tables
        WHERE archived_at IS NULL
        ORDER BY position, label, id
        "#
    )
    .fetch_all(tx.connection())
    .await?;

    Ok(rows.into_iter().map(|row| table_from!(row)).collect())
}

/// The live tables with whether each has a party at it, for the admin's floor.
///
/// Meant to run in the same snapshot as the section reads, so a table opened
/// between two statements cannot be shown free in one place and taken in
/// another.
///
/// # Errors
///
/// Returns [`DomainError::Unavailable`] if the read fails.
pub async fn live_tables_with_occupancy(
    tx: &mut ScopedTx<'_>,
) -> DomainResult<Vec<AdminFloorTable>> {
    let rows = sqlx::query!(
        r#"
        SELECT t.id, t.section_id, t.label, t.seats, t.position, t.version, t.archived_at,
               EXISTS (
                   SELECT 1 FROM visits AS v
                   WHERE v.table_id = t.id AND v.status = 'open'
               ) AS "occupied!"
        FROM dining_tables AS t
        WHERE t.archived_at IS NULL
        ORDER BY t.position, t.label, t.id
        "#
    )
    .fetch_all(tx.connection())
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| {
            let occupied = row.occupied;

            AdminFloorTable {
                table: table_from!(row),
                occupied,
            }
        })
        .collect())
}

/// Every archived section, most recently removed first.
///
/// For the admin floor's Archived section and nothing else.
///
/// # Errors
///
/// Returns [`DomainError::Unavailable`] if the read fails.
pub async fn archived_table_sections(tx: &mut ScopedTx<'_>) -> DomainResult<Vec<TableSection>> {
    let rows = sqlx::query!(
        r#"
        SELECT id, name, position, version, archived_at
        FROM table_sections
        WHERE archived_at IS NOT NULL
        ORDER BY archived_at DESC, name, id
        "#
    )
    .fetch_all(tx.connection())
    .await?;

    Ok(rows.into_iter().map(|row| section_from!(row)).collect())
}

/// Every archived table, in the order they stood before they were removed,
/// with their old section.
///
/// Ordered by position then label rather than by when they went, because the
/// section restore dialog lists a section's tables in their old order. The
/// admin screen re sorts the flat list itself if it wants another order.
///
/// # Errors
///
/// Returns [`DomainError::Unavailable`] if the read fails.
pub async fn archived_dining_tables(tx: &mut ScopedTx<'_>) -> DomainResult<Vec<ArchivedTable>> {
    let rows = sqlx::query!(
        r#"
        SELECT t.id, t.section_id, t.label, t.seats, t.position, t.version, t.archived_at,
               s.name AS "section_name?",
               (s.id IS NOT NULL AND s.archived_at IS NULL) AS "section_live!"
        FROM dining_tables AS t
        LEFT JOIN table_sections AS s ON s.id = t.section_id
        WHERE t.archived_at IS NOT NULL
        ORDER BY t.position, t.label, t.id
        "#
    )
    .fetch_all(tx.connection())
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| {
            let section_name = row.section_name;
            let section_live = row.section_live;

            ArchivedTable {
                table: table_from!(row),
                section_name,
                section_live,
            }
        })
        .collect())
}

/// One table, live or archived.
///
/// # Errors
///
/// Returns [`DomainError::NotFound`] if no such table belongs to this
/// restaurant.
pub async fn dining_table(
    tx: &mut ScopedTx<'_>,
    table_id: DiningTableId,
) -> DomainResult<DiningTable> {
    let row = sqlx::query!(
        r#"
        SELECT id, section_id, label, seats, position, version, archived_at
        FROM dining_tables
        WHERE id = $1
        "#,
        table_id.as_uuid()
    )
    .fetch_optional(tx.connection())
    .await?
    .ok_or(DomainError::NotFound)?;

    Ok(table_from!(row))
}

/// Which of `wanted` would clash with a live table, or with an earlier label
/// in `wanted`, in `wanted`'s order and spelling.
///
/// Used before a range or a section restore writes anything, and again, in a
/// fresh transaction, after a late clash made the write fail.
///
/// # Errors
///
/// Returns [`DomainError::Unavailable`] if the read fails.
pub async fn clashing_labels(
    tx: &mut ScopedTx<'_>,
    wanted: &[String],
) -> DomainResult<Vec<String>> {
    let lowered: Vec<String> = wanted.iter().map(|label| label.to_lowercase()).collect();

    let live: Vec<String> = sqlx::query_scalar!(
        r#"
        SELECT label
        FROM dining_tables
        WHERE archived_at IS NULL AND lower(label) = ANY($1::text[])
        "#,
        &lowered,
    )
    .fetch_all(tx.connection())
    .await?;

    Ok(floor::clashing_labels(wanted, &live))
}

/// The labels of some archived tables of one section, in their old order.
///
/// What a section restore would bring back, read again after a late clash so
/// the handler can say which labels were meant.
///
/// # Errors
///
/// Returns [`DomainError::Unavailable`] if the read fails.
pub async fn archived_labels_in_section(
    tx: &mut ScopedTx<'_>,
    section_id: TableSectionId,
    table_ids: &[Uuid],
) -> DomainResult<Vec<String>> {
    let labels = sqlx::query_scalar!(
        r#"
        SELECT label
        FROM dining_tables
        WHERE section_id = $1 AND archived_at IS NOT NULL AND id = ANY($2::uuid[])
        ORDER BY position, label, id
        "#,
        section_id.as_uuid(),
        table_ids,
    )
    .fetch_all(tx.connection())
    .await?;

    Ok(labels)
}

// ===========================================================================
// Positions and locks
// ===========================================================================

/// Where a new section goes: after the last live one.
///
/// Positions need not be contiguous and need not be unique. Two admins adding a
/// section at the same instant both land on the same number, and the name
/// breaks the tie in every read.
async fn next_section_position(tx: &mut ScopedTx<'_>) -> DomainResult<i32> {
    let row = sqlx::query!(
        r#"
        SELECT coalesce(max(position), 0) + 1 AS "next!"
        FROM table_sections
        WHERE archived_at IS NULL
        "#
    )
    .fetch_one(tx.connection())
    .await?;

    Ok(row.next)
}

/// Where a table arriving in a group goes: after its last live table.
///
/// `IS NOT DISTINCT FROM`, so the no section group, whose `section_id` is null,
/// is a group like any other.
async fn next_table_position(
    tx: &mut ScopedTx<'_>,
    section_id: Option<TableSectionId>,
) -> DomainResult<i32> {
    let row = sqlx::query!(
        r#"
        SELECT coalesce(max(position), 0) + 1 AS "next!"
        FROM dining_tables
        WHERE section_id IS NOT DISTINCT FROM $1 AND archived_at IS NULL
        "#,
        section_id.map(TableSectionId::as_uuid),
    )
    .fetch_one(tx.connection())
    .await?;

    Ok(row.next)
}

/// Locks the section a table is about to be put into, and refuses unless it is
/// live. No section at all needs no lock.
///
/// `FOR SHARE`, which conflicts with the `FOR UPDATE` a section archive takes,
/// so a table arriving and the section going cannot interleave.
///
/// # Errors
///
/// Returns [`DomainError::NotFound`] for a section this restaurant does not
/// have, and [`ConflictKind::SectionArchived`] for one that has been archived.
async fn lock_live_section_for_a_table(
    tx: &mut ScopedTx<'_>,
    section_id: Option<TableSectionId>,
) -> DomainResult<()> {
    let Some(section_id) = section_id else {
        return Ok(());
    };

    let row = sqlx::query!(
        "SELECT archived_at FROM table_sections WHERE id = $1 FOR SHARE",
        section_id.as_uuid()
    )
    .fetch_optional(tx.connection())
    .await?
    .ok_or(DomainError::NotFound)?;

    if row.archived_at.is_some() {
        return Err(DomainError::Conflict(ConflictKind::SectionArchived));
    }

    Ok(())
}

/// A failed multi row write, told apart: a late label clash, or anything else.
fn late_label_clash(error: sqlx::Error) -> DomainError {
    if violated_constraint(&error) == Some(TABLE_LABEL_KEY) {
        return DomainError::Conflict(ConflictKind::LabelsTaken(Vec::new()));
    }

    DomainError::from(error)
}

// ===========================================================================
// Sections
// ===========================================================================

/// Adds a section at the end of the section list.
///
/// # Errors
///
/// Returns [`DomainError::Invalid`] if the name is blank,
/// [`ConflictKind::NameTaken`] if a live section already has that name in any
/// letter case, and [`DomainError::Unavailable`] if a statement fails.
pub async fn create_table_section(
    tx: &mut ScopedTx<'_>,
    name: &str,
    actor: StaffId,
) -> DomainResult<TableSection> {
    let name = require_name(name, "a table section needs a name")?;
    let position = next_section_position(tx).await?;
    let id = TableSectionId::new();

    sqlx::query!(
        r#"
        INSERT INTO table_sections (id, restaurant_id, name, position, updated_at)
        VALUES ($1, $2, $3, $4, now())
        "#,
        id.as_uuid(),
        tx.restaurant_id().as_uuid(),
        name,
        position,
    )
    .execute(tx.connection())
    .await
    .map_err(|error| conflict_on(error, SECTION_NAME_KEY, ConflictKind::NameTaken))?;

    audit::record(
        tx,
        Some(actor),
        AuditAction::TableSectionCreated,
        EntityKind::TableSection.as_label(),
        id.as_uuid(),
        None,
        Some(json!({ "name": name, "position": position })),
    )
    .await?;

    Database::notify_entity_change(tx, EntityKind::TableSection, id.as_uuid()).await?;

    Ok(TableSection {
        id,
        name: name.to_owned(),
        position,
        version: 1,
        archived_at: None,
    })
}

/// Renames a live section, provided nobody has changed it since the form
/// loaded `version`.
///
/// # Errors
///
/// Returns [`ConflictKind::SectionChanged`] if the stored version is newer,
/// [`DomainError::NotFound`] if no such live section belongs to this
/// restaurant, [`ConflictKind::NameTaken`] if a live section already has the new
/// name, and [`DomainError::Invalid`] if the name is blank.
pub async fn rename_table_section(
    tx: &mut ScopedTx<'_>,
    section_id: TableSectionId,
    name: &str,
    version: i32,
    actor: StaffId,
) -> DomainResult<TableSection> {
    let name = require_name(name, "a table section needs a name")?;

    // The subquery is the row before this statement, for the audit row. It
    // cannot disagree with the row updated: a change meanwhile moved the version
    // and this statement matches nothing.
    let renamed = sqlx::query!(
        r#"
        UPDATE table_sections AS s
        SET name = $2, version = s.version + 1, updated_at = now()
        FROM (SELECT id, name FROM table_sections WHERE id = $1) AS old
        WHERE s.id = old.id AND s.version = $3 AND s.archived_at IS NULL
        RETURNING old.name AS "old_name!", s.position AS "position!", s.version AS "version!"
        "#,
        section_id.as_uuid(),
        name,
        version,
    )
    .fetch_optional(tx.connection())
    .await
    .map_err(|error| conflict_on(error, SECTION_NAME_KEY, ConflictKind::NameTaken))?;

    let Some(row) = renamed else {
        return Err(stale_or_missing_section(tx, section_id).await);
    };

    audit::record(
        tx,
        Some(actor),
        AuditAction::TableSectionRenamed,
        EntityKind::TableSection.as_label(),
        section_id.as_uuid(),
        Some(json!({ "name": row.old_name })),
        Some(json!({ "name": name })),
    )
    .await?;

    Database::notify_entity_change(tx, EntityKind::TableSection, section_id.as_uuid()).await?;

    Ok(TableSection {
        id: section_id,
        name: name.to_owned(),
        position: row.position,
        version: row.version,
        archived_at: None,
    })
}

/// Why a conditional update on a section matched nothing.
///
/// A live row means somebody changed it after the form loaded. No row, or an
/// archived one, means there is nothing left to edit, which reads as not found.
async fn stale_or_missing_section(
    tx: &mut ScopedTx<'_>,
    section_id: TableSectionId,
) -> DomainError {
    let found = sqlx::query!(
        "SELECT archived_at FROM table_sections WHERE id = $1",
        section_id.as_uuid()
    )
    .fetch_optional(tx.connection())
    .await;

    match found {
        Ok(Some(row)) if row.archived_at.is_none() => {
            DomainError::Conflict(ConflictKind::SectionChanged)
        }
        Ok(_) => DomainError::NotFound,
        Err(error) => error.into(),
    }
}

/// Rewrites the section order to exactly the list given.
///
/// Refused unless the list names every live section exactly once. The rows are
/// locked `ORDER BY id` first, a fixed lock order, so two admins reordering at
/// once queue rather than deadlock. Either every position is rewritten or none
/// is. No audit row and no version bump.
///
/// # Errors
///
/// Returns [`ConflictKind::FloorChanged`] if the list is not the live set.
pub async fn reorder_table_sections(
    tx: &mut ScopedTx<'_>,
    ids: &[Uuid],
) -> DomainResult<Vec<TableSection>> {
    let live: Vec<Uuid> = sqlx::query_scalar!(
        "SELECT id FROM table_sections WHERE archived_at IS NULL ORDER BY id FOR UPDATE"
    )
    .fetch_all(tx.connection())
    .await?;

    if !menu::is_the_same_list(ids, &live) {
        return Err(DomainError::Conflict(ConflictKind::FloorChanged));
    }

    sqlx::query!(
        r#"
        UPDATE table_sections AS s
        SET position = o.n::int, updated_at = now()
        FROM unnest($1::uuid[]) WITH ORDINALITY AS o(id, n)
        WHERE s.id = o.id
        "#,
        ids,
    )
    .execute(tx.connection())
    .await?;

    if let Some(first) = ids.first() {
        Database::notify_entity_change(tx, EntityKind::TableSection, *first).await?;
    }

    live_table_sections(tx).await
}

/// Takes a section off the floor without deleting it.
///
/// Refused while it still holds a live table. The row is locked `FOR UPDATE`
/// before the tables are counted, which is what stops a table being created in
/// it, moved into it, or restored into it between the count and the archive.
///
/// # Errors
///
/// Returns [`DomainError::NotFound`] if no such live section belongs to this
/// restaurant, and [`ConflictKind::SectionNotEmpty`] if it holds a live table.
pub async fn archive_table_section(
    tx: &mut ScopedTx<'_>,
    section_id: TableSectionId,
    actor: StaffId,
) -> DomainResult<TableSection> {
    sqlx::query!(
        "SELECT id FROM table_sections WHERE id = $1 AND archived_at IS NULL FOR UPDATE",
        section_id.as_uuid()
    )
    .fetch_optional(tx.connection())
    .await?
    .ok_or(DomainError::NotFound)?;

    let holds_a_table = sqlx::query_scalar!(
        r#"
        SELECT EXISTS (
            SELECT 1 FROM dining_tables WHERE section_id = $1 AND archived_at IS NULL
        ) AS "holds!"
        "#,
        section_id.as_uuid()
    )
    .fetch_one(tx.connection())
    .await?;

    if holds_a_table {
        return Err(DomainError::Conflict(ConflictKind::SectionNotEmpty));
    }

    let row = sqlx::query!(
        r#"
        UPDATE table_sections
        SET archived_at = now(), version = version + 1, updated_at = now()
        WHERE id = $1
        RETURNING id, name, position, version, archived_at
        "#,
        section_id.as_uuid()
    )
    .fetch_one(tx.connection())
    .await?;

    let archived = section_from!(row);

    audit::record(
        tx,
        Some(actor),
        AuditAction::TableSectionArchived,
        EntityKind::TableSection.as_label(),
        section_id.as_uuid(),
        Some(json!({ "archived_at": Option::<DateTime<Utc>>::None })),
        Some(json!({ "archived_at": archived.archived_at })),
    )
    .await?;

    Database::notify_entity_change(tx, EntityKind::TableSection, section_id.as_uuid()).await?;

    Ok(archived)
}

/// Puts an archived section back at the end of the section list, with the
/// chosen archived tables that were in it.
///
/// The tables come back into it, in their old order, after any table it
/// already holds. Everything happens in the caller's one transaction: the
/// section and every table, or nothing.
///
/// # Errors
///
/// Returns [`DomainError::NotFound`] if no such archived section belongs to
/// this restaurant, [`DomainError::Invalid`] if an id is not an archived table
/// of this section or is named twice, [`ConflictKind::NameTaken`] if a live
/// section now has its name, and [`ConflictKind::LabelsTaken`] if a chosen
/// table's label is taken. That last one is empty when the clash was only
/// found by the write itself; see the module header.
pub async fn restore_table_section(
    tx: &mut ScopedTx<'_>,
    section_id: TableSectionId,
    table_ids: &[Uuid],
    actor: StaffId,
) -> DomainResult<(TableSection, Vec<DiningTable>)> {
    // Locked so a second restore of the same section waits rather than
    // restoring the same tables twice.
    let before = sqlx::query!(
        r#"
        SELECT archived_at, position
        FROM table_sections
        WHERE id = $1
        FOR UPDATE
        "#,
        section_id.as_uuid()
    )
    .fetch_optional(tx.connection())
    .await?
    .ok_or(DomainError::NotFound)?;

    let Some(old_archived_at) = before.archived_at else {
        return Err(DomainError::NotFound);
    };

    // Every archived table of this section, in old order and locked, so the
    // chosen ones can be checked against it and none can be restored twice.
    let candidates = sqlx::query!(
        r#"
        SELECT id, label, archived_at AS "archived_at!"
        FROM dining_tables
        WHERE section_id = $1 AND archived_at IS NOT NULL
        ORDER BY position, label, id
        FOR UPDATE
        "#,
        section_id.as_uuid()
    )
    .fetch_all(tx.connection())
    .await?;

    let mut unique = table_ids.to_vec();
    unique.sort_unstable();
    unique.dedup();

    let chosen: Vec<_> = candidates
        .into_iter()
        .filter(|row| unique.binary_search(&row.id).is_ok())
        .collect();

    if unique.len() != table_ids.len() || chosen.len() != table_ids.len() {
        return Err(DomainError::Invalid(
            "every table must be an archived table of this section, named once".to_owned(),
        ));
    }

    let labels: Vec<String> = chosen.iter().map(|row| row.label.clone()).collect();
    let clashes = clashing_labels(tx, &labels).await?;

    let position = next_section_position(tx).await?;

    let row = sqlx::query!(
        r#"
        UPDATE table_sections
        SET archived_at = NULL, position = $2, version = version + 1, updated_at = now()
        WHERE id = $1
        RETURNING id, name, position, version, archived_at
        "#,
        section_id.as_uuid(),
        position,
    )
    .fetch_one(tx.connection())
    .await
    .map_err(|error| conflict_on(error, SECTION_NAME_KEY, ConflictKind::NameTaken))?;

    // After the section, so a name clash is what the admin hears first: the
    // section is the thing they asked to bring back.
    if !clashes.is_empty() {
        return Err(DomainError::Conflict(ConflictKind::LabelsTaken(clashes)));
    }

    let restored_section = section_from!(row);

    audit::record(
        tx,
        Some(actor),
        AuditAction::TableSectionRestored,
        EntityKind::TableSection.as_label(),
        section_id.as_uuid(),
        Some(json!({ "archived_at": old_archived_at, "position": before.position })),
        Some(json!({ "archived_at": Option::<DateTime<Utc>>::None, "position": position })),
    )
    .await?;

    Database::notify_entity_change(tx, EntityKind::TableSection, section_id.as_uuid()).await?;

    let ids: Vec<Uuid> = chosen.iter().map(|row| row.id).collect();
    let tables = restore_tables_in_order(tx, Some(section_id), &ids).await?;

    for (table, old) in tables.iter().zip(&chosen) {
        audit::record(
            tx,
            Some(actor),
            AuditAction::DiningTableRestored,
            EntityKind::DiningTable.as_label(),
            table.id.as_uuid(),
            Some(json!({ "archived_at": old.archived_at, "section_id": section_id })),
            Some(json!({
                "archived_at": Option::<DateTime<Utc>>::None,
                "section_id": section_id,
                "position": table.position,
            })),
        )
        .await?;

        Database::notify_entity_change(tx, EntityKind::DiningTable, table.id.as_uuid()).await?;
    }

    Ok((restored_section, tables))
}

/// Brings archived tables back into one group, in the order given, after the
/// group's last live table, in one statement.
async fn restore_tables_in_order(
    tx: &mut ScopedTx<'_>,
    section_id: Option<TableSectionId>,
    ids: &[Uuid],
) -> DomainResult<Vec<DiningTable>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }

    let start = next_table_position(tx, section_id).await?;

    let rows = sqlx::query!(
        r#"
        UPDATE dining_tables AS t
        SET archived_at = NULL, section_id = $2, position = ($3::int + o.n - 1)::int,
            version = t.version + 1, updated_at = now()
        FROM unnest($1::uuid[]) WITH ORDINALITY AS o(id, n)
        WHERE t.id = o.id AND t.archived_at IS NOT NULL
        RETURNING t.id, t.section_id, t.label, t.seats, t.position, t.version, t.archived_at
        "#,
        ids,
        section_id.map(TableSectionId::as_uuid),
        start,
    )
    .fetch_all(tx.connection())
    .await
    .map_err(late_label_clash)?;

    let mut tables: Vec<DiningTable> = rows.into_iter().map(|row| table_from!(row)).collect();
    tables.sort_by_key(|table| table.position);

    Ok(tables)
}

// ===========================================================================
// Tables
// ===========================================================================

/// What adding one table asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewTable<'a> {
    /// Which live section it goes in, or none.
    pub section_id: Option<TableSectionId>,
    /// What the staff call it.
    pub label: &'a str,
    /// How many it seats, when known.
    pub seats: Option<i16>,
}

/// Adds one table at the end of its group.
///
/// # Errors
///
/// Returns [`DomainError::Invalid`] if the label is blank,
/// [`DomainError::NotFound`] for a section this restaurant does not have,
/// [`ConflictKind::SectionArchived`] for one that has been archived,
/// [`ConflictKind::NameTaken`] if a live table already has that label in any
/// letter case, and [`DomainError::Unavailable`] if a statement fails.
pub async fn create_dining_table(
    tx: &mut ScopedTx<'_>,
    table: &NewTable<'_>,
    actor: StaffId,
) -> DomainResult<DiningTable> {
    let label = require_name(table.label, "a table needs a label")?;

    lock_live_section_for_a_table(tx, table.section_id).await?;
    let position = next_table_position(tx, table.section_id).await?;
    let id = DiningTableId::new();

    sqlx::query!(
        r#"
        INSERT INTO dining_tables
            (id, restaurant_id, section_id, label, seats, position, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, now())
        "#,
        id.as_uuid(),
        tx.restaurant_id().as_uuid(),
        table.section_id.map(TableSectionId::as_uuid),
        label,
        table.seats,
        position,
    )
    .execute(tx.connection())
    .await
    .map_err(|error| conflict_on(error, TABLE_LABEL_KEY, ConflictKind::NameTaken))?;

    audit::record(
        tx,
        Some(actor),
        AuditAction::DiningTableCreated,
        EntityKind::DiningTable.as_label(),
        id.as_uuid(),
        None,
        Some(json!({
            "section_id": table.section_id,
            "label": label,
            "seats": table.seats,
            "position": position,
        })),
    )
    .await?;

    // Every waiter's floor is holding the room this changes.
    Database::notify_entity_change(tx, EntityKind::DiningTable, id.as_uuid()).await?;

    Ok(DiningTable {
        id,
        section_id: table.section_id,
        label: label.to_owned(),
        seats: table.seats,
        position,
        version: 1,
        archived_at: None,
    })
}

/// What adding a numbered range asks for, once its labels are built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewTableRange<'a> {
    /// Which live section every table goes in, or none.
    pub section_id: Option<TableSectionId>,
    /// Every label, in number order.
    pub labels: &'a [String],
    /// How many each one seats, when known.
    pub seats: Option<i16>,
}

/// Adds a numbered range of tables at the end of their group, in number
/// order, every one or none.
///
/// # Errors
///
/// Returns [`DomainError::NotFound`] for a section this restaurant does not
/// have, [`ConflictKind::SectionArchived`] for one that has been archived, and
/// [`ConflictKind::LabelsTaken`] listing each label a live table already has.
/// That list is empty when the clash was only found by the insert itself; see
/// the module header.
pub async fn create_table_range(
    tx: &mut ScopedTx<'_>,
    range: &NewTableRange<'_>,
    actor: StaffId,
) -> DomainResult<Vec<DiningTable>> {
    if range.labels.is_empty() {
        return Ok(Vec::new());
    }

    lock_live_section_for_a_table(tx, range.section_id).await?;

    let clashes = clashing_labels(tx, range.labels).await?;

    if !clashes.is_empty() {
        return Err(DomainError::Conflict(ConflictKind::LabelsTaken(clashes)));
    }

    let start = next_table_position(tx, range.section_id).await?;
    let ids: Vec<Uuid> = range
        .labels
        .iter()
        .map(|_| DiningTableId::new().as_uuid())
        .collect();

    let rows = sqlx::query!(
        r#"
        INSERT INTO dining_tables
            (id, restaurant_id, section_id, label, seats, position, updated_at)
        SELECT o.id, $3, $4, o.label, $5, ($6::int + o.n - 1)::int, now()
        FROM unnest($1::uuid[], $2::text[]) WITH ORDINALITY AS o(id, label, n)
        RETURNING id, section_id, label, seats, position, version, archived_at
        "#,
        &ids,
        range.labels,
        tx.restaurant_id().as_uuid(),
        range.section_id.map(TableSectionId::as_uuid),
        range.seats,
        start,
    )
    .fetch_all(tx.connection())
    .await
    .map_err(late_label_clash)?;

    let mut tables: Vec<DiningTable> = rows.into_iter().map(|row| table_from!(row)).collect();
    tables.sort_by_key(|table| table.position);

    for table in &tables {
        audit::record(
            tx,
            Some(actor),
            AuditAction::DiningTableCreated,
            EntityKind::DiningTable.as_label(),
            table.id.as_uuid(),
            None,
            Some(json!({
                "section_id": table.section_id,
                "label": table.label,
                "seats": table.seats,
                "position": table.position,
            })),
        )
        .await?;

        Database::notify_entity_change(tx, EntityKind::DiningTable, table.id.as_uuid()).await?;
    }

    Ok(tables)
}

/// What an edit to a table may change, and which version it was made against.
///
/// Every field is supplied, so an edit is a statement of what the table should
/// be rather than a patch whose omissions mean two different things.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableEdit {
    /// Which live section it should be in, or none. A different group moves it
    /// to the end of that group.
    pub section_id: Option<TableSectionId>,
    /// What the staff should call it.
    pub label: String,
    /// How many it seats, or none.
    pub seats: Option<i16>,
    /// The version the edit form loaded.
    pub version: i32,
}

/// Edits a live table, and writes down what it was before.
///
/// Allowed while a party sits at it: nothing here touches a visit, and every
/// screen reads the label live. The target section is checked before the
/// version, so an edit both stale and aimed at a removed section reports the
/// section.
///
/// # Errors
///
/// Returns [`ConflictKind::SectionArchived`] if the target section has been
/// archived, [`ConflictKind::TableChanged`] if the stored version is newer,
/// [`DomainError::NotFound`] if no such live table or section belongs to this
/// restaurant, [`ConflictKind::NameTaken`] if a live table already has the new
/// label, and [`DomainError::Invalid`] for a blank label.
pub async fn edit_dining_table(
    tx: &mut ScopedTx<'_>,
    table_id: DiningTableId,
    edit: &TableEdit,
    actor: StaffId,
) -> DomainResult<DiningTable> {
    let label = require_name(&edit.label, "a table needs a label")?;

    lock_live_section_for_a_table(tx, edit.section_id).await?;
    let end_of_target = next_table_position(tx, edit.section_id).await?;
    let section_id = edit.section_id.map(TableSectionId::as_uuid);

    // A table staying in its group keeps its place; one moving goes to the end
    // of the new one.
    let updated = sqlx::query!(
        r#"
        UPDATE dining_tables AS t
        SET section_id = $2, label = $3, seats = $4,
            position = CASE WHEN t.section_id IS NOT DISTINCT FROM $2
                            THEN t.position ELSE $6 END,
            version = t.version + 1, updated_at = now()
        FROM (SELECT id, section_id, label, seats FROM dining_tables WHERE id = $1) AS old
        WHERE t.id = old.id AND t.version = $5 AND t.archived_at IS NULL
        RETURNING old.section_id AS "old_section_id?", old.label AS "old_label!",
                  old.seats AS "old_seats?", t.position AS "position!", t.version AS "version!"
        "#,
        table_id.as_uuid(),
        section_id,
        label,
        edit.seats,
        edit.version,
        end_of_target,
    )
    .fetch_optional(tx.connection())
    .await
    .map_err(|error| conflict_on(error, TABLE_LABEL_KEY, ConflictKind::NameTaken))?;

    let Some(row) = updated else {
        return Err(stale_or_missing_table(tx, table_id).await);
    };

    audit::record(
        tx,
        Some(actor),
        AuditAction::DiningTableEdited,
        EntityKind::DiningTable.as_label(),
        table_id.as_uuid(),
        Some(json!({
            "section_id": row.old_section_id,
            "label": row.old_label,
            "seats": row.old_seats,
        })),
        Some(json!({
            "section_id": section_id,
            "label": label,
            "seats": edit.seats,
        })),
    )
    .await?;

    // The waiter's table screen and the kitchen's tickets show this label too,
    // and the web's fan out for this kind reaches both.
    Database::notify_entity_change(tx, EntityKind::DiningTable, table_id.as_uuid()).await?;

    Ok(DiningTable {
        id: table_id,
        section_id: edit.section_id,
        label: label.to_owned(),
        seats: edit.seats,
        position: row.position,
        version: row.version,
        archived_at: None,
    })
}

/// Why a conditional update on a table matched nothing.
async fn stale_or_missing_table(tx: &mut ScopedTx<'_>, table_id: DiningTableId) -> DomainError {
    match dining_table(tx, table_id).await {
        Ok(found) if found.archived_at.is_none() => {
            DomainError::Conflict(ConflictKind::TableChanged)
        }
        Ok(_) => DomainError::NotFound,
        Err(error) => error,
    }
}

/// Rewrites one group's table order to exactly the list given.
///
/// `None` is the no section group, which also holds any live table whose
/// section was archived before this feature, because that is where both floors
/// show it. A table cannot be dragged into another group: that is a move, made
/// in the edit form where its version is checked.
///
/// # Errors
///
/// Returns [`DomainError::NotFound`] if the named section is not a live one of
/// this restaurant, and [`ConflictKind::FloorChanged`] if the list is not that
/// group's live set.
pub async fn reorder_table_group(
    tx: &mut ScopedTx<'_>,
    section_id: Option<TableSectionId>,
    ids: &[Uuid],
) -> DomainResult<Vec<DiningTable>> {
    let section = section_id.map(TableSectionId::as_uuid);

    if let Some(section) = section {
        sqlx::query!(
            "SELECT id FROM table_sections WHERE id = $1 AND archived_at IS NULL",
            section
        )
        .fetch_optional(tx.connection())
        .await?
        .ok_or(DomainError::NotFound)?;
    }

    let live: Vec<Uuid> = sqlx::query_scalar!(
        r#"
        SELECT t.id
        FROM dining_tables AS t
        LEFT JOIN table_sections AS s ON s.id = t.section_id
        WHERE t.archived_at IS NULL
          AND CASE WHEN $1::uuid IS NULL
                   THEN t.section_id IS NULL OR s.archived_at IS NOT NULL
                   ELSE t.section_id = $1
              END
        ORDER BY t.id
        FOR UPDATE OF t
        "#,
        section,
    )
    .fetch_all(tx.connection())
    .await?;

    if !menu::is_the_same_list(ids, &live) {
        return Err(DomainError::Conflict(ConflictKind::FloorChanged));
    }

    let rows = sqlx::query!(
        r#"
        UPDATE dining_tables AS t
        SET position = o.n::int, updated_at = now()
        FROM unnest($1::uuid[]) WITH ORDINALITY AS o(id, n)
        WHERE t.id = o.id
        RETURNING t.id, t.section_id, t.label, t.seats, t.position, t.version, t.archived_at
        "#,
        ids,
    )
    .fetch_all(tx.connection())
    .await?;

    if let Some(first) = ids.first() {
        Database::notify_entity_change(tx, EntityKind::DiningTable, *first).await?;
    }

    let mut tables: Vec<DiningTable> = rows.into_iter().map(|row| table_from!(row)).collect();
    tables.sort_by(|a, b| {
        (a.position, &a.label, a.id.as_uuid()).cmp(&(b.position, &b.label, b.id.as_uuid()))
    });

    Ok(tables)
}

/// Takes a table off the floor without deleting it.
///
/// Refused while a party sits at it. The row is locked `FOR UPDATE` before the
/// open visit is looked for, and opening a visit locks it `FOR SHARE`, so the
/// two cannot interleave: the database never holds an open visit on a table
/// this archived. Archiving rather than deleting keeps every visit, round, and
/// bill that named it resolving.
///
/// # Errors
///
/// Returns [`DomainError::NotFound`] if no such live table belongs to this
/// restaurant, and [`ConflictKind::TableInUse`] if it has an open visit.
pub async fn archive_dining_table(
    tx: &mut ScopedTx<'_>,
    table_id: DiningTableId,
    actor: StaffId,
) -> DomainResult<DiningTable> {
    sqlx::query!(
        "SELECT id FROM dining_tables WHERE id = $1 AND archived_at IS NULL FOR UPDATE",
        table_id.as_uuid()
    )
    .fetch_optional(tx.connection())
    .await?
    .ok_or(DomainError::NotFound)?;

    // A statement of its own, after the lock, so under read committed it sees a
    // visit whose opener committed while this waited.
    let in_use = sqlx::query_scalar!(
        r#"
        SELECT EXISTS (
            SELECT 1 FROM visits WHERE table_id = $1 AND status = 'open'
        ) AS "in_use!"
        "#,
        table_id.as_uuid()
    )
    .fetch_one(tx.connection())
    .await?;

    if in_use {
        return Err(DomainError::Conflict(ConflictKind::TableInUse));
    }

    let row = sqlx::query!(
        r#"
        UPDATE dining_tables
        SET archived_at = now(), version = version + 1, updated_at = now()
        WHERE id = $1
        RETURNING id, section_id, label, seats, position, version, archived_at
        "#,
        table_id.as_uuid()
    )
    .fetch_one(tx.connection())
    .await?;

    let archived = table_from!(row);

    audit::record(
        tx,
        Some(actor),
        AuditAction::DiningTableArchived,
        EntityKind::DiningTable.as_label(),
        table_id.as_uuid(),
        Some(json!({ "archived_at": Option::<DateTime<Utc>>::None })),
        Some(json!({ "archived_at": archived.archived_at })),
    )
    .await?;

    Database::notify_entity_change(tx, EntityKind::DiningTable, table_id.as_uuid()).await?;

    Ok(archived)
}

/// Puts an archived table back, at the end of a live section or of the no
/// section group. It keeps its label and seats.
///
/// # Errors
///
/// Returns [`ConflictKind::SectionArchived`] if the chosen section has been
/// archived, [`DomainError::NotFound`] if no such archived table or section
/// belongs to this restaurant, and [`ConflictKind::NameTaken`] if a live table
/// now has its label.
pub async fn restore_dining_table(
    tx: &mut ScopedTx<'_>,
    table_id: DiningTableId,
    section_id: Option<TableSectionId>,
    actor: StaffId,
) -> DomainResult<DiningTable> {
    lock_live_section_for_a_table(tx, section_id).await?;
    let position = next_table_position(tx, section_id).await?;

    let row = sqlx::query!(
        r#"
        UPDATE dining_tables AS t
        SET archived_at = NULL, section_id = $2, position = $3,
            version = t.version + 1, updated_at = now()
        FROM (SELECT id, archived_at, section_id FROM dining_tables WHERE id = $1) AS old
        WHERE t.id = old.id AND t.archived_at IS NOT NULL
        RETURNING t.id AS "id!", t.section_id AS "section_id?", t.label AS "label!",
                  t.seats AS "seats?", t.position AS "position!", t.version AS "version!",
                  t.archived_at AS "archived_at?",
                  old.archived_at AS "old_archived_at?", old.section_id AS "old_section_id?"
        "#,
        table_id.as_uuid(),
        section_id.map(TableSectionId::as_uuid),
        position,
    )
    .fetch_optional(tx.connection())
    .await
    .map_err(|error| conflict_on(error, TABLE_LABEL_KEY, ConflictKind::NameTaken))?
    .ok_or(DomainError::NotFound)?;

    audit::record(
        tx,
        Some(actor),
        AuditAction::DiningTableRestored,
        EntityKind::DiningTable.as_label(),
        table_id.as_uuid(),
        Some(json!({
            "archived_at": row.old_archived_at,
            "section_id": row.old_section_id,
        })),
        Some(json!({
            "archived_at": Option::<DateTime<Utc>>::None,
            "section_id": section_id,
            "position": position,
        })),
    )
    .await?;

    Database::notify_entity_change(tx, EntityKind::DiningTable, table_id.as_uuid()).await?;

    Ok(table_from!(row))
}
