//! What a restaurant sets up before it can serve anybody, and the edits to it
//! that are worth writing down.
//!
//! Every working read here filters archived rows out for the caller. That is
//! the whole reason these exist rather than each feature writing its own
//! `SELECT`: an archived dish that disappears from eight screens and reappears
//! on the ninth is the failure this shape prevents. The one reader that wants
//! archived rows is the admin menu's Archived section, and the two reads that
//! serve it say so in their names. A bill reprint reads through the line that
//! copied the name rather than through the menu.
//!
//! The menu section at the foot of the file carries spec 0008's rules, and two
//! of them are worth knowing before reading it:
//!
//! * **No live dish ever sits in an archived category.** Archiving a category
//!   locks its row `FOR UPDATE` before counting its live dishes; creating,
//!   moving, and restoring a dish lock the target category `FOR SHARE` and
//!   require it live. The two lock modes conflict, so the check and the write
//!   cannot interleave.
//! * **A stale edit is refused, never merged.** Every write that changes a row
//!   bumps its `version` in the same statement, and an edit or rename is a
//!   conditional update naming the version its form loaded. Zero rows then means
//!   stale or missing, told apart by one follow up read.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde_json::json;

use uuid::Uuid;

use crate::domain::audit::AuditAction;
use crate::domain::catalog::{
    ArchivedDish, DiningTable, Dish, MenuCategory, Restaurant, TableSection, TaxComponent,
};
use crate::domain::enums::{Diet, StaffRole};
use crate::domain::error::{ConflictKind, DomainError, DomainResult};
use crate::domain::event::EntityKind;
use crate::domain::ids::{
    DiningTableId, DishId, MenuCategoryId, RestaurantId, StaffId, TableSectionId, TaxComponentId,
};
use crate::domain::language::{FormattingLocale, LanguageCode};
use crate::domain::menu;
use crate::domain::money::Currency;
use crate::domain::people::Staff;

use super::super::{Database, ScopedTx};
use super::{audit, conflict_on};

/// Reads the restaurant's own settings.
///
/// Its currency and timezone are the source of every money figure and every
/// local day in the product, so this is the read most other things depend on.
///
/// # Errors
///
/// Returns [`DomainError::NotFound`] if the transaction is scoped to a
/// restaurant that does not exist, and [`DomainError::Invalid`] if the stored
/// currency does not satisfy the domain's own rules.
pub async fn restaurant(tx: &mut ScopedTx<'_>) -> DomainResult<Restaurant> {
    let row = sqlx::query!(
        r#"
        SELECT id, name, country_code, currency_code, currency_decimals, timezone,
               default_language, formatting_locale,
               service_charge_percent, address, tax_registration_number, deactivated_at
        FROM restaurants
        "#
    )
    .fetch_optional(tx.connection())
    .await?
    .ok_or(DomainError::NotFound)?;

    Ok(Restaurant {
        id: RestaurantId::from_uuid(row.id),
        name: row.name,
        // `char(2)` pads with spaces on the way out, the same as the currency
        // code does, so the trim is not optional.
        country_code: row.country_code.trim().to_owned(),
        currency: currency_from(&row.currency_code, row.currency_decimals)?,
        timezone: row.timezone,
        default_language: LanguageCode::new(&row.default_language)?,
        formatting_locale: FormattingLocale::new(&row.formatting_locale)?,
        service_charge_percent: row.service_charge_percent,
        address: row.address,
        tax_registration_number: row.tax_registration_number,
        deactivated_at: row.deactivated_at,
    })
}

/// Builds a [`Currency`] from what the database stored.
///
/// `char(3)` pads with spaces on the way out, so the trim is not optional.
fn currency_from(code: &str, decimals: i16) -> DomainResult<Currency> {
    let decimals = u32::try_from(decimals)
        .map_err(|_| DomainError::Invalid(format!("currency decimals {decimals} is negative")))?;

    Currency::new(code.trim(), decimals)
}

/// Every tax the restaurant currently charges, in printed order.
///
/// Archived components are left out, which is what stops a tax the restaurant
/// stopped charging reappearing on a bill closed tomorrow.
///
/// # Errors
///
/// Returns [`DomainError::Unavailable`] if the read fails.
pub async fn live_tax_components(tx: &mut ScopedTx<'_>) -> DomainResult<Vec<TaxComponent>> {
    let rows = sqlx::query!(
        r#"
        SELECT id, name, rate_percent, position, archived_at
        FROM tax_components
        WHERE archived_at IS NULL
        ORDER BY position, name
        "#
    )
    .fetch_all(tx.connection())
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| TaxComponent {
            id: TaxComponentId::from_uuid(row.id),
            name: row.name,
            rate_percent: row.rate_percent,
            position: row.position,
            archived_at: row.archived_at,
        })
        .collect())
}

/// The floor as a waiter sees it: live sections, in displayed order.
///
/// # Errors
///
/// Returns [`DomainError::Unavailable`] if the read fails.
pub async fn live_table_sections(tx: &mut ScopedTx<'_>) -> DomainResult<Vec<TableSection>> {
    let rows = sqlx::query!(
        r#"
        SELECT id, name, position, archived_at
        FROM table_sections
        WHERE archived_at IS NULL
        ORDER BY position, name
        "#
    )
    .fetch_all(tx.connection())
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| TableSection {
            id: TableSectionId::from_uuid(row.id),
            name: row.name,
            position: row.position,
            archived_at: row.archived_at,
        })
        .collect())
}

/// The floor as a waiter sees it: live tables, in displayed order.
///
/// # Errors
///
/// Returns [`DomainError::Unavailable`] if the read fails.
pub async fn live_dining_tables(tx: &mut ScopedTx<'_>) -> DomainResult<Vec<DiningTable>> {
    let rows = sqlx::query!(
        r#"
        SELECT id, section_id, label, seats, position, archived_at
        FROM dining_tables
        WHERE archived_at IS NULL
        ORDER BY position, label
        "#
    )
    .fetch_all(tx.connection())
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| DiningTable {
            id: DiningTableId::from_uuid(row.id),
            section_id: row.section_id.map(TableSectionId::from_uuid),
            label: row.label,
            seats: row.seats,
            position: row.position,
            archived_at: row.archived_at,
        })
        .collect())
}

/// Everybody who currently works here.
///
/// Deactivated accounts are left out. Their rows stay, so a bill closed by
/// somebody who has since left still says who closed it.
///
/// # Errors
///
/// Returns [`DomainError::Unavailable`] if the read fails.
pub async fn active_staff(tx: &mut ScopedTx<'_>) -> DomainResult<Vec<Staff>> {
    let rows = sqlx::query!(
        r#"
        SELECT id, email, display_name, role AS "role: StaffRole", language, deactivated_at
        FROM staff
        WHERE deactivated_at IS NULL
        ORDER BY display_name
        "#
    )
    .fetch_all(tx.connection())
    .await?;

    // Not `.map(...).collect()`: building a `LanguageCode` validates against
    // the catalogue and so can fail, and a `?` inside a closure would only
    // return from the closure. Collecting into a `DomainResult` keeps the first
    // bad row as the answer for the whole read.
    rows.into_iter()
        .map(|row| {
            Ok(Staff {
                id: StaffId::from_uuid(row.id),
                email: row.email,
                display_name: row.display_name,
                role: row.role,
                language: row.language.as_deref().map(LanguageCode::new).transpose()?,
                deactivated_at: row.deactivated_at,
            })
        })
        .collect()
}

/// Adds a named group of tables, such as a terrace.
///
/// This and the one below exist because a restaurant has to have tables before
/// anybody can order anything, and the seed is deliberately not allowed to write
/// rows of its own: SQL lives in this layer, and a seeded restaurant has to be
/// one the product could have produced. Feature 11 builds the admin screens over
/// exactly these, which is why each takes what an admin form would collect and
/// nothing more.
///
/// Neither invents a position; the caller supplies one. The menu's creates are
/// different, because spec 0008 settled that a new dish or category goes to the
/// end of its list and is moved by dragging from there.
///
/// # Errors
///
/// Returns [`DomainError::Invalid`] if the name is blank, and
/// [`DomainError::Unavailable`] if the statement fails. A second live section of
/// the same name is refused by the partial unique index.
pub async fn create_table_section(
    tx: &mut ScopedTx<'_>,
    name: &str,
    position: i32,
) -> DomainResult<TableSectionId> {
    let name = require_name(name, "a table section needs a name")?;
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
    .await?;

    Ok(id)
}

/// Adds a table somebody can sit at.
///
/// The section is optional, because a restaurant with one room has no use for
/// one. A section belonging to another restaurant is refused by the composite
/// foreign key rather than by a check written here.
///
/// # Errors
///
/// Returns [`DomainError::Invalid`] if the label is blank, and
/// [`DomainError::Unavailable`] if the statement fails.
pub async fn create_dining_table(
    tx: &mut ScopedTx<'_>,
    section_id: Option<TableSectionId>,
    label: &str,
    seats: Option<i16>,
    position: i32,
) -> DomainResult<DiningTableId> {
    let label = require_name(label, "a table needs a label")?;
    let id = DiningTableId::new();

    sqlx::query!(
        r#"
        INSERT INTO dining_tables
            (id, restaurant_id, section_id, label, seats, position, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, now())
        "#,
        id.as_uuid(),
        tx.restaurant_id().as_uuid(),
        section_id.map(TableSectionId::as_uuid),
        label,
        seats,
        position,
    )
    .execute(tx.connection())
    .await?;

    // The floor read is keyed on tables, so a waiter looking at it while an
    // admin adds one has to be told rather than left with a stale room.
    Database::notify_entity_change(tx, EntityKind::DiningTable, id.as_uuid()).await?;

    Ok(id)
}

/// Trims a name and refuses an empty one.
///
/// The database checks this too, with a `not_blank` constraint on every one of
/// these tables. Checking it here as well is what turns a constraint violation
/// that reads to a caller as "the database is unavailable" into a refusal that
/// names what is wrong.
fn require_name<'a>(value: &'a str, complaint: &'static str) -> DomainResult<&'a str> {
    let trimmed = value.trim();

    if trimmed.is_empty() {
        return Err(DomainError::Invalid(complaint.to_owned()));
    }

    Ok(trimmed)
}

/// Takes a table out of use without deleting it.
///
/// # Errors
///
/// Returns [`DomainError::NotFound`] if no such live table belongs to this
/// restaurant.
pub async fn archive_dining_table(
    tx: &mut ScopedTx<'_>,
    table_id: DiningTableId,
) -> DomainResult<DateTime<Utc>> {
    let archived = sqlx::query!(
        r#"
        UPDATE dining_tables
        SET archived_at = now(), updated_at = now()
        WHERE id = $1 AND archived_at IS NULL
        RETURNING archived_at AS "archived_at!"
        "#,
        table_id.as_uuid()
    )
    .fetch_optional(tx.connection())
    .await?
    .map(|row| row.archived_at)
    .ok_or(DomainError::NotFound)?;

    Database::notify_entity_change(tx, EntityKind::DiningTable, table_id.as_uuid()).await?;

    Ok(archived)
}

/// Takes a section out of use without deleting it.
///
/// # Errors
///
/// Returns [`DomainError::NotFound`] if no such live section belongs to this
/// restaurant.
pub async fn archive_table_section(
    tx: &mut ScopedTx<'_>,
    section_id: TableSectionId,
) -> DomainResult<DateTime<Utc>> {
    sqlx::query!(
        r#"
        UPDATE table_sections
        SET archived_at = now(), updated_at = now()
        WHERE id = $1 AND archived_at IS NULL
        RETURNING archived_at AS "archived_at!"
        "#,
        section_id.as_uuid()
    )
    .fetch_optional(tx.connection())
    .await?
    .map(|row| row.archived_at)
    .ok_or(DomainError::NotFound)
}

/// Changes a tax component's name or rate, and writes down that it happened.
///
/// Closed bills are untouched: they carry their own copy of the name and the
/// rate they were charged at.
///
/// # Errors
///
/// Returns [`DomainError::NotFound`] if no such component belongs to this
/// restaurant.
pub async fn update_tax_component(
    tx: &mut ScopedTx<'_>,
    component_id: TaxComponentId,
    name: &str,
    rate_percent: Decimal,
    actor: StaffId,
) -> DomainResult<TaxComponent> {
    let before = sqlx::query!(
        r#"
        SELECT name, rate_percent, position, archived_at
        FROM tax_components
        WHERE id = $1
        FOR UPDATE
        "#,
        component_id.as_uuid()
    )
    .fetch_optional(tx.connection())
    .await?
    .ok_or(DomainError::NotFound)?;

    sqlx::query!(
        r#"
        UPDATE tax_components
        SET name = $2, rate_percent = $3, updated_at = now()
        WHERE id = $1
        "#,
        component_id.as_uuid(),
        name,
        rate_percent,
    )
    .execute(tx.connection())
    .await?;

    audit::record(
        tx,
        Some(actor),
        AuditAction::TaxComponentEdited,
        "tax_component",
        component_id.as_uuid(),
        Some(json!({ "name": before.name, "rate_percent": before.rate_percent })),
        Some(json!({ "name": name, "rate_percent": rate_percent })),
    )
    .await?;

    Ok(TaxComponent {
        id: component_id,
        name: name.to_owned(),
        rate_percent,
        position: before.position,
        archived_at: before.archived_at,
    })
}

/// Changes the restaurant's service charge, and writes down that it happened.
///
/// [`None`] means the restaurant stops charging one, which yields an amount of
/// zero on future bills rather than a null.
///
/// # Errors
///
/// Returns [`DomainError::NotFound`] if the scoped restaurant does not exist.
pub async fn set_service_charge(
    tx: &mut ScopedTx<'_>,
    percent: Option<Decimal>,
    actor: StaffId,
) -> DomainResult<()> {
    let before = sqlx::query!("SELECT service_charge_percent FROM restaurants FOR UPDATE")
        .fetch_optional(tx.connection())
        .await?
        .ok_or(DomainError::NotFound)?;

    let restaurant_id = tx.restaurant_id().as_uuid();

    sqlx::query!(
        "UPDATE restaurants SET service_charge_percent = $1, updated_at = now()",
        percent
    )
    .execute(tx.connection())
    .await?;

    audit::record(
        tx,
        Some(actor),
        AuditAction::ServiceChargeEdited,
        "restaurant",
        restaurant_id,
        Some(json!({ "service_charge_percent": before.service_charge_percent })),
        Some(json!({ "service_charge_percent": percent })),
    )
    .await?;

    Ok(())
}

/// Changes the restaurant's language and formatting settings.
///
/// Both are set together because they are read together and because setting one
/// alone is almost always a mistake: a restaurant switching to Hindi wants its
/// figures in `hi-IN` or `en-IN`, not left on `en-US` because the form only
/// carried one field.
///
/// No audit entry. A language setting carries no personal data and no money, so
/// it is not one of the changes the audit log exists for.
///
/// Neither value can be a code this platform does not offer: the only way to
/// build the arguments is through the catalogue checked constructors, so an
/// unknown code is refused before a statement is ever prepared.
///
/// # Errors
///
/// Returns [`DomainError::NotFound`] if the scoped restaurant does not exist,
/// and [`DomainError::Unavailable`] if the statement fails.
pub async fn set_restaurant_languages(
    tx: &mut ScopedTx<'_>,
    default_language: &LanguageCode,
    formatting_locale: &FormattingLocale,
) -> DomainResult<()> {
    let updated = sqlx::query!(
        r#"
        UPDATE restaurants
        SET default_language = $1, formatting_locale = $2, updated_at = now()
        "#,
        default_language.as_str(),
        formatting_locale.as_str()
    )
    .execute(tx.connection())
    .await?
    .rows_affected();

    if updated == 0 {
        return Err(DomainError::NotFound);
    }

    Ok(())
}

/// Sets, or clears, one staff member's own interface language.
///
/// [`None`] clears it, which puts them back on the restaurant's default. That is
/// a real choice a person makes ("just give me whatever everyone else gets"),
/// not an absence, which is why it is expressed rather than left out.
///
/// Scoped like every other write here, so this can only ever reach a staff row
/// belonging to the transaction's own restaurant. Which staff member a signed in
/// person may write is feature 7's rule, and it is a rule about the caller
/// rather than about the row, so it belongs there and not here.
///
/// # Errors
///
/// Returns [`DomainError::NotFound`] if no such staff member belongs to this
/// restaurant, and [`DomainError::Unavailable`] if the statement fails.
pub async fn set_staff_language(
    tx: &mut ScopedTx<'_>,
    staff_id: StaffId,
    language: Option<&LanguageCode>,
) -> DomainResult<()> {
    let updated = sqlx::query!(
        "UPDATE staff SET language = $1, updated_at = now() WHERE id = $2",
        language.map(LanguageCode::as_str),
        staff_id.as_uuid()
    )
    .execute(tx.connection())
    .await?
    .rows_affected();

    if updated == 0 {
        return Err(DomainError::NotFound);
    }

    Ok(())
}

/// Changes what a staff member is allowed to be, and writes down that it
/// happened.
///
/// # Errors
///
/// Returns [`DomainError::NotFound`] if no such staff member belongs to this
/// restaurant.
pub async fn change_staff_role(
    tx: &mut ScopedTx<'_>,
    staff_id: StaffId,
    role: StaffRole,
    actor: StaffId,
) -> DomainResult<()> {
    let before = sqlx::query!(
        r#"SELECT role AS "role: StaffRole" FROM staff WHERE id = $1 FOR UPDATE"#,
        staff_id.as_uuid()
    )
    .fetch_optional(tx.connection())
    .await?
    .ok_or(DomainError::NotFound)?;

    sqlx::query!(
        "UPDATE staff SET role = $2, updated_at = now() WHERE id = $1",
        staff_id.as_uuid(),
        role as StaffRole,
    )
    .execute(tx.connection())
    .await?;

    audit::record(
        tx,
        Some(actor),
        AuditAction::StaffRoleChanged,
        "staff",
        staff_id.as_uuid(),
        Some(json!({ "role": before.role })),
        Some(json!({ "role": role })),
    )
    .await?;

    Database::notify_entity_change(tx, EntityKind::Staff, staff_id.as_uuid()).await?;

    Ok(())
}

/// Switches a staff member's account off, and writes down that it happened.
///
/// The row stays, so historical bills stay attributable to a real person. Only
/// deleting the whole restaurant removes anything.
///
/// # Errors
///
/// Returns [`DomainError::NotFound`] if no such active staff member belongs to
/// this restaurant.
pub async fn deactivate_staff(
    tx: &mut ScopedTx<'_>,
    staff_id: StaffId,
    actor: StaffId,
) -> DomainResult<DateTime<Utc>> {
    let deactivated = sqlx::query!(
        r#"
        UPDATE staff
        SET deactivated_at = now(), updated_at = now()
        WHERE id = $1 AND deactivated_at IS NULL
        RETURNING deactivated_at
        "#,
        staff_id.as_uuid()
    )
    .fetch_optional(tx.connection())
    .await?
    .and_then(|row| row.deactivated_at)
    .ok_or(DomainError::NotFound)?;

    audit::record(
        tx,
        Some(actor),
        AuditAction::StaffDeactivated,
        "staff",
        staff_id.as_uuid(),
        Some(json!({ "deactivated_at": Option::<DateTime<Utc>>::None })),
        Some(json!({ "deactivated_at": deactivated })),
    )
    .await?;

    Database::notify_entity_change(tx, EntityKind::Staff, staff_id.as_uuid()).await?;

    Ok(deactivated)
}

// ===========================================================================
// The menu
//
// Spec 0008. Everything an admin does to the menu, plus the one switch a chef
// may throw. Every write here notifies inside its own transaction, and every
// one except a reorder writes its audit row there too, so a change and its
// record commit together or not at all.
// ===========================================================================

/// The name of the partial unique index that keeps live category names apart.
const CATEGORY_NAME_KEY: &str = "menu_categories_live_name_key";

/// The name of the partial unique index that keeps live dish names apart.
const DISH_NAME_KEY: &str = "dishes_live_name_key";

/// Builds a [`Dish`] from any row that selected the dish's own columns.
///
/// A macro rather than a function because every `query!` returns its own
/// anonymous record type, and this is the one place the ten fields are named.
macro_rules! dish_from {
    ($row:expr) => {
        Dish {
            id: DishId::from_uuid($row.id),
            category_id: MenuCategoryId::from_uuid($row.category_id),
            name: $row.name,
            description: $row.description,
            price: $row.price,
            diet: $row.diet,
            is_available: $row.is_available,
            position: $row.position,
            version: $row.version,
            archived_at: $row.archived_at,
        }
    };
}

/// Builds a [`MenuCategory`] the same way.
macro_rules! category_from {
    ($row:expr) => {
        MenuCategory {
            id: MenuCategoryId::from_uuid($row.id),
            name: $row.name,
            position: $row.position,
            version: $row.version,
            archived_at: $row.archived_at,
        }
    };
}

/// The live categories, in printed order.
///
/// Position first, then name, then id, so two categories that share a position
/// (two admins adding one at the same moment) still come back in the same order
/// on every screen and every refetch.
///
/// # Errors
///
/// Returns [`DomainError::Unavailable`] if the read fails.
pub async fn live_menu_categories(tx: &mut ScopedTx<'_>) -> DomainResult<Vec<MenuCategory>> {
    let rows = sqlx::query!(
        r#"
        SELECT id, name, position, version, archived_at
        FROM menu_categories
        WHERE archived_at IS NULL
        ORDER BY position, name, id
        "#
    )
    .fetch_all(tx.connection())
    .await?;

    Ok(rows.into_iter().map(|row| category_from!(row)).collect())
}

/// The live dishes, flat, in printed order within their categories.
///
/// Includes dishes marked unavailable, because a waiter's screen shows them
/// greyed rather than hiding them. Ordering one is refused when the round is
/// sent.
///
/// # Errors
///
/// Returns [`DomainError::Unavailable`] if the read fails.
pub async fn live_dishes(tx: &mut ScopedTx<'_>) -> DomainResult<Vec<Dish>> {
    let rows = sqlx::query!(
        r#"
        SELECT id, category_id, name, description, price, diet AS "diet: Diet",
               is_available, position, version, archived_at
        FROM dishes
        WHERE archived_at IS NULL
        ORDER BY position, name, id
        "#
    )
    .fetch_all(tx.connection())
    .await?;

    Ok(rows.into_iter().map(|row| dish_from!(row)).collect())
}

/// Every archived category, most recently removed first.
///
/// For the admin menu's Archived section and nothing else.
///
/// # Errors
///
/// Returns [`DomainError::Unavailable`] if the read fails.
pub async fn archived_menu_categories(tx: &mut ScopedTx<'_>) -> DomainResult<Vec<MenuCategory>> {
    let rows = sqlx::query!(
        r#"
        SELECT id, name, position, version, archived_at
        FROM menu_categories
        WHERE archived_at IS NOT NULL
        ORDER BY archived_at DESC, name, id
        "#
    )
    .fetch_all(tx.connection())
    .await?;

    Ok(rows.into_iter().map(|row| category_from!(row)).collect())
}

/// Every archived dish, most recently removed first, with its old category.
///
/// For the admin menu's Archived section and nothing else. The category comes
/// with its name and whether it is still live, because the restore dialog
/// offers the old category back only when it can take the dish.
///
/// # Errors
///
/// Returns [`DomainError::Unavailable`] if the read fails.
pub async fn archived_dishes(tx: &mut ScopedTx<'_>) -> DomainResult<Vec<ArchivedDish>> {
    let rows = sqlx::query!(
        r#"
        SELECT d.id, d.category_id, d.name, d.description, d.price,
               d.diet AS "diet: Diet", d.is_available, d.position, d.version, d.archived_at,
               c.name AS category_name,
               (c.archived_at IS NULL) AS "category_live!"
        FROM dishes AS d
        JOIN menu_categories AS c ON c.id = d.category_id
        WHERE d.archived_at IS NOT NULL
        ORDER BY d.archived_at DESC, d.name, d.id
        "#
    )
    .fetch_all(tx.connection())
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| {
            let category_name = row.category_name;
            let category_live = row.category_live;

            ArchivedDish {
                dish: dish_from!(row),
                category_name,
                category_live,
            }
        })
        .collect())
}

/// One dish, live or archived.
///
/// # Errors
///
/// Returns [`DomainError::NotFound`] if no such dish belongs to this
/// restaurant.
pub async fn dish(tx: &mut ScopedTx<'_>, dish_id: DishId) -> DomainResult<Dish> {
    let row = sqlx::query!(
        r#"
        SELECT id, category_id, name, description, price, diet AS "diet: Diet",
               is_available, position, version, archived_at
        FROM dishes
        WHERE id = $1
        "#,
        dish_id.as_uuid()
    )
    .fetch_optional(tx.connection())
    .await?
    .ok_or(DomainError::NotFound)?;

    Ok(dish_from!(row))
}

/// Where a new category goes: after the last live one.
///
/// Positions need not be contiguous and need not be unique. Two admins adding a
/// category at the same instant both land on the same number, and the name
/// breaks the tie in every read.
async fn next_category_position(tx: &mut ScopedTx<'_>) -> DomainResult<i32> {
    let row = sqlx::query!(
        r#"
        SELECT coalesce(max(position), 0) + 1 AS "next!"
        FROM menu_categories
        WHERE archived_at IS NULL
        "#
    )
    .fetch_one(tx.connection())
    .await?;

    Ok(row.next)
}

/// Where a dish arriving in a category goes: after its last live dish.
async fn next_dish_position(
    tx: &mut ScopedTx<'_>,
    category_id: MenuCategoryId,
) -> DomainResult<i32> {
    let row = sqlx::query!(
        r#"
        SELECT coalesce(max(position), 0) + 1 AS "next!"
        FROM dishes
        WHERE category_id = $1 AND archived_at IS NULL
        "#,
        category_id.as_uuid()
    )
    .fetch_one(tx.connection())
    .await?;

    Ok(row.next)
}

/// Locks the category a dish is about to be put into, and refuses unless it is
/// live.
///
/// `FOR SHARE`, which conflicts with the `FOR UPDATE` an archive takes. So a
/// dish being created in, moved into, or restored into a category and that
/// category being archived at the same instant are serialised: whichever comes
/// second sees what the first did, and no live dish ever ends up under an
/// archived heading.
///
/// # Errors
///
/// Returns [`DomainError::NotFound`] for a category this restaurant does not
/// have, and [`ConflictKind::CategoryArchived`] for one that has been archived.
async fn lock_live_category_for_a_dish(
    tx: &mut ScopedTx<'_>,
    category_id: MenuCategoryId,
) -> DomainResult<()> {
    let row = sqlx::query!(
        "SELECT archived_at FROM menu_categories WHERE id = $1 FOR SHARE",
        category_id.as_uuid()
    )
    .fetch_optional(tx.connection())
    .await?
    .ok_or(DomainError::NotFound)?;

    if row.archived_at.is_some() {
        return Err(DomainError::Conflict(ConflictKind::CategoryArchived));
    }

    Ok(())
}

/// Adds a group of dishes, such as starters, at the end of the category list.
///
/// # Errors
///
/// Returns [`DomainError::Invalid`] if the name is blank,
/// [`ConflictKind::NameTaken`] if a live category already has that name in any
/// letter case, and [`DomainError::Unavailable`] if a statement fails.
pub async fn create_menu_category(
    tx: &mut ScopedTx<'_>,
    name: &str,
    actor: StaffId,
) -> DomainResult<MenuCategory> {
    let name = require_name(name, "a menu category needs a name")?;
    let position = next_category_position(tx).await?;
    let id = MenuCategoryId::new();

    sqlx::query!(
        r#"
        INSERT INTO menu_categories (id, restaurant_id, name, position, updated_at)
        VALUES ($1, $2, $3, $4, now())
        "#,
        id.as_uuid(),
        tx.restaurant_id().as_uuid(),
        name,
        position,
    )
    .execute(tx.connection())
    .await
    .map_err(|error| conflict_on(error, CATEGORY_NAME_KEY, ConflictKind::NameTaken))?;

    audit::record(
        tx,
        Some(actor),
        AuditAction::MenuCategoryCreated,
        EntityKind::MenuCategory.as_label(),
        id.as_uuid(),
        None,
        Some(json!({ "name": name, "position": position })),
    )
    .await?;

    Database::notify_entity_change(tx, EntityKind::MenuCategory, id.as_uuid()).await?;

    Ok(MenuCategory {
        id,
        name: name.to_owned(),
        position,
        version: 1,
        archived_at: None,
    })
}

/// Renames a live category, provided nobody has changed it since the form
/// loaded `version`.
///
/// # Errors
///
/// Returns [`ConflictKind::CategoryChanged`] if the stored version is newer,
/// [`DomainError::NotFound`] if no such live category belongs to this
/// restaurant, [`ConflictKind::NameTaken`] if a live category already has the
/// new name, and [`DomainError::Invalid`] if the name is blank.
pub async fn rename_menu_category(
    tx: &mut ScopedTx<'_>,
    category_id: MenuCategoryId,
    name: &str,
    version: i32,
    actor: StaffId,
) -> DomainResult<MenuCategory> {
    let name = require_name(name, "a menu category needs a name")?;

    // The subquery is what the row looked like before this statement, which is
    // what the audit row records. Reading it in the same statement rather than
    // before it costs no extra round trip, and it cannot disagree with the row
    // that was updated: if anything changed the row meanwhile, its version moved
    // and this statement matches nothing.
    let renamed = sqlx::query!(
        r#"
        UPDATE menu_categories AS c
        SET name = $2, version = c.version + 1, updated_at = now()
        FROM (SELECT id, name FROM menu_categories WHERE id = $1) AS old
        WHERE c.id = old.id AND c.version = $3 AND c.archived_at IS NULL
        RETURNING old.name AS "old_name!", c.position AS "position!", c.version AS "version!"
        "#,
        category_id.as_uuid(),
        name,
        version,
    )
    .fetch_optional(tx.connection())
    .await
    .map_err(|error| conflict_on(error, CATEGORY_NAME_KEY, ConflictKind::NameTaken))?;

    let Some(row) = renamed else {
        return Err(stale_or_missing_category(tx, category_id).await);
    };

    audit::record(
        tx,
        Some(actor),
        AuditAction::MenuCategoryRenamed,
        EntityKind::MenuCategory.as_label(),
        category_id.as_uuid(),
        Some(json!({ "name": row.old_name })),
        Some(json!({ "name": name })),
    )
    .await?;

    Database::notify_entity_change(tx, EntityKind::MenuCategory, category_id.as_uuid()).await?;

    Ok(MenuCategory {
        id: category_id,
        name: name.to_owned(),
        position: row.position,
        version: row.version,
        archived_at: None,
    })
}

/// Why a conditional update on a category matched nothing.
///
/// A live row means somebody changed it after the form loaded. No row, or an
/// archived one, means there is nothing left to edit, which reads as not found.
async fn stale_or_missing_category(
    tx: &mut ScopedTx<'_>,
    category_id: MenuCategoryId,
) -> DomainError {
    let found = sqlx::query!(
        "SELECT archived_at FROM menu_categories WHERE id = $1",
        category_id.as_uuid()
    )
    .fetch_optional(tx.connection())
    .await;

    match found {
        Ok(Some(row)) if row.archived_at.is_none() => {
            DomainError::Conflict(ConflictKind::CategoryChanged)
        }
        Ok(_) => DomainError::NotFound,
        Err(error) => error.into(),
    }
}

/// Rewrites the category order to exactly the list given.
///
/// Refused unless the list names every live category exactly once, which is
/// what catches a category added or removed while the admin was dragging. The
/// siblings are locked `ORDER BY id` first, a fixed lock order, so two admins
/// reordering at once queue rather than deadlock. Either every position is
/// rewritten or none is.
///
/// No audit row and no version bump: a reorder changes only `position`, which
/// no edit form carries, so it can make no open form stale.
///
/// # Errors
///
/// Returns [`ConflictKind::MenuChanged`] if the list is not the live set.
pub async fn reorder_menu_categories(
    tx: &mut ScopedTx<'_>,
    ids: &[Uuid],
) -> DomainResult<Vec<MenuCategory>> {
    let live: Vec<Uuid> = sqlx::query_scalar!(
        "SELECT id FROM menu_categories WHERE archived_at IS NULL ORDER BY id FOR UPDATE"
    )
    .fetch_all(tx.connection())
    .await?;

    if !menu::is_the_same_list(ids, &live) {
        return Err(DomainError::Conflict(ConflictKind::MenuChanged));
    }

    sqlx::query!(
        r#"
        UPDATE menu_categories AS c
        SET position = o.n::int, updated_at = now()
        FROM unnest($1::uuid[]) WITH ORDINALITY AS o(id, n)
        WHERE c.id = o.id
        "#,
        ids,
    )
    .execute(tx.connection())
    .await?;

    if let Some(first) = ids.first() {
        Database::notify_entity_change(tx, EntityKind::MenuCategory, *first).await?;
    }

    live_menu_categories(tx).await
}

/// Takes a category off the menu without deleting it.
///
/// Refused while it still holds a live dish. The row is locked `FOR UPDATE`
/// before the dishes are counted, which is what stops a dish being created in
/// it, moved into it, or restored into it between the count and the archive.
///
/// # Errors
///
/// Returns [`DomainError::NotFound`] if no such live category belongs to this
/// restaurant, and [`ConflictKind::CategoryNotEmpty`] if it holds a live dish.
pub async fn archive_menu_category(
    tx: &mut ScopedTx<'_>,
    category_id: MenuCategoryId,
    actor: StaffId,
) -> DomainResult<MenuCategory> {
    sqlx::query!(
        "SELECT id FROM menu_categories WHERE id = $1 AND archived_at IS NULL FOR UPDATE",
        category_id.as_uuid()
    )
    .fetch_optional(tx.connection())
    .await?
    .ok_or(DomainError::NotFound)?;

    let holds_a_dish = sqlx::query_scalar!(
        r#"
        SELECT EXISTS (
            SELECT 1 FROM dishes WHERE category_id = $1 AND archived_at IS NULL
        ) AS "holds!"
        "#,
        category_id.as_uuid()
    )
    .fetch_one(tx.connection())
    .await?;

    if holds_a_dish {
        return Err(DomainError::Conflict(ConflictKind::CategoryNotEmpty));
    }

    let row = sqlx::query!(
        r#"
        UPDATE menu_categories
        SET archived_at = now(), version = version + 1, updated_at = now()
        WHERE id = $1
        RETURNING id, name, position, version, archived_at
        "#,
        category_id.as_uuid()
    )
    .fetch_one(tx.connection())
    .await?;

    let archived = category_from!(row);

    audit::record(
        tx,
        Some(actor),
        AuditAction::MenuCategoryArchived,
        EntityKind::MenuCategory.as_label(),
        category_id.as_uuid(),
        Some(json!({ "archived_at": Option::<DateTime<Utc>>::None })),
        Some(json!({ "archived_at": archived.archived_at })),
    )
    .await?;

    Database::notify_entity_change(tx, EntityKind::MenuCategory, category_id.as_uuid()).await?;

    Ok(archived)
}

/// Puts an archived category back, at the end of the category list.
///
/// # Errors
///
/// Returns [`DomainError::NotFound`] if no such archived category belongs to
/// this restaurant, and [`ConflictKind::NameTaken`] if a live category now has
/// its name.
pub async fn restore_menu_category(
    tx: &mut ScopedTx<'_>,
    category_id: MenuCategoryId,
    actor: StaffId,
) -> DomainResult<MenuCategory> {
    let position = next_category_position(tx).await?;

    let row = sqlx::query!(
        r#"
        UPDATE menu_categories AS c
        SET archived_at = NULL, position = $2, version = c.version + 1, updated_at = now()
        FROM (SELECT id, archived_at, position FROM menu_categories WHERE id = $1) AS old
        WHERE c.id = old.id AND c.archived_at IS NOT NULL
        RETURNING c.id AS "id!", c.name AS "name!", c.version AS "version!",
                  old.archived_at AS "old_archived_at", old.position AS "old_position!"
        "#,
        category_id.as_uuid(),
        position,
    )
    .fetch_optional(tx.connection())
    .await
    .map_err(|error| conflict_on(error, CATEGORY_NAME_KEY, ConflictKind::NameTaken))?
    .ok_or(DomainError::NotFound)?;

    audit::record(
        tx,
        Some(actor),
        AuditAction::MenuCategoryRestored,
        EntityKind::MenuCategory.as_label(),
        category_id.as_uuid(),
        Some(json!({ "archived_at": row.old_archived_at, "position": row.old_position })),
        Some(json!({ "archived_at": Option::<DateTime<Utc>>::None, "position": position })),
    )
    .await?;

    Database::notify_entity_change(tx, EntityKind::MenuCategory, category_id.as_uuid()).await?;

    Ok(MenuCategory {
        id: MenuCategoryId::from_uuid(row.id),
        name: row.name,
        position,
        version: row.version,
        archived_at: None,
    })
}

/// What creating a dish asks for.
///
/// No availability and no position. A new dish is available, because the
/// create form has no switch, and it goes to the end of its category, from where
/// it is moved by dragging.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewDish<'a> {
    /// Which category it sits under.
    pub category_id: MenuCategoryId,
    /// What it is called.
    pub name: &'a str,
    /// What it is, for the waiter to read out.
    pub description: Option<&'a str>,
    /// What it costs.
    pub price: Decimal,
    /// Whether it is veg, non veg, or egg.
    pub diet: Diet,
}

/// Adds one dish to the end of its category, available.
///
/// # Errors
///
/// Returns [`DomainError::Invalid`] if the name is blank or the price is
/// negative, [`DomainError::NotFound`] for a category this restaurant does not
/// have, [`ConflictKind::CategoryArchived`] for one that has been archived,
/// [`ConflictKind::NameTaken`] if a live dish already has that name, and
/// [`DomainError::Unavailable`] if a statement fails.
pub async fn create_dish(
    tx: &mut ScopedTx<'_>,
    dish: &NewDish<'_>,
    actor: StaffId,
) -> DomainResult<Dish> {
    let name = require_name(dish.name, "a dish needs a name")?;

    if dish.price < Decimal::ZERO {
        return Err(DomainError::Invalid(
            "a dish cannot cost less than nothing".to_owned(),
        ));
    }

    let description = dish
        .description
        .map(str::trim)
        .filter(|text| !text.is_empty());

    lock_live_category_for_a_dish(tx, dish.category_id).await?;
    let position = next_dish_position(tx, dish.category_id).await?;
    let id = DishId::new();

    sqlx::query!(
        r#"
        INSERT INTO dishes
            (id, restaurant_id, category_id, name, description, price, diet, is_available,
             position, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, true, $8, now())
        "#,
        id.as_uuid(),
        tx.restaurant_id().as_uuid(),
        dish.category_id.as_uuid(),
        name,
        description,
        dish.price,
        dish.diet as Diet,
        position,
    )
    .execute(tx.connection())
    .await
    .map_err(|error| conflict_on(error, DISH_NAME_KEY, ConflictKind::NameTaken))?;

    audit::record(
        tx,
        Some(actor),
        AuditAction::DishCreated,
        EntityKind::Dish.as_label(),
        id.as_uuid(),
        None,
        Some(json!({
            "category_id": dish.category_id,
            "name": name,
            "description": description,
            "price": dish.price,
            "diet": dish.diet,
            "is_available": true,
            "position": position,
        })),
    )
    .await?;

    // A waiter with the ordering screen open is holding the menu this changes.
    Database::notify_entity_change(tx, EntityKind::Dish, id.as_uuid()).await?;

    Ok(Dish {
        id,
        category_id: dish.category_id,
        name: name.to_owned(),
        description: description.map(str::to_owned),
        price: dish.price,
        diet: dish.diet,
        is_available: true,
        position,
        version: 1,
        archived_at: None,
    })
}

/// What an edit to a dish may change, and which version of the dish it was
/// made against.
///
/// Every field is supplied, so an edit is a statement of what the dish should be
/// rather than a patch whose omissions mean two different things. Availability
/// is deliberately absent: only the switch writes it, so an edit form can never
/// switch back on a dish the kitchen has just run out of.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DishEdit {
    /// Which category it should sit under. A different one moves it to the end
    /// of that category.
    pub category_id: MenuCategoryId,
    /// What it should be called.
    pub name: String,
    /// What it is, for the waiter to read out.
    pub description: Option<String>,
    /// What it should cost from now on. Lines already sent keep the price they
    /// copied.
    pub price: Decimal,
    /// Whether it is veg, non veg, or egg.
    pub diet: Diet,
    /// The version the edit form loaded.
    pub version: i32,
}

/// Edits a live dish, and writes down what it was before.
///
/// The target category is checked first, then the version, so an edit that is
/// both stale and aimed at a removed category reports the category: that is the
/// thing the admin must change before anything else will save. Nothing here
/// reaches an order line: a line copied the name and the price when it was sent,
/// and never reads the menu again.
///
/// # Errors
///
/// Returns [`ConflictKind::CategoryArchived`] if the target category has been
/// archived, [`ConflictKind::DishChanged`] if the stored version is newer,
/// [`DomainError::NotFound`] if no such live dish or category belongs to this
/// restaurant, [`ConflictKind::NameTaken`] if a live dish already has the new
/// name, and [`DomainError::Invalid`] for a blank name or a negative price.
pub async fn update_dish(
    tx: &mut ScopedTx<'_>,
    dish_id: DishId,
    edit: &DishEdit,
    actor: StaffId,
) -> DomainResult<Dish> {
    let name = require_name(&edit.name, "a dish needs a name")?;

    if edit.price < Decimal::ZERO {
        return Err(DomainError::Invalid(
            "a dish cannot cost less than nothing".to_owned(),
        ));
    }

    lock_live_category_for_a_dish(tx, edit.category_id).await?;
    let end_of_target = next_dish_position(tx, edit.category_id).await?;

    // A dish staying in its category keeps its place; one moving goes to the
    // end of the new one. The subquery is the row before this statement, for
    // the audit row, for the same reason as in `rename_menu_category`.
    let updated = sqlx::query!(
        r#"
        UPDATE dishes AS d
        SET category_id = $2, name = $3, description = $4, price = $5, diet = $6,
            position = CASE WHEN d.category_id = $2 THEN d.position ELSE $8 END,
            version = d.version + 1, updated_at = now()
        FROM (
            SELECT id, category_id, name, description, price, diet
            FROM dishes
            WHERE id = $1
        ) AS old
        WHERE d.id = old.id AND d.version = $7 AND d.archived_at IS NULL
        RETURNING old.category_id AS "old_category_id!", old.name AS "old_name!",
                  old.description AS "old_description?", old.price AS "old_price!",
                  old.diet AS "old_diet!: Diet",
                  d.is_available AS "is_available!", d.position AS "position!",
                  d.version AS "version!"
        "#,
        dish_id.as_uuid(),
        edit.category_id.as_uuid(),
        name,
        edit.description.as_deref(),
        edit.price,
        edit.diet as Diet,
        edit.version,
        end_of_target,
    )
    .fetch_optional(tx.connection())
    .await
    .map_err(|error| conflict_on(error, DISH_NAME_KEY, ConflictKind::NameTaken))?;

    let Some(row) = updated else {
        return Err(stale_or_missing_dish(tx, dish_id).await);
    };

    audit::record(
        tx,
        Some(actor),
        AuditAction::DishEdited,
        EntityKind::Dish.as_label(),
        dish_id.as_uuid(),
        Some(json!({
            "category_id": row.old_category_id,
            "name": row.old_name,
            "description": row.old_description,
            "price": row.old_price,
            "diet": row.old_diet,
        })),
        Some(json!({
            "category_id": edit.category_id,
            "name": name,
            "description": edit.description,
            "price": edit.price,
            "diet": edit.diet,
        })),
    )
    .await?;

    Database::notify_entity_change(tx, EntityKind::Dish, dish_id.as_uuid()).await?;

    Ok(Dish {
        id: dish_id,
        category_id: edit.category_id,
        name: name.to_owned(),
        description: edit.description.clone(),
        price: edit.price,
        diet: edit.diet,
        is_available: row.is_available,
        position: row.position,
        version: row.version,
        archived_at: None,
    })
}

/// Why a conditional update on a dish matched nothing.
///
/// A live row means somebody changed it after the form loaded, the kitchen's
/// availability switch included. No row, or an archived one, reads as not
/// found.
async fn stale_or_missing_dish(tx: &mut ScopedTx<'_>, dish_id: DishId) -> DomainError {
    match dish(tx, dish_id).await {
        Ok(found) if found.archived_at.is_none() => {
            DomainError::Conflict(ConflictKind::DishChanged)
        }
        Ok(_) => DomainError::NotFound,
        Err(error) => error,
    }
}

/// Switches a live dish on or off, and writes down that it happened.
///
/// An absolute value rather than a toggle, and never refused as stale. "Off" is
/// the chef's whole intent, so two quick taps must not cancel each other out and
/// a switch made from a screen that had not refetched must still land. Setting
/// the value the dish already has succeeds and changes nothing: no write, no
/// version bump, no audit row, no event.
///
/// When it does change the value it bumps the version, which is what makes an
/// admin's open edit form for this dish stale, so saving that form cannot
/// switch the dish back on.
///
/// # Errors
///
/// Returns [`DomainError::NotFound`] if no such live dish belongs to this
/// restaurant. A dish archived a moment earlier reads the same as an unknown
/// one.
pub async fn set_dish_availability(
    tx: &mut ScopedTx<'_>,
    dish_id: DishId,
    available: bool,
    actor: StaffId,
) -> DomainResult<Dish> {
    let changed = sqlx::query!(
        r#"
        UPDATE dishes
        SET is_available = $2, version = version + 1, updated_at = now()
        WHERE id = $1 AND archived_at IS NULL AND is_available <> $2
        RETURNING id, category_id, name, description, price, diet AS "diet: Diet",
                  is_available, position, version, archived_at
        "#,
        dish_id.as_uuid(),
        available,
    )
    .fetch_optional(tx.connection())
    .await?;

    let Some(row) = changed else {
        // Nothing matched: either it already had this value, which is a
        // success that changed nothing, or there is no live dish to switch.
        let current = dish(tx, dish_id).await?;

        if current.archived_at.is_some() {
            return Err(DomainError::NotFound);
        }

        return Ok(current);
    };

    audit::record(
        tx,
        Some(actor),
        AuditAction::DishAvailabilityChanged,
        EntityKind::Dish.as_label(),
        dish_id.as_uuid(),
        Some(json!({ "is_available": !available })),
        Some(json!({ "is_available": available })),
    )
    .await?;

    Database::notify_entity_change(tx, EntityKind::Dish, dish_id.as_uuid()).await?;

    Ok(dish_from!(row))
}

/// Rewrites one category's dish order to exactly the list given.
///
/// The same rules as [`reorder_menu_categories`], over the live dishes of one
/// category. A dish cannot be dragged into another category: that is a move, and
/// it is done in the edit form, where its version is checked.
///
/// # Errors
///
/// Returns [`DomainError::NotFound`] if no such live category belongs to this
/// restaurant, and [`ConflictKind::MenuChanged`] if the list is not the live
/// set of that category's dishes.
pub async fn reorder_dishes(
    tx: &mut ScopedTx<'_>,
    category_id: MenuCategoryId,
    ids: &[Uuid],
) -> DomainResult<Vec<Dish>> {
    sqlx::query!(
        "SELECT id FROM menu_categories WHERE id = $1 AND archived_at IS NULL",
        category_id.as_uuid()
    )
    .fetch_optional(tx.connection())
    .await?
    .ok_or(DomainError::NotFound)?;

    let live: Vec<Uuid> = sqlx::query_scalar!(
        r#"
        SELECT id FROM dishes
        WHERE category_id = $1 AND archived_at IS NULL
        ORDER BY id
        FOR UPDATE
        "#,
        category_id.as_uuid()
    )
    .fetch_all(tx.connection())
    .await?;

    if !menu::is_the_same_list(ids, &live) {
        return Err(DomainError::Conflict(ConflictKind::MenuChanged));
    }

    sqlx::query!(
        r#"
        UPDATE dishes AS d
        SET position = o.n::int, updated_at = now()
        FROM unnest($1::uuid[]) WITH ORDINALITY AS o(id, n)
        WHERE d.id = o.id
        "#,
        ids,
    )
    .execute(tx.connection())
    .await?;

    if let Some(first) = ids.first() {
        Database::notify_entity_change(tx, EntityKind::Dish, *first).await?;
    }

    let rows = sqlx::query!(
        r#"
        SELECT id, category_id, name, description, price, diet AS "diet: Diet",
               is_available, position, version, archived_at
        FROM dishes
        WHERE category_id = $1 AND archived_at IS NULL
        ORDER BY position, name, id
        "#,
        category_id.as_uuid()
    )
    .fetch_all(tx.connection())
    .await?;

    Ok(rows.into_iter().map(|row| dish_from!(row)).collect())
}

/// Takes a dish off the menu without deleting it.
///
/// Archiving rather than deleting is what keeps every line, round, and bill that
/// referred to it resolving. The version is bumped, so an edit form still open
/// on it is refused rather than bringing it back to life.
///
/// # Errors
///
/// Returns [`DomainError::NotFound`] if no such live dish belongs to this
/// restaurant.
pub async fn archive_dish(
    tx: &mut ScopedTx<'_>,
    dish_id: DishId,
    actor: StaffId,
) -> DomainResult<Dish> {
    let row = sqlx::query!(
        r#"
        UPDATE dishes
        SET archived_at = now(), version = version + 1, updated_at = now()
        WHERE id = $1 AND archived_at IS NULL
        RETURNING id, category_id, name, description, price, diet AS "diet: Diet",
                  is_available, position, version, archived_at
        "#,
        dish_id.as_uuid()
    )
    .fetch_optional(tx.connection())
    .await?
    .ok_or(DomainError::NotFound)?;

    let archived = dish_from!(row);

    audit::record(
        tx,
        Some(actor),
        AuditAction::DishArchived,
        EntityKind::Dish.as_label(),
        dish_id.as_uuid(),
        Some(json!({ "archived_at": Option::<DateTime<Utc>>::None })),
        Some(json!({ "archived_at": archived.archived_at })),
    )
    .await?;

    Database::notify_entity_change(tx, EntityKind::Dish, dish_id.as_uuid()).await?;

    Ok(archived)
}

/// Puts an archived dish back, at the end of a live category.
///
/// It keeps its name, description, price, diet marker, and availability: a
/// seasonal dish comes back exactly as it left, and a mistaken removal costs
/// nothing.
///
/// # Errors
///
/// Returns [`ConflictKind::CategoryArchived`] if the chosen category has been
/// archived, [`DomainError::NotFound`] if no such archived dish or category
/// belongs to this restaurant, and [`ConflictKind::NameTaken`] if a live dish
/// now has its name.
pub async fn restore_dish(
    tx: &mut ScopedTx<'_>,
    dish_id: DishId,
    category_id: MenuCategoryId,
    actor: StaffId,
) -> DomainResult<Dish> {
    lock_live_category_for_a_dish(tx, category_id).await?;
    let position = next_dish_position(tx, category_id).await?;

    let row = sqlx::query!(
        r#"
        UPDATE dishes AS d
        SET archived_at = NULL, category_id = $2, position = $3,
            version = d.version + 1, updated_at = now()
        FROM (SELECT id, archived_at, category_id FROM dishes WHERE id = $1) AS old
        WHERE d.id = old.id AND d.archived_at IS NOT NULL
        RETURNING d.id AS "id!", d.category_id AS "category_id!", d.name AS "name!",
                  d.description AS "description?", d.price AS "price!",
                  d.diet AS "diet!: Diet", d.is_available AS "is_available!",
                  d.position AS "position!", d.version AS "version!",
                  d.archived_at AS "archived_at?",
                  old.archived_at AS "old_archived_at?", old.category_id AS "old_category_id!"
        "#,
        dish_id.as_uuid(),
        category_id.as_uuid(),
        position,
    )
    .fetch_optional(tx.connection())
    .await
    .map_err(|error| conflict_on(error, DISH_NAME_KEY, ConflictKind::NameTaken))?
    .ok_or(DomainError::NotFound)?;

    audit::record(
        tx,
        Some(actor),
        AuditAction::DishRestored,
        EntityKind::Dish.as_label(),
        dish_id.as_uuid(),
        Some(json!({
            "archived_at": row.old_archived_at,
            "category_id": row.old_category_id,
        })),
        Some(json!({
            "archived_at": Option::<DateTime<Utc>>::None,
            "category_id": category_id,
            "position": position,
        })),
    )
    .await?;

    Database::notify_entity_change(tx, EntityKind::Dish, dish_id.as_uuid()).await?;

    Ok(dish_from!(row))
}
