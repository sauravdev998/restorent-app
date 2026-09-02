//! What a restaurant sets up before it can serve anybody, and the edits to it
//! that are worth writing down.
//!
//! Every read here filters archived rows out for the caller. That is the whole
//! reason these exist rather than each feature writing its own `SELECT`: an
//! archived dish that disappears from eight screens and reappears on the ninth
//! is the failure this shape prevents. A caller that genuinely wants archived
//! rows, such as a bill reprint, reads through the line that copied the name
//! rather than through the menu.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde_json::json;

use crate::domain::audit::AuditAction;
use crate::domain::catalog::{
    DiningTable, Dish, MenuCategory, Restaurant, TableSection, TaxComponent,
};
use crate::domain::enums::StaffRole;
use crate::domain::error::{DomainError, DomainResult};
use crate::domain::event::EntityKind;
use crate::domain::ids::{
    DiningTableId, DishId, MenuCategoryId, RestaurantId, StaffId, TableSectionId, TaxComponentId,
};
use crate::domain::language::{FormattingLocale, LanguageCode};
use crate::domain::money::Currency;
use crate::domain::people::Staff;

use super::super::{Database, ScopedTx};
use super::audit;

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
        SELECT id, name, currency_code, currency_decimals, timezone,
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

/// The menu as a waiter sees it: live categories, in printed order.
///
/// # Errors
///
/// Returns [`DomainError::Unavailable`] if the read fails.
pub async fn live_menu_categories(tx: &mut ScopedTx<'_>) -> DomainResult<Vec<MenuCategory>> {
    let rows = sqlx::query!(
        r#"
        SELECT id, name, position, archived_at
        FROM menu_categories
        WHERE archived_at IS NULL
        ORDER BY position, name
        "#
    )
    .fetch_all(tx.connection())
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| MenuCategory {
            id: MenuCategoryId::from_uuid(row.id),
            name: row.name,
            position: row.position,
            archived_at: row.archived_at,
        })
        .collect())
}

/// The menu as a waiter sees it: live dishes, in printed order.
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
        SELECT id, category_id, name, description, price, is_available, position, archived_at
        FROM dishes
        WHERE archived_at IS NULL
        ORDER BY position, name
        "#
    )
    .fetch_all(tx.connection())
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| Dish {
            id: DishId::from_uuid(row.id),
            category_id: MenuCategoryId::from_uuid(row.category_id),
            name: row.name,
            description: row.description,
            price: row.price,
            is_available: row.is_available,
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

/// What an edit to a dish may change.
///
/// Every field is supplied, so an edit is a statement of what the dish should be
/// rather than a patch whose omissions mean two different things.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DishEdit {
    /// What it should be called.
    pub name: String,
    /// What it is, for the waiter to read out.
    pub description: Option<String>,
    /// What it should cost from now on. Lines already sent keep the price they
    /// copied.
    pub price: Decimal,
    /// Whether the kitchen can currently make it.
    pub is_available: bool,
}

/// Edits a dish, and writes down that it happened.
///
/// A price edit is one of the changes the audit log exists for, so the whole
/// before and after is recorded rather than just the new value. Nothing here
/// reaches a bill that has already closed: a line copied the price and the name
/// when it was ordered, and never reads the menu again.
///
/// # Errors
///
/// Returns [`DomainError::NotFound`] if no such dish belongs to this restaurant,
/// and [`DomainError::Unavailable`] if a statement fails.
pub async fn update_dish(
    tx: &mut ScopedTx<'_>,
    dish_id: DishId,
    edit: &DishEdit,
    actor: StaffId,
) -> DomainResult<Dish> {
    let before = sqlx::query!(
        r#"
        SELECT category_id, name, description, price, is_available, position, archived_at
        FROM dishes
        WHERE id = $1
        FOR UPDATE
        "#,
        dish_id.as_uuid()
    )
    .fetch_optional(tx.connection())
    .await?
    .ok_or(DomainError::NotFound)?;

    sqlx::query!(
        r#"
        UPDATE dishes
        SET name = $2, description = $3, price = $4, is_available = $5, updated_at = now()
        WHERE id = $1
        "#,
        dish_id.as_uuid(),
        edit.name,
        edit.description,
        edit.price,
        edit.is_available,
    )
    .execute(tx.connection())
    .await?;

    audit::record(
        tx,
        Some(actor),
        AuditAction::DishEdited,
        "dish",
        dish_id.as_uuid(),
        Some(json!({
            "name": before.name,
            "description": before.description,
            "price": before.price,
            "is_available": before.is_available,
        })),
        Some(json!({
            "name": edit.name,
            "description": edit.description,
            "price": edit.price,
            "is_available": edit.is_available,
        })),
    )
    .await?;

    Database::notify_entity_change(tx, EntityKind::Dish, dish_id.as_uuid()).await?;

    Ok(Dish {
        id: dish_id,
        category_id: MenuCategoryId::from_uuid(before.category_id),
        name: edit.name.clone(),
        description: edit.description.clone(),
        price: edit.price,
        is_available: edit.is_available,
        position: before.position,
        archived_at: before.archived_at,
    })
}

/// Takes a dish off the menu without deleting it.
///
/// Archiving rather than deleting is what keeps AC-10 true: the dish disappears
/// from every working query, and every line, round, and bill that referenced it
/// still resolves.
///
/// # Errors
///
/// Returns [`DomainError::NotFound`] if no such live dish belongs to this
/// restaurant.
pub async fn archive_dish(
    tx: &mut ScopedTx<'_>,
    dish_id: DishId,
    actor: StaffId,
) -> DomainResult<DateTime<Utc>> {
    let archived = sqlx::query!(
        r#"
        UPDATE dishes
        SET archived_at = now(), updated_at = now()
        WHERE id = $1 AND archived_at IS NULL
        RETURNING archived_at AS "archived_at!"
        "#,
        dish_id.as_uuid()
    )
    .fetch_optional(tx.connection())
    .await?
    .map(|row| row.archived_at)
    .ok_or(DomainError::NotFound)?;

    audit::record(
        tx,
        Some(actor),
        AuditAction::DishArchived,
        "dish",
        dish_id.as_uuid(),
        Some(json!({ "archived_at": Option::<DateTime<Utc>>::None })),
        Some(json!({ "archived_at": archived })),
    )
    .await?;

    Database::notify_entity_change(tx, EntityKind::Dish, dish_id.as_uuid()).await?;

    Ok(archived)
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

/// Takes a menu category out of use without deleting it.
///
/// # Errors
///
/// Returns [`DomainError::NotFound`] if no such live category belongs to this
/// restaurant.
pub async fn archive_menu_category(
    tx: &mut ScopedTx<'_>,
    category_id: MenuCategoryId,
) -> DomainResult<DateTime<Utc>> {
    sqlx::query!(
        r#"
        UPDATE menu_categories
        SET archived_at = now(), updated_at = now()
        WHERE id = $1 AND archived_at IS NULL
        RETURNING archived_at AS "archived_at!"
        "#,
        category_id.as_uuid()
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
