//! The rules an admin's menu edits have to pass, with no database anywhere.
//!
//! Every one of these is also held by the database: a length check constraint,
//! a partial unique index, `price >= 0`. Checking here as well is what turns a
//! refused statement, which reads to a caller as "the database is unavailable",
//! into a field error that names the box and the reason. The API is the
//! authority on every rule; the web's own checks are a courtesy.

use std::str::FromStr as _;

use rust_decimal::Decimal;
use uuid::Uuid;

use super::error::{FieldError, FieldErrors};

/// The longest a category name may be, in characters.
pub const CATEGORY_NAME_MAX: usize = 60;

/// The longest a dish name may be, in characters.
pub const DISH_NAME_MAX: usize = 80;

/// The longest a dish description may be, in characters.
pub const DESCRIPTION_MAX: usize = 300;

/// The first price `numeric(14,4)` cannot hold: ten digits before the point.
const PRICE_CEILING: i64 = 10_000_000_000;

/// A name, trimmed, or the reason it was refused.
///
/// Counted in characters rather than bytes, the same as `char_length` in the
/// check constraint, so "मटर पनीर" is eight long here and eight long there.
///
/// # Errors
///
/// Returns [`FieldError::Required`] for a blank name and [`FieldError::TooLong`]
/// for one over `max` characters.
pub fn name(value: &str, max: usize) -> Result<String, FieldError> {
    let trimmed = value.trim();

    if trimmed.is_empty() {
        return Err(FieldError::Required);
    }

    if trimmed.chars().count() > max {
        return Err(FieldError::TooLong);
    }

    Ok(trimmed.to_owned())
}

/// A description, trimmed, with a blank one read as none at all.
///
/// # Errors
///
/// Returns [`FieldError::TooLong`] for one over [`DESCRIPTION_MAX`] characters.
pub fn description(value: Option<&str>) -> Result<Option<String>, FieldError> {
    let Some(trimmed) = value.map(str::trim).filter(|text| !text.is_empty()) else {
        return Ok(None);
    };

    if trimmed.chars().count() > DESCRIPTION_MAX {
        return Err(FieldError::TooLong);
    }

    Ok(Some(trimmed.to_owned()))
}

/// A price, from the decimal string the form sent, checked against the
/// restaurant's currency.
///
/// Only ASCII digits with at most one `.` are a number here. The web turns a
/// locale's own decimal comma into a point before it sends anything, so a
/// comma arriving at the API is a grouping separator somebody typed, and
/// guessing which it meant is how `1,500` becomes one and a half.
///
/// Trailing zeros are not counted as decimal places: `12.50` is a fine price in
/// a currency with two, and so is `12.500`, because both are the same amount.
/// What is refused is precision the currency cannot write, such as `12.345`
/// rupees.
///
/// # Errors
///
/// Returns [`FieldError::Required`] for a blank price,
/// [`FieldError::NotANumber`] for anything that is not a plain decimal,
/// [`FieldError::Negative`] below zero, [`FieldError::TooLarge`] at ten
/// billion or more, and [`FieldError::TooManyDecimals`] for more places than
/// `currency_decimals`.
pub fn price(value: &str, currency_decimals: u32) -> Result<Decimal, FieldError> {
    let trimmed = value.trim();

    if trimmed.is_empty() {
        return Err(FieldError::Required);
    }

    let unsigned = trimmed.strip_prefix('-').unwrap_or(trimmed);

    if !is_plain_decimal(unsigned) {
        return Err(FieldError::NotANumber);
    }

    // Anything the syntax check let through that still does not parse is a
    // run of digits longer than a `Decimal` holds, which is too large rather
    // than not a number.
    let parsed = Decimal::from_str(trimmed).map_err(|_| FieldError::TooLarge)?;

    if parsed.is_sign_negative() && !parsed.is_zero() {
        return Err(FieldError::Negative);
    }

    if parsed >= Decimal::from(PRICE_CEILING) {
        return Err(FieldError::TooLarge);
    }

    if parsed.normalize().scale() > currency_decimals {
        return Err(FieldError::TooManyDecimals);
    }

    // `-0` parses, and is zero, and would be stored with its sign.
    Ok(parsed.abs())
}

/// Digits, optionally with one point that has digits on both sides.
fn is_plain_decimal(value: &str) -> bool {
    let mut parts = value.split('.');

    let whole = parts.next().unwrap_or_default();
    let fraction = parts.next();

    if parts.next().is_some() {
        return false;
    }

    let digits = |part: &str| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit());

    digits(whole) && fraction.is_none_or(digits)
}

/// Everything wrong with a dish form, or its values once they pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DishFields {
    /// The name, trimmed.
    pub name: String,
    /// The description, trimmed, or none.
    pub description: Option<String>,
    /// The price.
    pub price: Decimal,
}

/// Checks the three free text fields of a dish form together, so an admin who
/// got two of them wrong is told about both at once.
///
/// The field names are the ones on the wire, which is what the web reads to put
/// each message beside its own box.
///
/// # Errors
///
/// Returns every field that was refused, in form order.
pub fn dish_fields(
    name_text: &str,
    description_text: Option<&str>,
    price_text: &str,
    currency_decimals: u32,
) -> Result<DishFields, FieldErrors> {
    let mut errors = FieldErrors::default();

    let name = name(name_text, DISH_NAME_MAX)
        .map_err(|error| errors.add("name", error))
        .ok();
    let description = description(description_text)
        .map_err(|error| errors.add("description", error))
        .ok();
    let price = price(price_text, currency_decimals)
        .map_err(|error| errors.add("price", error))
        .ok();

    match (name, description, price) {
        (Some(name), Some(description), Some(price)) if errors.is_empty() => Ok(DishFields {
            name,
            description,
            price,
        }),
        _ => Err(errors),
    }
}

/// Whether a reorder names exactly the live list, each member once.
///
/// Same length and the same set is the whole test, and it has to be both: a
/// list with one id repeated in place of a missing one has the right length,
/// and a list naming every live id plus one extra has the right members.
#[must_use]
pub fn is_the_same_list(sent: &[Uuid], live: &[Uuid]) -> bool {
    if sent.len() != live.len() {
        return false;
    }

    let mut sent_sorted = sent.to_vec();
    let mut live_sorted = live.to_vec();
    sent_sorted.sort_unstable();
    live_sorted.sort_unstable();

    sent_sorted == live_sorted
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decimal(text: &str) -> Decimal {
        Decimal::from_str(text).expect("a test decimal")
    }

    /// covers: AC-14
    #[test]
    fn a_name_is_trimmed_and_must_have_something_in_it() {
        assert_eq!(
            name("  Starters  ", CATEGORY_NAME_MAX),
            Ok("Starters".to_owned())
        );
        assert_eq!(name("   ", CATEGORY_NAME_MAX), Err(FieldError::Required));
        assert_eq!(name("", DISH_NAME_MAX), Err(FieldError::Required));
    }

    /// covers: AC-14
    ///
    /// Characters, not bytes. A Hindi dish name is three bytes a character, so
    /// a byte count would refuse a name a third the length the rule allows.
    #[test]
    fn a_name_is_measured_in_characters_the_way_the_database_measures_it() {
        let longest = "a".repeat(DISH_NAME_MAX);
        assert_eq!(name(&longest, DISH_NAME_MAX), Ok(longest.clone()));
        assert_eq!(
            name(&format!("{longest}a"), DISH_NAME_MAX),
            Err(FieldError::TooLong)
        );

        let hindi = "प".repeat(CATEGORY_NAME_MAX);
        assert_eq!(name(&hindi, CATEGORY_NAME_MAX), Ok(hindi.clone()));
    }

    /// covers: AC-14
    #[test]
    fn a_blank_description_is_no_description() {
        assert_eq!(description(None), Ok(None));
        assert_eq!(description(Some("   ")), Ok(None));
        assert_eq!(
            description(Some("  Slow cooked  ")),
            Ok(Some("Slow cooked".to_owned()))
        );
        assert_eq!(
            description(Some(&"a".repeat(DESCRIPTION_MAX + 1))),
            Err(FieldError::TooLong)
        );
    }

    /// covers: AC-14
    #[test]
    fn a_plain_price_in_the_currency_passes() {
        assert_eq!(price("320", 2), Ok(decimal("320")));
        assert_eq!(price("320.50", 2), Ok(decimal("320.50")));
        assert_eq!(price(" 0 ", 2), Ok(Decimal::ZERO));
        assert_eq!(price("9999999999.99", 2), Ok(decimal("9999999999.99")));
    }

    /// covers: AC-14
    ///
    /// Each refusal from the spec's validation scenario, with its own code.
    #[test]
    fn every_bad_price_is_refused_with_its_own_reason() {
        assert_eq!(price("", 2), Err(FieldError::Required));
        assert_eq!(price("abc", 2), Err(FieldError::NotANumber));
        assert_eq!(price("-1", 2), Err(FieldError::Negative));
        assert_eq!(price("10000000000", 2), Err(FieldError::TooLarge));
        assert_eq!(price("12.345", 2), Err(FieldError::TooManyDecimals));
    }

    /// covers: AC-14
    ///
    /// A comma is a grouping separator by the time it reaches the API, because
    /// the web has already turned a locale's decimal comma into a point.
    /// Reading `1,500` as either number would be a guess.
    #[test]
    fn anything_but_digits_and_one_point_is_not_a_number() {
        for text in [
            "1,500", "1.2.3", ".5", "5.", "1e3", "+5", "₹5", "--1", "1 000",
        ] {
            assert_eq!(
                price(text, 2),
                Err(FieldError::NotANumber),
                "{text:?} was read as a number"
            );
        }
    }

    /// covers: AC-14
    #[test]
    fn a_currency_with_no_decimals_takes_none() {
        assert_eq!(price("1500", 0), Ok(decimal("1500")));
        assert_eq!(price("1500.5", 0), Err(FieldError::TooManyDecimals));
    }

    /// covers: AC-14
    ///
    /// The same amount written with more zeros is the same amount, not more
    /// precision the currency cannot write.
    #[test]
    fn trailing_zeros_are_not_extra_decimal_places() {
        assert_eq!(price("12.500", 2), Ok(decimal("12.5")));
        assert_eq!(price("1500.00", 0), Ok(decimal("1500")));
    }

    #[test]
    fn negative_zero_is_zero_and_is_stored_without_its_sign() {
        let zero = price("-0", 2).expect("negative zero is still zero");
        assert!(zero.is_zero());
        assert!(!zero.is_sign_negative());
    }

    /// A run of digits longer than a `Decimal` holds is too large, not
    /// unreadable.
    #[test]
    fn a_price_longer_than_any_decimal_is_too_large() {
        assert_eq!(price(&"9".repeat(40), 2), Err(FieldError::TooLarge));
    }

    /// covers: AC-14
    #[test]
    fn every_bad_field_on_a_dish_form_is_reported_at_once() {
        let refused = dish_fields("  ", Some(&"a".repeat(301)), "abc", 2)
            .expect_err("a form with three bad fields was accepted");

        assert_eq!(
            refused.pairs(),
            [
                ("name".to_owned(), FieldError::Required),
                ("description".to_owned(), FieldError::TooLong),
                ("price".to_owned(), FieldError::NotANumber),
            ]
        );
    }

    #[test]
    fn a_good_dish_form_comes_back_trimmed() {
        let fields =
            dish_fields(" Dal makhani ", Some(" "), "340.00", 2).expect("a good form was refused");

        assert_eq!(fields.name, "Dal makhani");
        assert_eq!(fields.description, None);
        assert_eq!(fields.price, decimal("340.00"));
    }

    /// covers: AC-5
    #[test]
    fn a_reorder_must_name_the_live_list_exactly() {
        let a = Uuid::now_v7();
        let b = Uuid::now_v7();
        let c = Uuid::now_v7();

        assert!(is_the_same_list(&[c, a, b], &[a, b, c]));
        assert!(is_the_same_list(&[], &[]));

        // One missing: a dish was added a moment ago.
        assert!(!is_the_same_list(&[b, a], &[a, b, c]));
        // One too many: a dish was removed a moment ago.
        assert!(!is_the_same_list(&[a, b, c], &[a, b]));
        // Right length, one repeated in place of a missing one.
        assert!(!is_the_same_list(&[a, a, b], &[a, b, c]));
    }
}
