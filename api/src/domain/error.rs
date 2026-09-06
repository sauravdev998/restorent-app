//! The one error type the domain and application layers speak.
//!
//! It deliberately knows nothing about HTTP or about `SQLx`. The presentation
//! layer maps it to status codes, and the infrastructure layer maps database
//! failures into it. That keeps the dependency rule intact: nothing here points
//! outward.

use thiserror::Error;

/// What is wrong with one named field of a request.
///
/// A closed list rather than a sentence, because the words a person reads live
/// on the web side where they are translated with everything else. The API
/// names what happened; the interface says it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum FieldError {
    /// Somebody already has that value, and it has to be unique.
    AlreadyTaken,
    /// Shorter than the rule allows.
    TooShort,
    /// Longer than the rule allows.
    TooLong,
    /// Not shaped like the kind of value this field holds.
    InvalidFormat,
    /// Not a country this platform serves.
    UnknownCountry,
    /// Not a language or formatting locale `locales/catalogue.json` offers.
    NotInCatalogue,
    /// The value given does not match what is stored, as with a current
    /// password.
    Incorrect,
    /// The field is required and was not sent at all.
    Required,
}

impl FieldError {
    /// The exact string the client branches on, and maps to a translation key.
    #[must_use]
    pub const fn as_code(self) -> &'static str {
        match self {
            Self::AlreadyTaken => "already_taken",
            Self::TooShort => "too_short",
            Self::TooLong => "too_long",
            Self::InvalidFormat => "invalid_format",
            Self::UnknownCountry => "unknown_country",
            Self::NotInCatalogue => "not_in_catalogue",
            Self::Incorrect => "incorrect",
            Self::Required => "required",
        }
    }
}

/// Everything wrong with a request, field by field.
///
/// Ordered rather than a map, so the first problem found is the first one
/// reported and two runs of the same validation produce the same body.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FieldErrors(Vec<(String, FieldError)>);

impl FieldErrors {
    /// One problem with one field, which is the common case.
    #[must_use]
    pub fn one(field: &str, error: FieldError) -> Self {
        let mut errors = Self::default();
        errors.add(field, error);
        errors
    }

    /// Adds another one.
    pub fn add(&mut self, field: &str, error: FieldError) {
        self.0.push((field.to_owned(), error));
    }

    /// Whether anything is wrong at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Every problem, in the order they were found.
    #[must_use]
    pub fn pairs(&self) -> &[(String, FieldError)] {
        &self.0
    }
}

/// Anything that can go wrong while carrying out a use case.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum DomainError {
    /// The thing asked for does not exist, or does not exist for this tenant,
    /// which callers must not be able to tell apart.
    #[error("not found")]
    NotFound,

    /// The caller is not signed in.
    #[error("not authenticated")]
    Unauthenticated,

    /// The caller is signed in but not allowed to do this.
    #[error("forbidden")]
    Forbidden,

    /// The request itself is malformed or breaks a business rule.
    #[error("invalid request: {0}")]
    Invalid(String),

    /// Named fields of the request are wrong, and the caller can be told which.
    ///
    /// Separate from [`Self::Invalid`] because the interface does something
    /// different with it: it puts the message beside the control rather than at
    /// the top of the form.
    #[error("invalid request fields")]
    InvalidFields(FieldErrors),

    /// The action conflicts with the current state, for example a second open
    /// bill on a table that already has one.
    #[error("conflict: {0}")]
    Conflict(String),

    /// Too many attempts in too short a window. Carries how long until the
    /// window clears, in seconds, which becomes the `Retry-After` header.
    ///
    /// Never permanent: every throttle in this product is a time window that
    /// clears itself, so there is nothing for an admin to unlock.
    #[error("too many attempts")]
    Throttled(u64),

    /// A dependency the request needed is not answering. Always a server fault,
    /// never the caller's.
    #[error("dependency unavailable: {0}")]
    Unavailable(String),
}

/// The usual result type for domain and application code.
pub type DomainResult<T> = Result<T, DomainError>;
