//! The three writes a signed in person may make: to their own account, to their
//! own password, and, if they are an admin, to the restaurant's settings.
//!
//! What ties them together is what none of them takes: a staff id. `PATCH
//! /api/me` and `POST /api/me/password` write the caller's own row, always,
//! because the row they write comes from the resolved session and there is no
//! parameter that could name another. Supplying somebody else's id in the body
//! does nothing, because there is nothing to supply it to.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use serde::Deserialize;
use utoipa::ToSchema;

use crate::application::ports::PasswordHasher as _;
use crate::domain::catalog::{ThresholdProblem, merge_kitchen_thresholds};
use crate::domain::credentials::Password;
use crate::domain::error::{DomainError, FieldError, FieldErrors};
use crate::domain::language::{FormattingLocale, LanguageCode};
use crate::infrastructure::db::repository::{accounts, catalog, sessions};
use crate::presentation::dto::IdentityBundle;
use crate::presentation::error::{ApiError, ErrorBody};
use crate::presentation::extract::{Actor, Admin, AnyRole, JsonBody, PasswordMayBeOwed};
use crate::presentation::state::AppState;

use super::auth::password_problem;

/// What somebody may change about their own account.
///
/// Both fields are optional and absent means "leave it alone". `language` is
/// deliberately `Option<Option<String>>`: absent leaves it, and an explicit
/// `null` clears it, which is a real choice somebody makes rather than an
/// absence. Collapsing the two would make "follow the restaurant" unsayable.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateMeRequest {
    /// What to call them on screen.
    #[serde(default)]
    pub display_name: Option<String>,
    /// Their own interface language, or `null` to follow the restaurant.
    ///
    /// Clippy is overruled here, and the reason is the whole point of the
    /// field. The three states are genuinely different and all three have to be
    /// sayable: absent means "do not touch my language", `null` means "follow
    /// the restaurant", and a code means that code. A custom enum would say the
    /// same thing with a hand written `Deserialize` and a hand written
    /// `ToSchema`, and the wire shape would be identical.
    #[allow(clippy::option_option)]
    #[serde(default, deserialize_with = "double_option")]
    pub language: Option<Option<String>>,
}

/// What changing your own password asks for.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChangePasswordRequest {
    /// The password they are signing in with now.
    pub current_password: String,
    /// What they want it to be.
    pub new_password: String,
}

/// What an admin may change about the restaurant.
///
/// No money setting appears here at all. The currency and its decimals were
/// fixed by the country at registration and changing them under bills that have
/// already closed would rewrite history.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateRestaurantRequest {
    /// Which version of the restaurant this edit was made against. Required, and
    /// an edit naming one that is no longer current is refused rather than
    /// merged over somebody else's change. Every identity bundle carries it,
    /// including the one a successful save returns.
    pub version: i32,
    /// What the restaurant is called.
    #[serde(default)]
    pub name: Option<String>,
    /// Where it is, for printing on a bill. An empty string clears it.
    #[serde(default)]
    pub address: Option<String>,
    /// Its IANA timezone.
    #[serde(default)]
    pub timezone: Option<String>,
    /// What the kitchen screen reads.
    #[serde(default)]
    pub default_language: Option<String>,
    /// How it writes money, numbers, and dates.
    #[serde(default)]
    pub formatting_locale: Option<String>,
    /// After how many seconds the kitchen pass draws a waiting ticket amber.
    /// Between 60 and 14400, and below the late threshold once laid over what is
    /// stored.
    #[serde(default)]
    pub kitchen_warning_after_seconds: Option<i32>,
    /// After how many seconds it draws one red. The same bounds, and above the
    /// warning threshold.
    #[serde(default)]
    pub kitchen_late_after_seconds: Option<i32>,
}

/// Tells an absent field from one explicitly set to `null`.
///
/// serde collapses both into `None` on an `Option<T>`, which is exactly the
/// distinction "clear my personal language" depends on. See the field above for
/// why the three states are all needed.
#[allow(clippy::option_option)]
fn double_option<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    serde::Deserialize::deserialize(deserializer).map(Some)
}

/// Changes the caller's own display name or personal language.
///
/// # Errors
///
/// Returns a `400` naming the field that was not accepted, `401` if nobody is
/// signed in, and a `503` if the database is unavailable.
#[utoipa::path(
    patch,
    path = "/api/me",
    tag = "accounts",
    request_body = UpdateMeRequest,
    responses(
        (status = 200, description = "The updated identity. Any signed in role, own row only.", body = IdentityBundle),
        (status = 400, description = "A field was not accepted.", body = ErrorBody),
        (status = 401, description = "Nobody is signed in.", body = ErrorBody),
    )
)]
pub async fn update_me(
    State(state): State<AppState>,
    actor: Actor,
    JsonBody(request): JsonBody<UpdateMeRequest>,
) -> Result<Json<IdentityBundle>, ApiError> {
    // Validated before the transaction opens, so a refused language never costs
    // a write.
    let language = match &request.language {
        Some(Some(code)) => {
            Some(Some(LanguageCode::new(code).map_err(|_| {
                field("language", FieldError::NotInCatalogue)
            })?))
        }
        Some(None) => Some(None),
        None => None,
    };

    if request
        .display_name
        .as_deref()
        .is_some_and(|name| name.trim().is_empty())
    {
        return Err(field("displayName", FieldError::Required));
    }

    let mut tx = state.database.begin_scoped(actor.restaurant_id()).await?;

    if let Some(display_name) = request.display_name.as_deref() {
        accounts::set_display_name(&mut tx, actor.staff_id(), display_name).await?;
    }

    if let Some(language) = language {
        catalog::set_staff_language(&mut tx, actor.staff_id(), language.as_ref()).await?;
    }

    let staff = accounts::staff_by_id(&mut tx, actor.staff_id()).await?;
    let restaurant = catalog::restaurant(&mut tx).await?;

    tx.commit().await?;

    Ok(Json(IdentityBundle {
        staff: staff.into(),
        restaurant: restaurant.into(),
    }))
}

/// Changes the caller's own password, and signs their other devices out.
///
/// The session that made the request keeps working, so somebody changing their
/// password on the screen in front of them is not immediately signed out of it.
/// Every other session of theirs is revoked in the same transaction, which is
/// what makes "I think somebody has my password" a thing they can act on alone.
///
/// The other of exactly two handlers that name `PasswordMayBeOwed`, and the
/// only way out of owing one. Somebody an admin created gives the password they
/// were handed as their current one, and this is what spends it: the flag is
/// cleared by the write itself, in the repository, so no path can set a
/// password without settling what was owed on it.
///
/// # Errors
///
/// Returns a `400` with `fields.currentPassword=incorrect` for a wrong current
/// password, `401` if nobody is signed in, and a `503` if the database or the
/// password hash is unavailable.
#[utoipa::path(
    post,
    path = "/api/me/password",
    tag = "accounts",
    request_body = ChangePasswordRequest,
    responses(
        (status = 204, description = "Changed. Any signed in role, own row only, and reachable while a password change is owed."),
        (status = 400, description = "The current password was wrong, or the new one broke a rule.", body = ErrorBody),
        (status = 401, description = "Nobody is signed in.", body = ErrorBody),
    )
)]
pub async fn change_password(
    State(state): State<AppState>,
    actor: Actor<AnyRole, PasswordMayBeOwed>,
    JsonBody(request): JsonBody<ChangePasswordRequest>,
) -> Result<StatusCode, ApiError> {
    let new_password = Password::new(&request.new_password)
        .map_err(|_| field("newPassword", password_problem(&request.new_password)))?;

    let mut tx = state.database.begin_scoped(actor.restaurant_id()).await?;
    let stored = accounts::password_hash_of(&mut tx, actor.staff_id()).await?;

    // The transaction is open across the verify, which costs tens of
    // milliseconds. That is the price of doing the check and the write under one
    // consistent view: the alternative reads the hash, closes, hashes, reopens,
    // and can write a new password over one that changed in between.
    let current = Password::new(&request.current_password)
        .map_err(|_| field("currentPassword", FieldError::Incorrect))?;

    if !state.passwords.verify(current, stored).await? {
        return Err(field("currentPassword", FieldError::Incorrect));
    }

    let hash = state.passwords.hash(new_password).await?;
    accounts::set_password_hash(&mut tx, actor.staff_id(), &hash).await?;

    let revoked =
        sessions::revoke_every_other(&mut tx, actor.staff_id(), actor.session_id()).await?;

    tx.commit().await?;

    tracing::info!(
        staff_id = %actor.staff_id(),
        revoked_sessions = revoked,
        "a password was changed"
    );

    Ok(StatusCode::NO_CONTENT)
}

/// Changes the restaurant's settings. Admins only.
///
/// The role requirement is in the signature: `Actor<Admin>` refuses a waiter and
/// a chef with `403` before this body runs, so there is no line here to forget.
///
/// **The edit names the version it was made against**, and a stale one is refused
/// rather than merged. That covers all seven settings, not only the two kitchen
/// thresholds this endpoint grew in spec 0012: name, address, timezone, and the
/// two language fields come under the same conditional update.
///
/// **The two thresholds are merged over the stored pair before either is
/// checked.** Each may be edited on its own while the rule that amber comes
/// before red spans both, so an edit raising only the warning can cross a stored
/// late value it never mentions. The merge is what catches that, and it is what
/// lets a refusal name a field rather than say "those two are wrong".
///
/// # Errors
///
/// Returns a `400` naming each field that was not accepted, `409
/// restaurant_changed` if the version is no longer current, `403` for a waiter
/// or a chef, `401` if nobody is signed in, and a `503` if the database is
/// unavailable.
#[utoipa::path(
    patch,
    path = "/api/restaurant",
    tag = "accounts",
    request_body = UpdateRestaurantRequest,
    responses(
        (status = 200, description = "The updated identity, carrying the new version. Admins only.", body = IdentityBundle),
        (status = 400, description = "A field was not accepted.", body = ErrorBody),
        (status = 403, description = "A waiter or a chef asked.", body = ErrorBody),
        (status = 401, description = "Nobody is signed in.", body = ErrorBody),
        (status = 409, description = "`restaurant_changed`: somebody saved these settings first.", body = ErrorBody),
    )
)]
pub async fn update_restaurant(
    State(state): State<AppState>,
    actor: Actor<Admin>,
    JsonBody(request): JsonBody<UpdateRestaurantRequest>,
) -> Result<Json<IdentityBundle>, ApiError> {
    // Both catalogue checks happen before the transaction opens, so a refused
    // code never costs a write, and both produce the same field code because
    // both mean the same thing: the platform does not offer that.
    let default_language = request
        .default_language
        .as_deref()
        .map(LanguageCode::new)
        .transpose()
        .map_err(|_| field("defaultLanguage", FieldError::NotInCatalogue))?;

    let formatting_locale = request
        .formatting_locale
        .as_deref()
        .map(FormattingLocale::new)
        .transpose()
        .map_err(|_| field("formattingLocale", FieldError::NotInCatalogue))?;

    let mut tx = state.database.begin_scoped(actor.restaurant_id()).await?;

    // Read in the same transaction as the write, so the pair the rule is checked
    // against is the pair the write lands on. A change committed in between bumps
    // the version, and the conditional update below refuses this edit rather than
    // storing a pair checked against something that has moved.
    let thresholds = match (
        request.kitchen_warning_after_seconds,
        request.kitchen_late_after_seconds,
    ) {
        (None, None) => None,
        (warning, late) => {
            let stored = catalog::kitchen_thresholds(&mut tx).await?;
            Some(merge_kitchen_thresholds(warning, late, stored).map_err(threshold_field_error)?)
        }
    };

    accounts::update_settings(
        &mut tx,
        &accounts::RestaurantSettingsPatch {
            name: request.name.as_deref(),
            address: request.address.as_deref(),
            timezone: request.timezone.as_deref(),
            default_language: default_language.as_ref(),
            formatting_locale: formatting_locale.as_ref(),
            kitchen_thresholds: thresholds,
        },
        request.version,
        actor.staff_id(),
    )
    .await
    .map_err(settings_field_error)?;

    let staff = accounts::staff_by_id(&mut tx, actor.staff_id()).await?;
    let restaurant = catalog::restaurant(&mut tx).await?;

    tx.commit().await?;

    Ok(Json(IdentityBundle {
        staff: staff.into(),
        restaurant: restaurant.into(),
    }))
}

/// One field, one problem.
fn field(name: &str, error: FieldError) -> ApiError {
    DomainError::InvalidFields(FieldErrors::one(name, error)).into()
}

/// Names the threshold box a refusal belongs to.
///
/// The domain says which of the two was refused and why, in its own words; this
/// is the only place that knows what the boxes are called on the wire.
fn threshold_field_error(problem: ThresholdProblem) -> ApiError {
    match problem {
        ThresholdProblem::Warning(error) => field("kitchenWarningAfterSeconds", error),
        ThresholdProblem::Late(error) => field("kitchenLateAfterSeconds", error),
    }
}

/// Puts the repository's two validation refusals beside the box they belong to.
///
/// The timezone is checked by Postgres, which only says so once the statement
/// runs, so it cannot be turned into a field error before the transaction opens
/// the way the two catalogue codes can.
fn settings_field_error(error: DomainError) -> ApiError {
    match &error {
        DomainError::Invalid(reason) if reason.contains("timezone") => {
            field("timezone", FieldError::InvalidFormat)
        }
        DomainError::Invalid(reason) if reason.contains("restaurant name") => {
            field("name", FieldError::Required)
        }
        _ => error.into(),
    }
}
