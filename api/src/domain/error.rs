//! The one error type the domain and application layers speak.
//!
//! It deliberately knows nothing about HTTP or about `SQLx`. The presentation
//! layer maps it to status codes, and the infrastructure layer maps database
//! failures into it. That keeps the dependency rule intact: nothing here points
//! outward.

use thiserror::Error;

use super::enums::{LineStatus, VisitStatus};

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
    /// The field is required and was not sent at all, or was sent blank.
    Required,
    /// Meant to be a number and is not one, such as `abc` in a price box.
    NotANumber,
    /// Below zero where zero is the least a value may be.
    Negative,
    /// Larger than the column holding it can store.
    TooLarge,
    /// More decimal places than the currency writes, such as `12.345` rupees.
    TooManyDecimals,
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
            Self::NotANumber => "not_a_number",
            Self::Negative => "negative",
            Self::TooLarge => "too_large",
            Self::TooManyDecimals => "too_many_decimals",
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

/// What exactly conflicted, as a closed vocabulary.
///
/// This replaces the free English sentence [`DomainError::Conflict`] used to
/// carry, and the reason is what a waiter reads. A refusal has to arrive on the
/// screen in the reader's own language, and a sentence written in Rust cannot
/// be translated by the browser. So the sentence stays here for the log, where
/// English is fine, and [`Self::as_code`] gives a stable word the web maps to a
/// translation key the way it already maps every other error code.
///
/// Closed on purpose. A new conflict means a variant here, a code below, and a
/// key on the web side, in one change; a `String` would let a new refusal reach
/// a screen as untranslated English with nothing failing.
///
/// Two variants carry the status they expected rather than naming it, because
/// the same conditional update helper raises them for several expectations and
/// the code has to say which one was missed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ConflictKind {
    /// That table already has a party at it.
    TableOccupied,
    /// The visit is not in the state this action needed it to be in.
    VisitNot(VisitStatus),
    /// The visit still has an open bill on it.
    VisitHasOpenBill,
    /// A dish on the visit has not been put on a bill.
    VisitHasUnbilledLine,
    /// The dish is not in the state this action needed it to be in.
    LineNot(LineStatus),
    /// The whole ticket is not waiting to be carried out.
    RoundNotReady,
    /// The bill this action targets has already closed.
    BillNotOpen,
    /// One of those dishes is on a bill that has already closed.
    LineOnClosedBill,
    /// The bill has already been closed once.
    BillAlreadyClosed,
    /// A dish on the bill has neither reached the table nor been cancelled.
    BillHasUnservedLines,
    /// The bill has nothing on it that counts.
    BillHasNoLines,
    /// The bill has to be closed before this can happen.
    BillNotClosed,
    /// Two sessions were minted with the same token, which means the random
    /// source repeated itself.
    SessionCollision,
    /// That email address already has an account.
    ///
    /// The one kind that never reaches the wire: `handlers/auth.rs` catches it
    /// and turns it into a field error on the email box, so the message lands
    /// beside the control rather than at the top of the form.
    EmailTaken,
    /// The dish changed after the edit form loaded it, so saving the form
    /// would write over that change.
    DishChanged,
    /// The category was renamed, archived, or restored after the rename form
    /// loaded it.
    CategoryChanged,
    /// A reorder named a different set of dishes or categories from the live
    /// one, because something was added, moved, or removed meanwhile.
    MenuChanged,
    /// The category still holds a live dish, so archiving it would leave that
    /// dish under a heading nobody can see.
    CategoryNotEmpty,
    /// The category a dish is being put into has been archived.
    CategoryArchived,
    /// A live dish or category already has that name.
    ///
    /// Reaches the wire as a conflict only from a restore. A create, a rename,
    /// and an edit catch it and turn it into `already_taken` on the name box,
    /// the way `EmailTaken` becomes a field error on registration.
    NameTaken,
    /// A dish in the basket was switched off or taken off the menu before the
    /// ticket went, so the whole ticket was refused.
    DishNotOrderable,
}

impl ConflictKind {
    /// The stable machine readable word the client branches on.
    ///
    /// Total over both carried enums, so a status that no repository currently
    /// expects still produces a real code rather than borrowing another one's.
    #[must_use]
    pub const fn as_code(self) -> &'static str {
        match self {
            Self::TableOccupied => "table_occupied",
            Self::VisitNot(VisitStatus::Open) => "visit_not_open",
            Self::VisitNot(VisitStatus::Closed) => "visit_not_closed",
            Self::VisitHasOpenBill => "visit_has_open_bill",
            Self::VisitHasUnbilledLine => "visit_has_unbilled_line",
            Self::LineNot(LineStatus::Queued) => "line_not_queued",
            Self::LineNot(LineStatus::Ready) => "line_not_ready",
            Self::LineNot(LineStatus::Served) => "line_not_served",
            Self::LineNot(LineStatus::Voided) => "line_not_voided",
            Self::RoundNotReady => "round_not_ready",
            Self::BillNotOpen => "bill_not_open",
            Self::LineOnClosedBill => "line_on_closed_bill",
            Self::BillAlreadyClosed => "bill_already_closed",
            Self::BillHasUnservedLines => "bill_has_unserved_lines",
            Self::BillHasNoLines => "bill_has_no_lines",
            Self::BillNotClosed => "bill_not_closed",
            Self::SessionCollision => "session_collision",
            Self::EmailTaken => "email_taken",
            Self::DishChanged => "dish_changed",
            Self::CategoryChanged => "category_changed",
            Self::MenuChanged => "menu_changed",
            Self::CategoryNotEmpty => "category_not_empty",
            Self::CategoryArchived => "category_archived",
            Self::NameTaken => "name_taken",
            Self::DishNotOrderable => "dish_not_orderable",
        }
    }
}

impl std::fmt::Display for ConflictKind {
    /// The English sentence, for a log line and for a test that reads one.
    ///
    /// Never rendered on a screen. The web reads [`Self::as_code`] and says it
    /// in whatever language the person is reading.
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let sentence = match self {
            Self::TableOccupied => "that table already has a party at it",
            Self::VisitNot(VisitStatus::Open) => "that party has already left",
            Self::VisitNot(VisitStatus::Closed) => "that party is still at the table",
            Self::VisitHasOpenBill => "a bill on this visit is still open",
            Self::VisitHasUnbilledLine => "a dish on this visit has not been put on a bill",
            Self::LineNot(LineStatus::Queued) => "that dish is no longer waiting to be cooked",
            Self::LineNot(LineStatus::Ready) => "that dish is not waiting to be carried out",
            Self::LineNot(LineStatus::Served) => "that dish has not reached the table",
            Self::LineNot(LineStatus::Voided) => "that dish has not been cancelled",
            Self::RoundNotReady => "that ticket is not waiting to be carried out",
            Self::BillNotOpen => "that bill is no longer open",
            Self::LineOnClosedBill => "one of those dishes is on a bill that has already closed",
            Self::BillAlreadyClosed => "that bill has already been closed",
            Self::BillHasUnservedLines => "a dish on this bill has not reached the table yet",
            Self::BillHasNoLines => "a bill with nothing on it cannot be closed",
            Self::BillNotClosed => "a bill has to be closed before it can be paid",
            Self::SessionCollision => "that session token is already in use",
            Self::EmailTaken => "that email address already has an account",
            Self::DishChanged => "that dish changed after the form was opened",
            Self::CategoryChanged => "that category changed after the form was opened",
            Self::MenuChanged => "the menu changed while it was being reordered",
            Self::CategoryNotEmpty => "that category still has dishes on the menu",
            Self::CategoryArchived => "that category is no longer on the menu",
            Self::NameTaken => "something live on the menu already has that name",
            Self::DishNotOrderable => "a dish in the basket cannot be ordered right now",
        };

        formatter.write_str(sentence)
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
    ///
    /// Carries a [`ConflictKind`] rather than a sentence, so the refusal
    /// reaches the browser as a code that can be said in the reader's own
    /// language. Still a single field, so `matches!(.., Conflict(_))` keeps
    /// working.
    #[error("conflict: {0}")]
    Conflict(ConflictKind),

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

#[cfg(test)]
mod tests {
    use super::*;

    /// Every kind, so the tests below run over all of them.
    const ALL: [ConflictKind; 25] = [
        ConflictKind::TableOccupied,
        ConflictKind::VisitNot(VisitStatus::Open),
        ConflictKind::VisitNot(VisitStatus::Closed),
        ConflictKind::VisitHasOpenBill,
        ConflictKind::VisitHasUnbilledLine,
        ConflictKind::LineNot(LineStatus::Queued),
        ConflictKind::LineNot(LineStatus::Ready),
        ConflictKind::LineNot(LineStatus::Served),
        ConflictKind::LineNot(LineStatus::Voided),
        ConflictKind::RoundNotReady,
        ConflictKind::BillNotOpen,
        ConflictKind::LineOnClosedBill,
        ConflictKind::BillAlreadyClosed,
        ConflictKind::BillHasUnservedLines,
        ConflictKind::BillHasNoLines,
        ConflictKind::BillNotClosed,
        ConflictKind::SessionCollision,
        ConflictKind::EmailTaken,
        ConflictKind::DishChanged,
        ConflictKind::CategoryChanged,
        ConflictKind::MenuChanged,
        ConflictKind::CategoryNotEmpty,
        ConflictKind::CategoryArchived,
        ConflictKind::NameTaken,
        ConflictKind::DishNotOrderable,
    ];

    /// covers: AC-12, AC-13
    ///
    /// Two kinds sharing a code would put one refusal's translated sentence in
    /// front of somebody the other one happened to. The whole reason the code
    /// exists is that it names what actually happened.
    #[test]
    fn no_two_conflict_kinds_share_a_code() {
        for kind in ALL {
            let clashes = ALL
                .into_iter()
                .filter(|other| other.as_code() == kind.as_code())
                .count();

            assert_eq!(
                clashes,
                1,
                "{kind:?} shares its code {:?} with another kind",
                kind.as_code()
            );
        }
    }

    /// The code is what the web maps to a translation key, so an empty or
    /// shouty one would either find no key or find the wrong one.
    #[test]
    fn every_code_is_a_lower_snake_case_word() {
        for kind in ALL {
            let code = kind.as_code();

            assert!(!code.is_empty(), "{kind:?} has an empty code");
            assert!(
                code.chars()
                    .all(|character| character.is_ascii_lowercase() || character == '_'),
                "{kind:?} has the code {code:?}, which is not lower snake case"
            );
        }
    }

    /// covers: AC-12
    ///
    /// The English sentence is for a log and a test. It must never be empty,
    /// because a log line saying "conflict: " tells whoever is on call nothing.
    #[test]
    fn every_kind_says_what_happened_in_english_for_the_log() {
        for kind in ALL {
            assert!(
                !kind.to_string().trim().is_empty(),
                "{kind:?} has no English sentence"
            );
        }
    }

    /// covers: AC-13
    ///
    /// The distinction the two carried statuses exist for. A chef who lost the
    /// race on a dish and a waiter who lost the race on serving one are told
    /// different things, and this is the only place that is decided.
    #[test]
    fn the_two_line_expectations_produce_the_two_different_codes() {
        assert_eq!(
            ConflictKind::LineNot(LineStatus::Queued).as_code(),
            "line_not_queued"
        );
        assert_eq!(
            ConflictKind::LineNot(LineStatus::Ready).as_code(),
            "line_not_ready"
        );
    }
}
