//! Money, and the one rounding rule the whole product uses.
//!
//! Every money value in this system is a [`Decimal`] backed by `numeric(14,4)`
//! in Postgres. There is no float anywhere near a price, a tax, or a total.
//!
//! Restaurants on this platform may be in different countries, so a bill is
//! rounded to its own restaurant's currency rather than to a fixed two places. A
//! yen bill rounds to zero decimals and a dinar bill to three, and both are
//! ordinary here.

use rust_decimal::{Decimal, RoundingStrategy};

use super::error::{DomainError, DomainResult};

/// The largest number of decimal places any currency on the platform uses.
///
/// Matches the `currency_decimals between 0 and 4` check on both `restaurants`
/// and `bills`.
pub const MAX_CURRENCY_DECIMALS: u32 = 4;

/// One hundred, as a [`Decimal`], for turning a percentage into a fraction.
const PERCENT: Decimal = Decimal::from_parts(100, 0, 0, false, 0);

/// What currency a restaurant charges in, and to how many decimal places.
///
/// Both halves travel together because neither is any use alone: rounding a
/// figure needs the decimals, and printing it needs the code. They are copied
/// onto a bill when it closes, so a bill keeps the currency it was charged in
/// even if the restaurant later changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Currency {
    code: String,
    decimals: u32,
}

impl Currency {
    /// Builds a currency from a three letter code and its decimal places.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::Invalid`] if the code is not three upper case
    /// letters, or if the decimals are outside the range the database accepts.
    /// Both are refused here as well as by a check constraint, so a bad value is
    /// caught before it reaches a statement rather than as a constraint
    /// violation the caller has to decode.
    pub fn new(code: &str, decimals: u32) -> DomainResult<Self> {
        if code.len() != 3 || !code.bytes().all(|b| b.is_ascii_uppercase()) {
            return Err(DomainError::Invalid(format!(
                "currency code must be three upper case letters, got {code:?}"
            )));
        }

        if decimals > MAX_CURRENCY_DECIMALS {
            return Err(DomainError::Invalid(format!(
                "a currency may have at most {MAX_CURRENCY_DECIMALS} decimal places, got {decimals}"
            )));
        }

        Ok(Self {
            code: code.to_owned(),
            decimals,
        })
    }

    /// The three letter code, such as `EUR`.
    #[must_use]
    pub fn code(&self) -> &str {
        &self.code
    }

    /// How many decimal places this currency is charged to.
    #[must_use]
    pub const fn decimals(&self) -> u32 {
        self.decimals
    }

    /// Rounds a figure to this currency, half away from zero.
    ///
    /// Half away from zero is stated explicitly and deliberately, because
    /// `rust_decimal` rounds half to even by default. Half to even is the right
    /// default for long chains of scientific arithmetic and the wrong one for a
    /// receipt: a customer looking at 0.125 expects 0.13, and a restaurant
    /// explaining "banker's rounding" to a customer is a conversation nobody
    /// wants to have.
    #[must_use]
    pub fn round(&self, amount: Decimal) -> Decimal {
        amount.round_dp_with_strategy(self.decimals, RoundingStrategy::MidpointAwayFromZero)
    }
}

/// Applies a percentage to a base figure, without rounding.
///
/// Left unrounded on purpose: the caller rounds once, at the end, to the bill's
/// own currency. Rounding here as well would round twice and drift.
#[must_use]
pub fn apply_percent(base: Decimal, percent: Decimal) -> Decimal {
    base * percent / PERCENT
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn decimal(value: &str) -> Decimal {
        Decimal::from_str(value).expect("test constant is a valid decimal")
    }

    /// The whole reason [`Currency::round`] names its strategy.
    ///
    /// `rust_decimal`'s default is half to even, which would send 0.125 to 0.12
    /// and 0.135 to 0.14: two adjacent figures rounding in opposite directions
    /// on the same receipt. Guards against someone simplifying this to
    /// `round_dp`.
    #[test]
    fn a_half_rounds_away_from_zero_rather_than_to_the_even_neighbour() {
        let euro = Currency::new("EUR", 2).expect("EUR is a valid currency");

        assert_eq!(euro.round(decimal("0.125")), decimal("0.13"));
        assert_eq!(euro.round(decimal("0.135")), decimal("0.14"));
        assert_eq!(euro.round(decimal("-0.125")), decimal("-0.13"));
    }

    /// Not every restaurant charges to two decimal places.
    #[test]
    fn each_currency_rounds_to_its_own_decimals() {
        let yen = Currency::new("JPY", 0).expect("JPY is a valid currency");
        let dinar = Currency::new("BHD", 3).expect("BHD is a valid currency");

        assert_eq!(yen.round(decimal("1234.5")), decimal("1235"));
        assert_eq!(dinar.round(decimal("1.23456")), decimal("1.235"));
    }

    #[test]
    fn a_percentage_is_applied_without_being_rounded() {
        // 12.5% of 99.99 is 12.49875, and it stays that way until a bill rounds
        // it once, together with everything else.
        assert_eq!(
            apply_percent(decimal("99.99"), decimal("12.5")),
            decimal("12.49875")
        );
    }

    #[test]
    fn a_malformed_currency_code_is_refused() {
        assert!(Currency::new("eur", 2).is_err(), "lower case was accepted");
        assert!(
            Currency::new("EURO", 2).is_err(),
            "four letters were accepted"
        );
        assert!(Currency::new("EU", 2).is_err(), "two letters were accepted");
    }

    #[test]
    fn more_decimals_than_the_column_holds_are_refused() {
        assert!(
            Currency::new("EUR", 5).is_err(),
            "five decimals were accepted"
        );
        assert!(
            Currency::new("EUR", 4).is_ok(),
            "four decimals were refused"
        );
    }
}
