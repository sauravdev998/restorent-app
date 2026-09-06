//! The scoped operations every feature above this one builds on.
//!
//! Every function here takes a [`ScopedTx`](super::ScopedTx), which is a
//! transaction that already has `app.restaurant_id` set on it. There is no way
//! to obtain one except through `Database::begin_scoped`, so there is no way to
//! call any of this unscoped. The two exceptions, the sign in lookups, are
//! deliberately not here: they live on `Database` itself, because being unscoped
//! is the whole point of them and burying that among ordinary reads would hide
//! it.
//!
//! Three habits run through all of it:
//!
//! * **A state change is a conditional update naming the state it expects.**
//!   When two people act on the same dish at once, the loser's statement changes
//!   zero rows and is told the state moved, rather than overwriting it.
//! * **A ticket's status is never decided, only recomputed.** Every write that
//!   touches a line recomputes its ticket from all of that ticket's lines, in
//!   the same transaction.
//! * **Reads filter archived rows for you.** No caller has to remember, which is
//!   what stops an archived dish reappearing on one screen out of nine.

pub mod accounts;
pub mod audit;
pub mod billing;
pub mod catalog;
pub mod service;
pub mod sessions;

use crate::domain::error::DomainError;

/// Names the constraint a failed statement broke, if it broke one.
///
/// Used to turn a specific violation into the conflict the caller can act on,
/// such as "that table already has an open visit", instead of the blanket
/// "database unavailable" every other failure maps to.
pub(crate) fn violated_constraint(error: &sqlx::Error) -> Option<&str> {
    match error {
        sqlx::Error::Database(database_error) => database_error.constraint(),
        _ => None,
    }
}

/// Turns a named constraint violation into a conflict, and anything else into
/// whatever it already was.
pub(crate) fn conflict_on(
    error: sqlx::Error,
    constraint: &str,
    message: &'static str,
) -> DomainError {
    if violated_constraint(&error) == Some(constraint) {
        return DomainError::Conflict(message.to_owned());
    }

    DomainError::from(error)
}
