//! The admin's staff: who works here, and the six things an admin does to an
//! account that is not their own.
//!
//! Every endpoint here is admin only and says so in its own signature through
//! `Actor<Admin>`, so a waiter or a chef is refused with `403` before any body
//! runs and the restriction reaches the `OpenAPI` document rather than living
//! in a line somebody could delete.
//!
//! **No handler here can name a restaurant.** The transaction is scoped from
//! the resolved session, so `{id}` is always read inside the caller's own
//! restaurant and a staff id belonging to somebody else's reads exactly like an
//! id nobody has: `404`, with no way to tell the two apart.
//!
//! **The rules live in the repository, not here.** The two guard rails and the
//! one fixed refusal order all need the same lock and the same read, so putting
//! any of them in a handler would mean deciding them outside the transaction
//! that acts on them. What these handlers do is read the request, turn the one
//! conflict that belongs beside a form control into a field error, and hand the
//! rest through unchanged.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::Deserialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::application::ports::PasswordHasher as _;
use crate::domain::credentials::{EmailAddress, Password};
use crate::domain::error::{ConflictKind, DomainError, FieldError, FieldErrors};
use crate::domain::ids::StaffId;
use crate::infrastructure::db::repository::staff;
use crate::presentation::dto::{RoleDto, StaffMemberDto};
use crate::presentation::error::{ApiError, ErrorBody};
use crate::presentation::extract::{Actor, Admin, JsonBody};
use crate::presentation::state::AppState;

use super::auth::password_problem;

/// The longest display name accepted, in characters.
///
/// The same ceiling the `staff_display_name_length` check constraint holds
/// underneath. Both, deliberately: this one tells an admin which box is wrong,
/// and that one stops a path that forgot to ask.
const MAXIMUM_DISPLAY_NAME_CHARACTERS: usize = 80;

/// Everybody who works here, in one response.
#[derive(Debug, serde::Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StaffListResponse {
    /// Active people first, then by display name ignoring letter case, then by
    /// id. Deactivated people are included, in the section below.
    pub staff: Vec<StaffMemberDto>,
}

/// What creating a member of staff asks for.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateStaffRequest {
    /// What to call them on screen. At most 80 characters once trimmed.
    pub display_name: String,
    /// The address they will sign in with. Unique across the whole platform.
    pub email: String,
    /// The password the admin is about to hand them. They must replace it
    /// before they can do anything else.
    pub password: String,
    /// What they are allowed to be.
    pub role: RoleDto,
}

/// What renaming somebody asks for.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct EditStaffRequest {
    /// What they should be called.
    pub display_name: String,
    /// The version the edit form loaded. An older one is refused as stale.
    pub version: i32,
}

/// What changing somebody's role asks for.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChangeRoleRequest {
    /// What they should be allowed to be.
    pub role: RoleDto,
    /// The version the form loaded. An older one is refused as stale.
    pub version: i32,
}

/// What resetting somebody's password asks for.
///
/// No version. An admin resetting a password has decided this person needs a
/// new one, and whether somebody renamed them meanwhile does not change that.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ResetPasswordRequest {
    /// The password the admin is about to hand them.
    pub password: String,
}

/// Everybody who works here. Admins only.
///
/// # Errors
///
/// Returns `401` if nobody is signed in, `403` for a waiter or a chef, and
/// `503` if the database is unavailable.
#[utoipa::path(
    get,
    path = "/api/staff",
    tag = "staff",
    responses(
        (status = 200, description = "Everybody who works here, active first. Admins only.", body = StaffListResponse),
        (status = 401, description = "Nobody is signed in.", body = ErrorBody),
        (status = 403, description = "Not an admin.", body = ErrorBody),
    )
)]
pub async fn list_staff(
    State(state): State<AppState>,
    actor: Actor<Admin>,
) -> Result<Json<StaffListResponse>, ApiError> {
    let mut tx = state.database.begin_scoped(actor.restaurant_id()).await?;
    let people = staff::list(&mut tx).await?;
    tx.commit().await?;

    Ok(Json(StaffListResponse {
        staff: people.into_iter().map(StaffMemberDto::from).collect(),
    }))
}

/// Adds somebody to the restaurant, with a password they must replace.
///
/// The response carries the new row and never the password or its hash. What
/// the admin hands over is what they typed into their own form, which the
/// browser still has; sending it back would be putting a plain password in a
/// response body for no gain.
///
/// # Errors
///
/// Returns `400` naming each field that was not accepted, including
/// `fields.email=already_taken`, `401` if nobody is signed in, `403` for a
/// waiter or a chef, and `503` if the database is unavailable.
#[utoipa::path(
    post,
    path = "/api/staff",
    tag = "staff",
    request_body = CreateStaffRequest,
    responses(
        (status = 201, description = "The new staff member. Admins only.", body = StaffMemberDto),
        (status = 400, description = "A field was not accepted.", body = ErrorBody),
        (status = 401, description = "Nobody is signed in.", body = ErrorBody),
        (status = 403, description = "Not an admin.", body = ErrorBody),
    )
)]
pub async fn create_staff(
    State(state): State<AppState>,
    actor: Actor<Admin>,
    JsonBody(request): JsonBody<CreateStaffRequest>,
) -> Result<(StatusCode, Json<StaffMemberDto>), ApiError> {
    let (display_name, email, password) = read_new_staff(&request)?;

    // Hashed before the transaction opens. `argon2id` costs tens of
    // milliseconds on purpose, and holding a transaction open across it would
    // be holding one open for no reason: there is nothing to be consistent with
    // yet.
    let password_hash = state.passwords.hash(password).await?;

    let mut tx = state.database.begin_scoped(actor.restaurant_id()).await?;

    let created = staff::create(
        &mut tx,
        &staff::NewStaff {
            display_name: &display_name,
            email: &email,
            password_hash: &password_hash,
            role: request.role.into(),
        },
        actor.staff_id(),
    )
    .await
    .map_err(taken_email)?;

    tx.commit().await?;

    tracing::info!(
        staff_id = %created.id,
        role = created.role.as_label(),
        "a member of staff was created"
    );

    Ok((StatusCode::CREATED, Json(created.into())))
}

/// Changes somebody's display name. Their sessions are left alone.
///
/// # Errors
///
/// Returns `409 staff_changed` if the stored version is newer,
/// `409 staff_inactive` if the account is switched off, `404` for an id this
/// restaurant does not have, `400` naming the field that was not accepted,
/// `401` if nobody is signed in, and `403` for a waiter or a chef.
#[utoipa::path(
    patch,
    path = "/api/staff/{id}",
    tag = "staff",
    params(("id" = Uuid, Path, description = "The staff member to rename.")),
    request_body = EditStaffRequest,
    responses(
        (status = 200, description = "The updated staff member. Admins only.", body = StaffMemberDto),
        (status = 400, description = "A field was not accepted.", body = ErrorBody),
        (status = 401, description = "Nobody is signed in.", body = ErrorBody),
        (status = 403, description = "Not an admin.", body = ErrorBody),
        (status = 404, description = "No such staff member here.", body = ErrorBody),
        (status = 409, description = "`staff_inactive`, or `staff_changed` if somebody changed them after the form loaded.", body = ErrorBody),
    )
)]
pub async fn edit_staff(
    State(state): State<AppState>,
    actor: Actor<Admin>,
    Path(staff_id): Path<Uuid>,
    JsonBody(request): JsonBody<EditStaffRequest>,
) -> Result<Json<StaffMemberDto>, ApiError> {
    let display_name = read_display_name(&request.display_name)?;

    let mut tx = state.database.begin_scoped(actor.restaurant_id()).await?;

    let edited = staff::rename(
        &mut tx,
        StaffId::from_uuid(staff_id),
        &display_name,
        request.version,
        actor.staff_id(),
    )
    .await?;

    tx.commit().await?;

    Ok(Json(edited.into()))
}

/// Changes what somebody is allowed to be, and signs their devices out.
///
/// # Errors
///
/// Returns `409 cannot_act_on_self`, `409 staff_inactive`, `409 last_admin`,
/// and `409 staff_changed` in that order when more than one applies, `404` for
/// an id this restaurant does not have, `401` if nobody is signed in, and `403`
/// for a waiter or a chef.
#[utoipa::path(
    put,
    path = "/api/staff/{id}/role",
    tag = "staff",
    params(("id" = Uuid, Path, description = "The staff member whose role is changing.")),
    request_body = ChangeRoleRequest,
    responses(
        (status = 200, description = "The updated staff member. Admins only.", body = StaffMemberDto),
        (status = 401, description = "Nobody is signed in.", body = ErrorBody),
        (status = 403, description = "Not an admin.", body = ErrorBody),
        (status = 404, description = "No such staff member here.", body = ErrorBody),
        (status = 409, description = "`cannot_act_on_self`, `staff_inactive`, `last_admin`, or `staff_changed`, reported in that order.", body = ErrorBody),
    )
)]
pub async fn change_role(
    State(state): State<AppState>,
    actor: Actor<Admin>,
    Path(staff_id): Path<Uuid>,
    JsonBody(request): JsonBody<ChangeRoleRequest>,
) -> Result<Json<StaffMemberDto>, ApiError> {
    let mut tx = state.database.begin_scoped(actor.restaurant_id()).await?;

    let changed = staff::change_role(
        &mut tx,
        StaffId::from_uuid(staff_id),
        request.role.into(),
        request.version,
        actor.staff_id(),
    )
    .await?;

    tx.commit().await?;

    Ok(Json(changed.into()))
}

/// Writes a new password onto somebody's row, and signs their devices out.
///
/// Answers `204`, carrying nothing. The admin already has the password: they
/// typed it, and their own form is what shows it to them once.
///
/// # Errors
///
/// Returns `400` with `fields.password` naming the rule it broke,
/// `409 cannot_act_on_self`, `409 staff_inactive`, `404` for an id this
/// restaurant does not have, `401` if nobody is signed in, and `403` for a
/// waiter or a chef.
#[utoipa::path(
    post,
    path = "/api/staff/{id}/password",
    tag = "staff",
    params(("id" = Uuid, Path, description = "The staff member whose password is being reset.")),
    request_body = ResetPasswordRequest,
    responses(
        (status = 204, description = "Reset, and every session of theirs ended. Admins only."),
        (status = 400, description = "The password broke a rule.", body = ErrorBody),
        (status = 401, description = "Nobody is signed in.", body = ErrorBody),
        (status = 403, description = "Not an admin.", body = ErrorBody),
        (status = 404, description = "No such staff member here.", body = ErrorBody),
        (status = 409, description = "`cannot_act_on_self` or `staff_inactive`.", body = ErrorBody),
    )
)]
pub async fn reset_password(
    State(state): State<AppState>,
    actor: Actor<Admin>,
    Path(staff_id): Path<Uuid>,
    JsonBody(request): JsonBody<ResetPasswordRequest>,
) -> Result<StatusCode, ApiError> {
    let password = Password::new(&request.password)
        .map_err(|_| field("password", password_problem(&request.password)))?;

    let password_hash = state.passwords.hash(password).await?;

    let mut tx = state.database.begin_scoped(actor.restaurant_id()).await?;

    staff::reset_password(
        &mut tx,
        StaffId::from_uuid(staff_id),
        &password_hash,
        actor.staff_id(),
    )
    .await?;

    tx.commit().await?;

    Ok(StatusCode::NO_CONTENT)
}

/// Switches an account off, and signs their devices out at once.
///
/// Never refused because of the service. Open visits, unserved rounds, and open
/// bills keep pointing at the row and this succeeds regardless.
///
/// # Errors
///
/// Returns `409 cannot_act_on_self`, `409 staff_inactive`, and `409 last_admin`
/// in that order when more than one applies, `404` for an id this restaurant
/// does not have, `401` if nobody is signed in, and `403` for a waiter or a
/// chef.
#[utoipa::path(
    post,
    path = "/api/staff/{id}/deactivate",
    tag = "staff",
    params(("id" = Uuid, Path, description = "The staff member to switch off.")),
    responses(
        (status = 200, description = "The switched off staff member. Admins only.", body = StaffMemberDto),
        (status = 401, description = "Nobody is signed in.", body = ErrorBody),
        (status = 403, description = "Not an admin.", body = ErrorBody),
        (status = 404, description = "No such staff member here.", body = ErrorBody),
        (status = 409, description = "`cannot_act_on_self`, `staff_inactive`, or `last_admin`, reported in that order.", body = ErrorBody),
    )
)]
pub async fn deactivate(
    State(state): State<AppState>,
    actor: Actor<Admin>,
    Path(staff_id): Path<Uuid>,
) -> Result<Json<StaffMemberDto>, ApiError> {
    let mut tx = state.database.begin_scoped(actor.restaurant_id()).await?;
    let switched_off =
        staff::deactivate(&mut tx, StaffId::from_uuid(staff_id), actor.staff_id()).await?;
    tx.commit().await?;

    Ok(Json(switched_off.into()))
}

/// Brings a switched off account back, exactly as it was.
///
/// # Errors
///
/// Returns `409 staff_inactive` if the account is already active, `404` for an
/// id this restaurant does not have, `401` if nobody is signed in, and `403`
/// for a waiter or a chef.
#[utoipa::path(
    post,
    path = "/api/staff/{id}/reactivate",
    tag = "staff",
    params(("id" = Uuid, Path, description = "The staff member to bring back.")),
    responses(
        (status = 200, description = "The staff member, active again. Admins only.", body = StaffMemberDto),
        (status = 401, description = "Nobody is signed in.", body = ErrorBody),
        (status = 403, description = "Not an admin.", body = ErrorBody),
        (status = 404, description = "No such staff member here.", body = ErrorBody),
        (status = 409, description = "`staff_inactive`: the account is already active.", body = ErrorBody),
    )
)]
pub async fn reactivate(
    State(state): State<AppState>,
    actor: Actor<Admin>,
    Path(staff_id): Path<Uuid>,
) -> Result<Json<StaffMemberDto>, ApiError> {
    let mut tx = state.database.begin_scoped(actor.restaurant_id()).await?;
    let brought_back =
        staff::reactivate(&mut tx, StaffId::from_uuid(staff_id), actor.staff_id()).await?;
    tx.commit().await?;

    Ok(Json(brought_back.into()))
}

// ===========================================================================
// Reading the request
// ===========================================================================

/// Validates a create, collecting every problem rather than the first.
///
/// Collecting matters: an admin standing beside a new waiter should fix
/// everything at once rather than discovering one problem per round trip.
fn read_new_staff(
    request: &CreateStaffRequest,
) -> Result<(String, EmailAddress, Password), ApiError> {
    let mut errors = FieldErrors::default();

    let display_name = request.display_name.trim();
    if display_name.is_empty() {
        errors.add("displayName", FieldError::Required);
    } else if display_name.chars().count() > MAXIMUM_DISPLAY_NAME_CHARACTERS {
        errors.add("displayName", FieldError::TooLong);
    }

    let email = EmailAddress::new(&request.email);
    if email.is_err() {
        errors.add(
            "email",
            if request.email.trim().is_empty() {
                FieldError::Required
            } else {
                FieldError::InvalidFormat
            },
        );
    }

    let password = Password::new(&request.password);
    if password.is_err() {
        errors.add("password", password_problem(&request.password));
    }

    if !errors.is_empty() {
        return Err(DomainError::InvalidFields(errors).into());
    }

    // Both are `Ok` here: a failure would have added an error above and
    // returned. `ok_or` rather than `expect` because the crate denies `expect`
    // outside tests and `main`, and the fallback is a correct, if unreachable,
    // refusal.
    let refuse = || DomainError::Invalid("the staff member could not be read".to_owned());

    Ok((
        display_name.to_owned(),
        email.map_err(|_| refuse())?,
        password.map_err(|_| refuse())?,
    ))
}

/// The trimmed name, or the field error saying what is wrong with it.
///
/// The trimmed value is what is returned and what is written, not merely what
/// was measured. Measuring the trimmed value and storing the padded one is how
/// a name that passed an 80 character rule ends up as 84 characters in a
/// column that refuses it.
fn read_display_name(raw: &str) -> Result<String, ApiError> {
    let trimmed = raw.trim();

    if trimmed.is_empty() {
        return Err(field("displayName", FieldError::Required));
    }

    if trimmed.chars().count() > MAXIMUM_DISPLAY_NAME_CHARACTERS {
        return Err(field("displayName", FieldError::TooLong));
    }

    Ok(trimmed.to_owned())
}

/// One field, one problem.
fn field(name: &str, error: FieldError) -> ApiError {
    DomainError::InvalidFields(FieldErrors::one(name, error)).into()
}

/// Puts a taken address beside the address box, the way registration does.
///
/// An address held by a deactivated person, and one held in another restaurant,
/// both arrive here identically, and both are reported identically. An admin
/// cannot learn from this screen whether an address belongs to somebody who
/// left their own restaurant or to somebody at a restaurant across town.
fn taken_email(error: DomainError) -> ApiError {
    match error {
        DomainError::Conflict(ConflictKind::EmailTaken) => {
            DomainError::InvalidFields(FieldErrors::one("email", FieldError::AlreadyTaken)).into()
        }
        other => other.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_request(display_name: &str, email: &str, password: &str) -> CreateStaffRequest {
        CreateStaffRequest {
            display_name: display_name.to_owned(),
            email: email.to_owned(),
            password: password.to_owned(),
            role: RoleDto::Waiter,
        }
    }

    /// covers: AC-3
    ///
    /// The trimmed name is the one stored, not merely the one measured. A rule
    /// that measures one value and writes another is the kind that passes every
    /// test written from the outside and fails at the column.
    #[test]
    fn the_name_that_comes_back_is_the_trimmed_one() {
        let (display_name, _, _) = read_new_staff(&create_request(
            "  Ada  ",
            "ada@example.com",
            "correct horse",
        ))
        .expect("a normal create");

        assert_eq!(display_name, "Ada");
        assert_eq!(
            read_display_name("  Ada Lovelace \n").expect("a padded name"),
            "Ada Lovelace"
        );
    }

    /// covers: AC-3
    #[test]
    fn a_blank_or_overlong_name_is_refused() {
        for raw in ["", "   ", "\t\n"] {
            read_display_name(raw).expect_err("a blank name should be refused");
        }

        read_display_name(&"a".repeat(MAXIMUM_DISPLAY_NAME_CHARACTERS))
            .expect("exactly the ceiling is accepted");
        read_display_name(&"a".repeat(MAXIMUM_DISPLAY_NAME_CHARACTERS + 1))
            .expect_err("one character over the ceiling is refused");
    }

    /// covers: AC-3
    ///
    /// The ceiling counts characters here and bytes on the password, and the
    /// two genuinely differ. A name of eighty Devanagari characters is well
    /// past eighty bytes and is a perfectly ordinary name.
    #[test]
    fn the_name_ceiling_counts_characters_rather_than_bytes() {
        let devanagari = "अ".repeat(MAXIMUM_DISPLAY_NAME_CHARACTERS);
        assert!(devanagari.len() > MAXIMUM_DISPLAY_NAME_CHARACTERS);

        read_display_name(&devanagari)
            .expect("eighty characters is eighty characters whatever they weigh");
    }

    /// covers: AC-3
    ///
    /// Everything wrong with the form arrives in one response, so an admin
    /// standing beside a new waiter fixes it once.
    #[test]
    fn every_problem_with_a_create_is_reported_together() {
        let refused = read_new_staff(&create_request("", "not-an-address", "short"))
            .expect_err("three problems should be refused");

        let response = axum::response::IntoResponse::into_response(refused);
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    /// covers: AC-1
    ///
    /// The address is stored exactly as typed. An admin who typed somebody's
    /// address with capitals should see it back that way, and the index is what
    /// makes it one account either way.
    #[test]
    fn the_address_keeps_the_capitalisation_the_admin_typed() {
        let (_, email, _) = read_new_staff(&create_request(
            "Ada",
            "Ada@Example.Com",
            "correct horse battery",
        ))
        .expect("a normal create");

        assert_eq!(email.as_str(), "Ada@Example.Com");
        assert_eq!(email.lowered(), "ada@example.com");
    }
}
