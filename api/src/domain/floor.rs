//! The rules an admin's floor edits have to pass, with no database anywhere.
//!
//! Spec 0010. Every one of these is also held by the database: a length check
//! constraint, a seat range check, a partial unique index. Checking here as well
//! is what turns a refused statement, which reads to a caller as "the database
//! is unavailable", into a field error that names the box and the reason.

use super::error::{FieldError, FieldErrors};
use super::menu;

/// The longest a section name may be, in characters.
pub const SECTION_NAME_MAX: usize = 40;

/// The longest a table label may be, in characters.
pub const LABEL_MAX: usize = 12;

/// The fewest seats a table may record.
pub const SEATS_MIN: i64 = 1;

/// The most seats a table may record.
pub const SEATS_MAX: i64 = 50;

/// The lowest number a range may start or end on.
pub const RANGE_NUMBER_MIN: i64 = 1;

/// The highest number a range may start or end on.
pub const RANGE_NUMBER_MAX: i64 = 999;

/// The most tables one range may add.
pub const RANGE_COUNT_MAX: i64 = 50;

/// A section name, trimmed, or the reason it was refused.
///
/// # Errors
///
/// Returns [`FieldError::Required`] for a blank name and [`FieldError::TooLong`]
/// for one over [`SECTION_NAME_MAX`] characters.
pub fn section_name(value: &str) -> Result<String, FieldError> {
    menu::name(value, SECTION_NAME_MAX)
}

/// A table label, trimmed, or the reason it was refused.
///
/// # Errors
///
/// Returns [`FieldError::Required`] for a blank label and
/// [`FieldError::TooLong`] for one over [`LABEL_MAX`] characters.
pub fn label(value: &str) -> Result<String, FieldError> {
    menu::name(value, LABEL_MAX)
}

/// A seat count, where none at all is a real answer.
///
/// Taken as a wide integer so a value far outside the range is still reported
/// as too large, rather than failing to decode as a whole request.
///
/// # Errors
///
/// Returns [`FieldError::TooSmall`] below [`SEATS_MIN`] and
/// [`FieldError::TooLarge`] above [`SEATS_MAX`].
pub fn seats(value: Option<i64>) -> Result<Option<i16>, FieldError> {
    let Some(count) = value else {
        return Ok(None);
    };

    if count < SEATS_MIN {
        return Err(FieldError::TooSmall);
    }

    if count > SEATS_MAX {
        return Err(FieldError::TooLarge);
    }

    // Inside 1 to 50, so the conversion cannot fail. Answered as too large
    // anyway rather than with a panic.
    i16::try_from(count)
        .map(Some)
        .map_err(|_| FieldError::TooLarge)
}

/// A table form's label and seats, both checked, so an admin who got both
/// wrong is told about both at once.
///
/// # Errors
///
/// Returns every field that was refused, in form order.
pub fn table_fields(
    label_text: &str,
    seats_value: Option<i64>,
) -> Result<(String, Option<i16>), FieldErrors> {
    let mut errors = FieldErrors::default();

    let label = label(label_text)
        .map_err(|error| errors.add("label", error))
        .ok();
    let seats = seats(seats_value)
        .map_err(|error| errors.add("seats", error))
        .ok();

    match (label, seats) {
        (Some(label), Some(seats)) if errors.is_empty() => Ok((label, seats)),
        _ => Err(errors),
    }
}

/// What a numbered range asks for, once it has passed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableRange {
    /// Every label, in number order.
    pub labels: Vec<String>,
    /// The seats every table in the range shares.
    pub seats: Option<i16>,
}

/// Builds the labels of a numbered range, checking every field of the form.
///
/// The prefix loses its leading spaces and keeps its trailing ones, so `Bar `
/// then 1 gives `Bar 1`. A blank prefix means bare numbers. Numbers are written
/// plainly, with no leading zeros.
///
/// A label longer than [`LABEL_MAX`] is reported on the prefix, because the
/// prefix is the part the admin typed. It is only checked once both numbers
/// pass, since the longest label depends on the last number.
///
/// # Errors
///
/// Returns every field that was refused: `from` and `to` as
/// [`FieldError::TooSmall`] or [`FieldError::TooLarge`], `to` as
/// [`FieldError::BeforeStart`] below `from` or [`FieldError::TooMany`] past
/// [`RANGE_COUNT_MAX`] tables, `prefix` as [`FieldError::TooLong`], and
/// `seats` as for a single table.
pub fn table_range(
    prefix: Option<&str>,
    from: i64,
    to: i64,
    seats_value: Option<i64>,
) -> Result<TableRange, FieldErrors> {
    let mut errors = FieldErrors::default();

    let from_ok = range_number(from)
        .map_err(|error| errors.add("from", error))
        .is_ok();

    let to_ok = match range_number(to) {
        Err(error) => {
            errors.add("to", error);
            false
        }
        Ok(()) if from_ok && to < from => {
            errors.add("to", FieldError::BeforeStart);
            false
        }
        Ok(()) if from_ok && to - from + 1 > RANGE_COUNT_MAX => {
            errors.add("to", FieldError::TooMany);
            false
        }
        Ok(()) => true,
    };

    let prefix = prefix.map(str::trim_start).unwrap_or_default();

    let labels: Vec<String> = if from_ok && to_ok {
        (from..=to)
            .map(|number| format!("{prefix}{number}"))
            .collect()
    } else {
        Vec::new()
    };

    if labels.iter().any(|label| label.chars().count() > LABEL_MAX) {
        errors.add("prefix", FieldError::TooLong);
    }

    let seats = seats(seats_value)
        .map_err(|error| errors.add("seats", error))
        .ok()
        .flatten();

    if errors.is_empty() {
        Ok(TableRange { labels, seats })
    } else {
        Err(errors)
    }
}

/// One end of a range, inside 1 to 999.
fn range_number(value: i64) -> Result<(), FieldError> {
    if value < RANGE_NUMBER_MIN {
        return Err(FieldError::TooSmall);
    }

    if value > RANGE_NUMBER_MAX {
        return Err(FieldError::TooLarge);
    }

    Ok(())
}

/// Which of `wanted` would clash, given the live labels that matched them.
///
/// A label clashes when a live label equals it ignoring letter case, or when an
/// earlier label in `wanted` already does, since the two could not both be live.
/// The answer keeps `wanted`'s order and spelling, which is the order and
/// spelling the admin asked for.
#[must_use]
pub fn clashing_labels(wanted: &[String], live: &[String]) -> Vec<String> {
    let live: Vec<String> = live.iter().map(|label| label.to_lowercase()).collect();
    let mut seen: Vec<String> = Vec::with_capacity(wanted.len());
    let mut clashes = Vec::new();

    for label in wanted {
        let lowered = label.to_lowercase();

        if live.contains(&lowered) || seen.contains(&lowered) {
            clashes.push(label.clone());
        }

        seen.push(lowered);
    }

    clashes
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pairs(errors: &FieldErrors) -> Vec<(&str, FieldError)> {
        errors
            .pairs()
            .iter()
            .map(|(field, error)| (field.as_str(), *error))
            .collect()
    }

    fn refused(prefix: Option<&str>, from: i64, to: i64, seats: Option<i64>) -> FieldErrors {
        table_range(prefix, from, to, seats).expect_err("a bad range was accepted")
    }

    /// covers: AC-12
    #[test]
    fn a_label_is_trimmed_required_and_at_most_twelve_characters() {
        assert_eq!(label("  T1 "), Ok("T1".to_owned()));
        assert_eq!(label("   "), Err(FieldError::Required));
        assert_eq!(label(&"a".repeat(12)), Ok("a".repeat(12)));
        assert_eq!(label(&"a".repeat(13)), Err(FieldError::TooLong));
    }

    /// covers: AC-12
    #[test]
    fn a_section_name_is_trimmed_required_and_at_most_forty_characters() {
        assert_eq!(section_name(" Terrace "), Ok("Terrace".to_owned()));
        assert_eq!(section_name(""), Err(FieldError::Required));
        assert_eq!(section_name(&"छ".repeat(40)), Ok("छ".repeat(40)));
        assert_eq!(section_name(&"a".repeat(41)), Err(FieldError::TooLong));
    }

    /// covers: AC-12
    #[test]
    fn seats_are_empty_or_one_to_fifty() {
        assert_eq!(seats(None), Ok(None));
        assert_eq!(seats(Some(1)), Ok(Some(1)));
        assert_eq!(seats(Some(50)), Ok(Some(50)));
        assert_eq!(seats(Some(0)), Err(FieldError::TooSmall));
        assert_eq!(seats(Some(-3)), Err(FieldError::TooSmall));
        assert_eq!(seats(Some(51)), Err(FieldError::TooLarge));
        assert_eq!(seats(Some(i64::MAX)), Err(FieldError::TooLarge));
    }

    /// covers: AC-12
    #[test]
    fn a_table_form_reports_both_bad_fields_at_once() {
        let errors = table_fields(" ", Some(0)).expect_err("a bad form was accepted");
        assert_eq!(
            pairs(&errors),
            [
                ("label", FieldError::Required),
                ("seats", FieldError::TooSmall)
            ]
        );
        assert_eq!(table_fields(" T9 ", None), Ok(("T9".to_owned(), None)));
    }

    /// covers: AC-4
    #[test]
    fn a_range_writes_the_prefix_then_the_plain_number() {
        let range = table_range(Some("T"), 1, 3, Some(4)).expect("a good range");
        assert_eq!(range.labels, ["T1", "T2", "T3"]);
        assert_eq!(range.seats, Some(4));

        let bare = table_range(None, 9, 11, None).expect("a range with no prefix");
        assert_eq!(bare.labels, ["9", "10", "11"]);

        let blank = table_range(Some("   "), 1, 1, None).expect("a blank prefix");
        assert_eq!(blank.labels, ["1"]);
    }

    /// covers: AC-4
    ///
    /// The trailing space is what makes `Bar 1` rather than `Bar1`, so only the
    /// leading ones go.
    #[test]
    fn a_prefix_keeps_its_trailing_space_and_loses_its_leading_ones() {
        let range = table_range(Some("  Bar "), 1, 2, None).expect("a spaced prefix");
        assert_eq!(range.labels, ["Bar 1", "Bar 2"]);
    }

    /// covers: AC-4, AC-12
    ///
    /// Each refusal from the spec's range scenario, with its own code.
    #[test]
    fn every_bad_range_is_refused_with_its_own_reason() {
        assert_eq!(
            pairs(&refused(Some("T"), 1, 51, None)),
            [("to", FieldError::TooMany)]
        );
        assert_eq!(
            pairs(&refused(Some("T"), 5, 2, None)),
            [("to", FieldError::BeforeStart)]
        );
        assert_eq!(
            pairs(&refused(Some("T"), 0, 2, None)),
            [("from", FieldError::TooSmall)]
        );
        assert_eq!(
            pairs(&refused(Some("T"), 1, 1000, None)),
            [("to", FieldError::TooLarge)]
        );
        assert_eq!(
            pairs(&refused(Some("Rooftop bar "), 1, 10, None)),
            [("prefix", FieldError::TooLong)]
        );
        assert_eq!(
            pairs(&refused(None, 1, 2, Some(51))),
            [("seats", FieldError::TooLarge)]
        );
    }

    /// covers: AC-4
    ///
    /// Fifty is allowed and fifty one is not, counting both ends.
    #[test]
    fn a_range_holds_at_most_fifty_tables() {
        assert_eq!(
            table_range(None, 1, 50, None)
                .expect("fifty tables")
                .labels
                .len(),
            50
        );
        assert_eq!(
            pairs(&refused(None, 950, 1000, None)),
            [("to", FieldError::TooLarge)]
        );
    }

    /// A label only too long at the last number still refuses the prefix.
    #[test]
    fn the_prefix_is_too_long_when_only_the_last_label_passes_twelve() {
        // "Rooftop b" plus three digits is exactly 12. "Rooftop ba" plus 99 is
        // also 12, and only its last label, "Rooftop ba100", is 13.
        assert!(table_range(Some("Rooftop b"), 998, 999, None).is_ok());
        assert_eq!(
            pairs(&refused(Some("Rooftop ba"), 99, 100, None)),
            [("prefix", FieldError::TooLong)]
        );
    }

    /// covers: AC-4, AC-11
    #[test]
    fn clashes_keep_the_requested_order_and_spelling() {
        let wanted = ["t1", "t2", "t3", "t4"].map(str::to_owned);
        let live = ["T3", "t1"].map(str::to_owned);

        assert_eq!(clashing_labels(&wanted, &live), ["t1", "t3"]);
        assert!(clashing_labels(&wanted, &[]).is_empty());
    }

    /// covers: AC-11
    ///
    /// Two archived tables called `P1` and `p1` cannot both come back.
    #[test]
    fn a_label_repeated_in_the_request_clashes_with_itself() {
        let wanted = ["P1", "P2", "p1"].map(str::to_owned);

        assert_eq!(clashing_labels(&wanted, &[]), ["p1"]);
    }
}
