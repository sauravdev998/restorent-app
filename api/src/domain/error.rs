//! The one error type the domain and application layers speak.
//!
//! It deliberately knows nothing about HTTP or about SQLx. The presentation
//! layer maps it to status codes, and the infrastructure layer maps database
//! failures into it. That keeps the dependency rule intact: nothing here points
//! outward.

use thiserror::Error;

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

    /// The action conflicts with the current state, for example a second open
    /// bill on a table that already has one.
    #[error("conflict: {0}")]
    Conflict(String),

    /// A dependency the request needed is not answering. Always a server fault,
    /// never the caller's.
    #[error("dependency unavailable: {0}")]
    Unavailable(String),
}

/// The usual result type for domain and application code.
pub type DomainResult<T> = Result<T, DomainError>;
