//! Registering, signing in, signing out, and asking who you are.
//!
//! Two things run through all of it.
//!
//! **The raw session token exists in exactly two places**: the `Set-Cookie`
//! header these handlers write, and the browser's own cookie jar. It is never
//! logged, never stored, and never returned in a body.
//!
//! **A refused sign in says nothing about why.** A wrong password and an
//! address nobody has produce the same status, the same code, the same body,
//! and comparable time, because an unknown address still costs one password
//! hash. Making the body identical is the easy half; the clock is the half that
//! gets forgotten.

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum_extra::extract::cookie::CookieJar;
use serde::Deserialize;
use utoipa::ToSchema;

use crate::application::ports::PasswordHasher as _;
use crate::domain::country::CountryCode;
use crate::domain::credentials::{EmailAddress, Password};
use crate::domain::error::{DomainError, DomainResult, FieldError, FieldErrors};
use crate::domain::ids::{RestaurantId, StaffId};
use crate::domain::session::SessionToken;
use crate::infrastructure::db::repository::{accounts, catalog, sessions};
use crate::presentation::cookie;
use crate::presentation::dto::IdentityBundle;
use crate::presentation::error::{ApiError, ErrorBody};
use crate::presentation::extract::{Actor, ClientAddress, JsonBody};
use crate::presentation::state::AppState;

/// What registration asks for.
///
/// Five fields, and that is the product decision showing through: trying this
/// costs a minute, not an afternoon of settings. Everything else about the
/// restaurant is derived from the country or changed later.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RegisterRequest {
    /// What the restaurant is called.
    pub restaurant_name: String,
    /// What to call the owner on screen.
    pub display_name: String,
    /// The address they will sign in with.
    pub email: String,
    /// Their password. At least 10 characters.
    pub password: String,
    /// Where the restaurant is, as an ISO 3166-1 alpha-2 code. Accepted in any
    /// case.
    pub country_code: String,
}

/// What signing in asks for.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SignInRequest {
    /// The address the account was registered with. Matched case insensitively.
    pub email: String,
    /// The password.
    pub password: String,
}

/// Registers a restaurant and signs its owner in as the admin.
///
/// One transaction. Any failure inside it, a duplicate address in particular,
/// leaves no restaurant, no staff row, and no session: the transaction is
/// simply dropped, and everything it wrote goes with it.
///
/// The five settings the restaurant starts with come from the country's row in
/// `locales/countries.json` and from nowhere else.
///
/// # Errors
///
/// Returns a `400` naming each field that was not accepted, including
/// `fields.email=already_taken` for an address that already has an account, and
/// a `503` if the database or the password hash is unavailable.
#[utoipa::path(
    post,
    path = "/api/auth/register",
    tag = "accounts",
    request_body = RegisterRequest,
    responses(
        (
            status = 200,
            description = "The restaurant and its admin now exist, and the response sets the session cookie.",
            body = IdentityBundle,
        ),
        (
            status = 400,
            description = "A field was not accepted. `fields.email=already_taken` when that address already has an account.",
            body = ErrorBody,
        ),
    )
)]
pub async fn register(
    State(state): State<AppState>,
    jar: CookieJar,
    client: ClientAddress,
    JsonBody(request): JsonBody<RegisterRequest>,
) -> Result<Response, ApiError> {
    // Before the password is hashed, and before anything is read. Registration
    // answers whether an address is already taken, so without a throttle it is
    // a way to work through a list of addresses at speed.
    state
        .database
        .record_login_attempt(&request.email, client.address())
        .await?;

    let (email, password, country) = read_registration(&request)?;

    let password_hash = state.passwords.hash(password).await?;

    // Minted here rather than by the database, so the transaction can be scoped
    // to the restaurant it is about to create. The tenant policy on
    // `restaurants` checks `id = current_restaurant_id()`, so inserting the row
    // this transaction is already scoped to passes its own check.
    let restaurant_id = RestaurantId::new();
    let mut tx = state.database.begin_scoped(restaurant_id).await?;

    let admin = accounts::register(
        &mut tx,
        &accounts::Registration {
            restaurant_name: &request.restaurant_name,
            country: country.settings()?,
            display_name: &request.display_name,
            email: &email,
            password_hash: &password_hash,
        },
    )
    .await
    .map_err(taken_email)?;

    let token = SessionToken::mint()?;
    sessions::open(&mut tx, admin, &token.hash()).await?;
    sessions::mark_signed_in(&mut tx, admin).await?;

    let bundle = bundle_for(&mut tx, admin).await?;

    tx.commit().await?;

    // Same reason as the one in `sign_in`: the attempt above was recorded
    // before any of this ran, and a registration that worked must not spend one
    // of the five tries the owner gets at their own password a minute later.
    let cleared = state
        .database
        .clear_login_attempts(email.as_str())
        .await
        .unwrap_or_else(|error| {
            tracing::warn!(error = %error, "could not clear this address's login attempts");
            0
        });

    tracing::info!(
        restaurant_id = %restaurant_id,
        staff_id = %admin,
        cleared_attempts = cleared,
        "a restaurant was registered"
    );

    Ok(signed_in(jar, &token, &state, bundle))
}

/// Signs somebody in.
///
/// The address is looked up across every restaurant, because sign in has a
/// chicken and egg problem: there is no restaurant to scope to until the
/// account is found. That lookup is one of exactly two in the system that read
/// across restaurants, and it is `SECURITY DEFINER` in the database with a
/// fixed narrow shape.
///
/// # Errors
///
/// Returns `401` for a wrong password, an address nobody has, and a deactivated
/// account alike, and a `503` if the database or the password hash is
/// unavailable.
#[utoipa::path(
    post,
    path = "/api/auth/sign-in",
    tag = "accounts",
    request_body = SignInRequest,
    responses(
        (
            status = 200,
            description = "Signed in, and the response sets the session cookie.",
            body = IdentityBundle,
        ),
        (
            status = 401,
            description = "Identical for a wrong password and for an address nobody has.",
            body = ErrorBody,
        ),
    )
)]
pub async fn sign_in(
    State(state): State<AppState>,
    jar: CookieJar,
    client: ClientAddress,
    JsonBody(request): JsonBody<SignInRequest>,
) -> Result<Response, ApiError> {
    // Before the lookup and before the hash. Counting only failures would let
    // an attacker who guesses right on the fifth try never be counted at all,
    // and counting after the hash would let a flood of attempts each cost a
    // full argon2 verification before being refused.
    state
        .database
        .record_login_attempt(&request.email, client.address())
        .await?;

    // Deliberately not through the newtypes' validation. A refused sign in must
    // look the same whatever was typed, and telling somebody their address is
    // malformed is a different answer from telling them it did not work.
    let email = request.email.trim().to_owned();
    let password = Password::new(&request.password);

    let found = state.database.find_staff_for_login(&email).await?;

    let Some(credentials) = found else {
        // No account. Spend the same time anyway, then give the same answer.
        state.passwords.verify_nothing().await?;
        return Err(refused(&email));
    };

    // A password that could never have been set cannot match a stored hash, but
    // it still costs a hash to say so, for the same reason.
    let Ok(password) = password else {
        state.passwords.verify_nothing().await?;
        return Err(refused(&email));
    };

    let matched = state
        .passwords
        .verify(password, credentials.password_hash)
        .await?;

    if !matched {
        return Err(refused(&email));
    }

    if credentials.deactivated_at.is_some() {
        // A switched off account is refused exactly like a wrong password, so
        // somebody who has left cannot learn that their account still exists.
        tracing::info!(
            staff_id = %credentials.staff_id,
            "refusing a sign in for a deactivated account"
        );
        return Err(refused(&email));
    }

    let mut tx = state
        .database
        .begin_scoped(credentials.restaurant_id)
        .await?;

    let token = SessionToken::mint()?;
    sessions::open(&mut tx, credentials.staff_id, &token.hash()).await?;
    sessions::mark_signed_in(&mut tx, credentials.staff_id).await?;

    // Both sweeps are scoped to the person signing in, so a routine sign in is
    // never a platform wide write.
    let swept = sessions::sweep_dead(&mut tx, credentials.staff_id).await?;

    let bundle = bundle_for(&mut tx, credentials.staff_id).await?;

    tx.commit().await?;

    // The other sweep, on its own connection because `login_attempts` is the
    // one table outside every restaurant. Scoped to this one address, and its
    // failure is not this person's problem: they are already signed in.
    //
    // This is also what stops the throttle counting the sign in that just
    // worked. The attempt was recorded before the password was checked, so the
    // bucket only means "failures" because holding the account empties it.
    let swept_attempts = state
        .database
        .clear_login_attempts(&email)
        .await
        .unwrap_or_else(|error| {
            tracing::warn!(error = %error, "could not clear this address's login attempts");
            0
        });

    tracing::info!(
        staff_id = %credentials.staff_id,
        restaurant_id = %credentials.restaurant_id,
        swept_sessions = swept,
        swept_attempts,
        "signed in"
    );

    Ok(signed_in(jar, &token, &state, bundle))
}

/// Signs out exactly the session that made the request.
///
/// Every other session belonging to the same person keeps working, which is
/// what makes signing out on the house phone at the end of a shift safe to do.
///
/// # Errors
///
/// Returns `401` if nobody is signed in, and a `503` if the database is
/// unavailable.
#[utoipa::path(
    post,
    path = "/api/auth/sign-out",
    tag = "accounts",
    responses(
        (status = 204, description = "Signed out, and the response clears the cookie."),
        (status = 401, description = "Nobody was signed in.", body = ErrorBody),
    )
)]
pub async fn sign_out(
    State(state): State<AppState>,
    jar: CookieJar,
    actor: Actor,
) -> Result<Response, ApiError> {
    let mut tx = state.database.begin_scoped(actor.restaurant_id()).await?;
    sessions::revoke(&mut tx, actor.session_id()).await?;
    tx.commit().await?;

    tracing::info!(staff_id = %actor.staff_id(), "signed out");

    Ok((
        jar.add(cookie::clear(state.environment)),
        StatusCode::NO_CONTENT,
    )
        .into_response())
}

/// Who is signed in, and where they work.
///
/// The same bundle sign in returns, so a browser that has reloaded gets back to
/// exactly the state it had without a second endpoint or a second shape.
///
/// # Errors
///
/// Returns `401` if nobody is signed in, and a `503` if the database is
/// unavailable.
#[utoipa::path(
    get,
    path = "/api/me",
    tag = "accounts",
    responses(
        (status = 200, description = "The signed in identity.", body = IdentityBundle),
        (status = 401, description = "Nobody is signed in.", body = ErrorBody),
    )
)]
pub async fn me(
    State(state): State<AppState>,
    actor: Actor,
) -> Result<axum::Json<IdentityBundle>, ApiError> {
    let mut tx = state.database.begin_scoped(actor.restaurant_id()).await?;
    let bundle = bundle_for(&mut tx, actor.staff_id()).await?;
    tx.commit().await?;

    Ok(axum::Json(bundle))
}

/// Reads the identity bundle inside a transaction that is already scoped.
///
/// One function so the five endpoints that return it cannot drift into five
/// slightly different answers.
async fn bundle_for(
    tx: &mut crate::infrastructure::db::ScopedTx<'_>,
    staff_id: StaffId,
) -> DomainResult<IdentityBundle> {
    let staff = accounts::staff_by_id(tx, staff_id).await?;
    let restaurant = catalog::restaurant(tx).await?;

    Ok(IdentityBundle {
        staff: staff.into(),
        restaurant: restaurant.into(),
    })
}

/// The response both register and sign in return: the bundle, plus the cookie.
fn signed_in(
    jar: CookieJar,
    token: &SessionToken,
    state: &AppState,
    bundle: IdentityBundle,
) -> Response {
    (
        jar.add(cookie::issue(token.cookie_value(), state.environment)),
        axum::Json(bundle),
    )
        .into_response()
}

/// Validates a registration, collecting every problem rather than the first.
///
/// Collecting matters: somebody filling in a form should fix everything at once
/// rather than discovering one problem per round trip.
fn read_registration(
    request: &RegisterRequest,
) -> Result<(EmailAddress, Password, CountryCode), ApiError> {
    let mut errors = FieldErrors::default();

    if request.restaurant_name.trim().is_empty() {
        errors.add("restaurantName", FieldError::Required);
    }
    if request.display_name.trim().is_empty() {
        errors.add("displayName", FieldError::Required);
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

    let country = CountryCode::new(&request.country_code);
    if country.is_err() {
        errors.add(
            "countryCode",
            if request.country_code.trim().is_empty() {
                FieldError::Required
            } else {
                FieldError::UnknownCountry
            },
        );
    }

    if !errors.is_empty() {
        return Err(DomainError::InvalidFields(errors).into());
    }

    // Every one of the three is `Ok` here: a failure would have added an error
    // above and returned. `ok_or` rather than `expect` because the crate denies
    // `expect` outside tests and `main`, and the fallback is a correct, if
    // unreachable, refusal.
    let refuse = || DomainError::Invalid("the registration could not be read".to_owned());

    Ok((
        email.map_err(|_| refuse())?,
        password.map_err(|_| refuse())?,
        country.map_err(|_| refuse())?,
    ))
}

/// Which way a password broke its rule, so the screen can say which.
pub(super) fn password_problem(raw: &str) -> FieldError {
    use crate::domain::credentials::MAXIMUM_PASSWORD_BYTES;

    if raw.trim().len() > MAXIMUM_PASSWORD_BYTES {
        FieldError::TooLong
    } else {
        FieldError::TooShort
    }
}

/// Turns the repository's duplicate address conflict into the field error the
/// registration form shows beside the address box.
fn taken_email(error: DomainError) -> ApiError {
    match error {
        DomainError::Conflict(_) => {
            DomainError::InvalidFields(FieldErrors::one("email", FieldError::AlreadyTaken)).into()
        }
        other => other.into(),
    }
}

/// The one refusal a failed sign in ever produces.
///
/// A truncated hash of the lowered address goes to the log as a correlation
/// value, never the address itself: this is the platform's first personal data
/// and a log of who tried to sign in and failed is a log of who has an account
/// here.
fn refused(email: &str) -> ApiError {
    tracing::info!(
        attempt = %correlation_of(email),
        "refusing a sign in"
    );

    DomainError::Unauthenticated.into()
}

/// Eight hex characters of the lowered address's hash.
///
/// Enough to tie a run of failures together in a log, far too little to
/// recover the address from.
fn correlation_of(email: &str) -> String {
    use sha2::{Digest as _, Sha256};

    use std::fmt::Write as _;

    let digest = Sha256::digest(email.to_lowercase().as_bytes());

    digest
        .iter()
        .take(4)
        .fold(String::new(), |mut value, byte| {
            // Writing into a `String` cannot fail, and the crate denies `expect`
            // outside tests, so the result is dropped rather than unwrapped. A
            // correlation value that came out short would still correlate.
            let _ = write!(value, "{byte:02x}");
            value
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registration() -> RegisterRequest {
        RegisterRequest {
            restaurant_name: "The Test Kitchen".to_owned(),
            display_name: "Ada Admin".to_owned(),
            email: "Ada@Example.com".to_owned(),
            password: "correct horse battery".to_owned(),
            country_code: "in".to_owned(),
        }
    }

    fn refusal_fields(request: &RegisterRequest) -> Vec<(String, FieldError)> {
        let error = read_registration(request).expect_err("this registration should be refused");

        // The rejection is opaque on purpose, so the body is what is inspected,
        // exactly as a caller would.
        let response = error.into_response();
        let bytes =
            futures::executor::block_on(axum::body::to_bytes(response.into_body(), usize::MAX))
                .expect("the error body is small");
        let json: serde_json::Value = serde_json::from_slice(&bytes).expect("JSON");

        json["fields"]
            .as_object()
            .expect("a field error carries a fields member")
            .iter()
            .map(|(field, code)| {
                let code = code.as_str().expect("a code is a string");
                let error = [
                    FieldError::AlreadyTaken,
                    FieldError::TooShort,
                    FieldError::TooLong,
                    FieldError::InvalidFormat,
                    FieldError::UnknownCountry,
                    FieldError::NotInCatalogue,
                    FieldError::Incorrect,
                    FieldError::Required,
                ]
                .into_iter()
                .find(|candidate| candidate.as_code() == code)
                .expect("the code is one of the closed set");

                (field.clone(), error)
            })
            .collect()
    }

    /// covers: AC-1
    #[test]
    fn a_complete_registration_is_accepted_and_normalises_what_it_should() {
        let (email, _password, country) =
            read_registration(&registration()).expect("a complete registration");

        assert_eq!(
            email.as_str(),
            "Ada@Example.com",
            "the address was flattened, so the bundle will not show what was typed"
        );
        assert_eq!(
            country.as_str(),
            "IN",
            "a lower case country was not raised"
        );
    }

    /// covers: AC-2
    #[test]
    fn a_blank_required_field_is_reported_as_required() {
        let mut request = registration();
        request.restaurant_name = "   ".to_owned();

        assert_eq!(
            refusal_fields(&request),
            [("restaurantName".to_owned(), FieldError::Required)]
        );
    }

    /// covers: AC-1
    #[test]
    fn a_country_the_platform_does_not_serve_is_named_as_such() {
        let mut request = registration();
        request.country_code = "ZZ".to_owned();

        assert_eq!(
            refusal_fields(&request),
            [("countryCode".to_owned(), FieldError::UnknownCountry)]
        );
    }

    /// covers: AC-5
    #[test]
    fn a_password_says_which_way_it_broke_the_rule() {
        let mut request = registration();

        request.password = "short".to_owned();
        assert_eq!(
            refusal_fields(&request),
            [("password".to_owned(), FieldError::TooShort)]
        );

        request.password = "a".repeat(100);
        assert_eq!(
            refusal_fields(&request),
            [("password".to_owned(), FieldError::TooLong)]
        );
    }

    /// covers: AC-2
    ///
    /// Every problem with the form arrives together. One per round trip is how
    /// a five field form takes five attempts to fill in.
    #[test]
    fn every_problem_with_a_form_arrives_in_one_response() {
        let request = RegisterRequest {
            restaurant_name: String::new(),
            display_name: String::new(),
            email: "not-an-address".to_owned(),
            password: "short".to_owned(),
            country_code: "ZZ".to_owned(),
        };

        let fields = refusal_fields(&request);

        assert_eq!(fields.len(), 5, "only some of the problems were reported");
    }

    /// The first personal data this platform holds. A log line naming who
    /// failed to sign in is a log of which addresses have accounts here.
    #[test]
    fn a_failed_sign_in_correlates_without_naming_anybody() {
        let value = correlation_of("Ada@Example.com");

        assert_eq!(
            value.len(),
            8,
            "a correlation value is eight hex characters"
        );
        assert!(!value.contains("ada"), "the address leaked into {value:?}");
        assert!(!value.contains('@'));

        // Case insensitive, so a run of attempts against one account ties
        // together however the address was capitalised each time.
        assert_eq!(value, correlation_of("ada@example.com"));
        assert_ne!(value, correlation_of("bob@example.com"));
    }
}
