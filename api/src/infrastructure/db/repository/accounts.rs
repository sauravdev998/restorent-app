//! Registering a restaurant, reading who somebody is, and the three writes a
//! signed in person may make to their own account and to their restaurant.
//!
//! Registration is the one place in the product that creates an admin without
//! an admin already existing, and it is one transaction. The restaurant, the
//! admin's `staff` row, and the audit entry either all exist or none of them do,
//! which is what makes "a restaurant never exists without an admin" a fact
//! rather than a hope.
//!
//! It runs through an ordinary [`ScopedTx`] like everything else, scoped to the
//! restaurant identifier the caller just minted. That works because the tenant
//! policy on `restaurants` checks `id = current_restaurant_id()`, so inserting
//! the row that the transaction is already scoped to passes its own check.
//! Nothing here needs an unscoped connection.

use serde_json::json;

use crate::domain::audit::AuditAction;
use crate::domain::country::Country;
use crate::domain::credentials::EmailAddress;
use crate::domain::enums::StaffRole;
use crate::domain::error::{DomainError, DomainResult};
use crate::domain::ids::StaffId;
use crate::domain::language::{FormattingLocale, LanguageCode};
use crate::domain::people::Staff;

use super::super::ScopedTx;
use super::audit;

/// What registration was asked to create.
///
/// Every value is already validated: the country came through
/// [`CountryCode`](crate::domain::country::CountryCode), the address through
/// [`EmailAddress`], and the hash is the output of the password port. There is
/// nothing left for this layer to check, which is the point of the newtypes.
#[derive(Debug, Clone)]
pub struct Registration<'a> {
    /// What the restaurant is called.
    pub restaurant_name: &'a str,
    /// The row the five settings come from.
    pub country: &'a Country,
    /// What to call the owner on screen.
    pub display_name: &'a str,
    /// Their address, exactly as they typed it.
    pub email: &'a EmailAddress,
    /// The `argon2id` hash of their password. Never the password.
    pub password_hash: &'a str,
}

/// Creates a restaurant and its first admin, in one transaction.
///
/// The five settings come from the country's row and from nowhere else. The
/// audit entry names the restaurant as its entity, carries no `before`, and
/// carries no password hash in its `after`.
///
/// # Errors
///
/// Returns [`DomainError::Conflict`] if that email address already belongs to
/// an account anywhere on the platform, which the caller turns into
/// `fields.email=already_taken`, and [`DomainError::Unavailable`] if a
/// statement fails. Either way the caller drops the transaction and nothing at
/// all is created.
pub async fn register(
    tx: &mut ScopedTx<'_>,
    registration: &Registration<'_>,
) -> DomainResult<StaffId> {
    let restaurant_id = tx.restaurant_id().as_uuid();
    let country = registration.country;

    // `currency_decimals` is `smallint`. The cast cannot fail for any value the
    // country list will parse, and the country test pins that, but a fallible
    // conversion here beats an `as` that would silently wrap if it ever did.
    let currency_decimals = i16::try_from(country.currency_decimals).map_err(|_| {
        DomainError::Invalid(format!(
            "{} uses {} decimal places, which is not a currency this schema can hold",
            country.code, country.currency_decimals
        ))
    })?;

    sqlx::query!(
        r#"
        INSERT INTO restaurants
            (id, name, country_code, currency_code, currency_decimals, timezone,
             default_language, formatting_locale, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, now())
        "#,
        restaurant_id,
        registration.restaurant_name.trim(),
        &country.code,
        &country.currency_code,
        currency_decimals,
        &country.default_timezone,
        &country.default_language,
        &country.formatting_locale,
    )
    .execute(tx.connection())
    .await?;

    let admin = StaffId::new();

    sqlx::query!(
        r#"
        INSERT INTO staff
            (id, restaurant_id, email, password_hash, display_name, role, updated_at)
        VALUES ($1, $2, $3, $4, $5, 'admin', now())
        "#,
        admin.as_uuid(),
        restaurant_id,
        registration.email.as_str(),
        registration.password_hash,
        registration.display_name.trim(),
    )
    .execute(tx.connection())
    .await
    .map_err(|error| {
        super::conflict_on(
            error,
            "staff_email_key",
            "that email address already has an account",
        )
    })?;

    // Named as the restaurant rather than as the staff member, because what
    // happened is that a restaurant came into existence. The admin's details
    // ride along in `after` so the row answers "who did this restaurant start
    // with" without a second lookup. No hash appears anywhere in it.
    audit::record(
        tx,
        Some(admin),
        AuditAction::RestaurantRegistered,
        "restaurant",
        restaurant_id,
        None,
        Some(json!({
            "name": registration.restaurant_name.trim(),
            "countryCode": &country.code,
            "currencyCode": &country.currency_code,
            "currencyDecimals": country.currency_decimals,
            "timezone": &country.default_timezone,
            "defaultLanguage": &country.default_language,
            "formattingLocale": &country.formatting_locale,
            "adminStaffId": admin,
            "adminDisplayName": registration.display_name.trim(),
            "adminEmail": registration.email.as_str(),
        })),
    )
    .await?;

    Ok(admin)
}

/// Reads one staff member by identifier.
///
/// Scoped like every other read here, so it can only ever reach a row in the
/// transaction's own restaurant. Which staff member a caller may ask for is a
/// rule about the caller and lives above this layer.
///
/// # Errors
///
/// Returns [`DomainError::NotFound`] if no such staff member belongs to this
/// restaurant, and [`DomainError::Invalid`] if their stored language is no
/// longer in the catalogue.
pub async fn staff_by_id(tx: &mut ScopedTx<'_>, staff_id: StaffId) -> DomainResult<Staff> {
    let row = sqlx::query!(
        r#"
        SELECT id, email, display_name, role AS "role: StaffRole", language, deactivated_at
        FROM staff
        WHERE id = $1
        "#,
        staff_id.as_uuid(),
    )
    .fetch_optional(tx.connection())
    .await?
    .ok_or(DomainError::NotFound)?;

    Ok(Staff {
        id: StaffId::from_uuid(row.id),
        email: row.email,
        display_name: row.display_name,
        role: row.role,
        language: row.language.as_deref().map(LanguageCode::new).transpose()?,
        deactivated_at: row.deactivated_at,
    })
}

/// The stored password hash for one staff member, for checking a current
/// password before changing it.
///
/// Scoped, unlike the sign in lookup, because by this point the caller is
/// already signed in and the row is their own.
///
/// # Errors
///
/// Returns [`DomainError::NotFound`] if no such staff member belongs to this
/// restaurant.
pub async fn password_hash_of(tx: &mut ScopedTx<'_>, staff_id: StaffId) -> DomainResult<String> {
    let row = sqlx::query!(
        "SELECT password_hash FROM staff WHERE id = $1",
        staff_id.as_uuid(),
    )
    .fetch_optional(tx.connection())
    .await?
    .ok_or(DomainError::NotFound)?;

    Ok(row.password_hash)
}

/// Writes a new password hash onto one staff member's own row.
///
/// Revoking their other sessions is the caller's job, in the same transaction.
/// It is deliberately not folded in here: the rule is about sessions, and
/// feature 10 has three more events that trigger the same revocation without
/// touching a password.
///
/// # Errors
///
/// Returns [`DomainError::NotFound`] if no such staff member belongs to this
/// restaurant.
pub async fn set_password_hash(
    tx: &mut ScopedTx<'_>,
    staff_id: StaffId,
    password_hash: &str,
) -> DomainResult<()> {
    let updated = sqlx::query!(
        "UPDATE staff SET password_hash = $1, updated_at = now() WHERE id = $2",
        password_hash,
        staff_id.as_uuid(),
    )
    .execute(tx.connection())
    .await?
    .rows_affected();

    if updated == 0 {
        return Err(DomainError::NotFound);
    }

    // What changed is recorded; what it changed to is not, and neither is what
    // it changed from. The row answers "who changed their password and when",
    // which is the question an access control log exists for.
    audit::record(
        tx,
        Some(staff_id),
        AuditAction::PasswordChanged,
        "staff",
        staff_id.as_uuid(),
        None,
        Some(json!({ "staffId": staff_id })),
    )
    .await?;

    Ok(())
}

/// Sets one staff member's own display name.
///
/// # Errors
///
/// Returns [`DomainError::NotFound`] if no such staff member belongs to this
/// restaurant, and [`DomainError::Invalid`] if the name is blank.
pub async fn set_display_name(
    tx: &mut ScopedTx<'_>,
    staff_id: StaffId,
    display_name: &str,
) -> DomainResult<()> {
    let trimmed = display_name.trim();

    if trimmed.is_empty() {
        return Err(DomainError::Invalid(
            "a display name is required".to_owned(),
        ));
    }

    let updated = sqlx::query!(
        "UPDATE staff SET display_name = $1, updated_at = now() WHERE id = $2",
        trimmed,
        staff_id.as_uuid(),
    )
    .execute(tx.connection())
    .await?
    .rows_affected();

    if updated == 0 {
        return Err(DomainError::NotFound);
    }

    Ok(())
}

/// What an admin may change about the restaurant.
///
/// Every field is optional and [`None`] means "leave it alone", because this is
/// a patch of a settings form rather than a statement of the whole restaurant.
/// No money setting appears here at all: the currency and its decimals were
/// fixed by the country at registration, and the service charge has its own
/// audited path in [`catalog`](super::catalog).
#[derive(Debug, Clone, Default)]
pub struct RestaurantSettingsPatch<'a> {
    /// What the restaurant should be called.
    pub name: Option<&'a str>,
    /// Where it is, for printing on a bill. `Some("")` clears it.
    pub address: Option<&'a str>,
    /// Its IANA timezone. Checked by Postgres, which is the only thing that
    /// knows the real list.
    pub timezone: Option<&'a str>,
    /// What the kitchen screen reads in.
    pub default_language: Option<&'a LanguageCode>,
    /// How it writes money, numbers, and dates.
    pub formatting_locale: Option<&'a FormattingLocale>,
}

impl RestaurantSettingsPatch<'_> {
    /// Whether this patch would change anything at all.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.name.is_none()
            && self.address.is_none()
            && self.timezone.is_none()
            && self.default_language.is_none()
            && self.formatting_locale.is_none()
    }
}

/// Applies an admin's edit to the restaurant's settings, and writes it down.
///
/// One statement with `COALESCE` per column rather than a built up `SET` list,
/// so there is no string concatenation anywhere near it and an omitted field is
/// expressed as a null bind rather than as an absent clause.
///
/// # Errors
///
/// Returns [`DomainError::NotFound`] if the scoped restaurant does not exist,
/// [`DomainError::Invalid`] if the timezone is not one Postgres knows or the
/// name is blank, and [`DomainError::Unavailable`] if a statement fails.
pub async fn update_settings(
    tx: &mut ScopedTx<'_>,
    patch: &RestaurantSettingsPatch<'_>,
    actor: StaffId,
) -> DomainResult<()> {
    if patch.is_empty() {
        return Ok(());
    }

    if patch.name.is_some_and(|name| name.trim().is_empty()) {
        return Err(DomainError::Invalid(
            "a restaurant name is required".to_owned(),
        ));
    }

    // Postgres is the only authority on whether a string is a real IANA zone,
    // and asking it directly turns a typo into a refused edit rather than a
    // restaurant whose local day is nonsense.
    if let Some(timezone) = patch.timezone {
        let known = sqlx::query!(
            "SELECT EXISTS (SELECT 1 FROM pg_timezone_names WHERE name = $1) AS \"known!\"",
            timezone,
        )
        .fetch_one(tx.connection())
        .await?
        .known;

        if !known {
            return Err(DomainError::Invalid(format!(
                "{timezone:?} is not a timezone this platform knows"
            )));
        }
    }

    let before = sqlx::query!(
        r#"
        SELECT name, address, timezone, default_language, formatting_locale
        FROM restaurants
        FOR UPDATE
        "#
    )
    .fetch_optional(tx.connection())
    .await?
    .ok_or(DomainError::NotFound)?;

    let name = patch.name.map(str::trim);
    // An empty address means "there is no address", which is a real edit, so it
    // is stored as null rather than as an empty string.
    let address = patch
        .address
        .map(str::trim)
        .map(|value| if value.is_empty() { None } else { Some(value) });

    sqlx::query!(
        r#"
        UPDATE restaurants
           SET name              = COALESCE($1, name),
               address           = CASE WHEN $2::boolean THEN $3::text ELSE address END,
               timezone          = COALESCE($4, timezone),
               default_language  = COALESCE($5, default_language),
               formatting_locale = COALESCE($6, formatting_locale),
               updated_at        = now()
        "#,
        name,
        address.is_some(),
        address.flatten(),
        patch.timezone,
        patch.default_language.map(LanguageCode::as_str),
        patch.formatting_locale.map(FormattingLocale::as_str),
    )
    .execute(tx.connection())
    .await?;

    let after = sqlx::query!(
        r#"
        SELECT name, address, timezone, default_language, formatting_locale
        FROM restaurants
        "#
    )
    .fetch_one(tx.connection())
    .await?;

    let restaurant_id = tx.restaurant_id().as_uuid();

    audit::record(
        tx,
        Some(actor),
        AuditAction::RestaurantSettingsUpdated,
        "restaurant",
        restaurant_id,
        Some(json!({
            "name": before.name,
            "address": before.address,
            "timezone": before.timezone,
            "defaultLanguage": before.default_language,
            "formattingLocale": before.formatting_locale,
        })),
        Some(json!({
            "name": after.name,
            "address": after.address,
            "timezone": after.timezone,
            "defaultLanguage": after.default_language,
            "formattingLocale": after.formatting_locale,
        })),
    )
    .await?;

    Ok(())
}
