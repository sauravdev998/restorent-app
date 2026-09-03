//! Which countries a restaurant may register in, and what it starts with.
//!
//! The list is not written here. It lives in `locales/countries.json` at the
//! repository root, which the web app imports directly for the registration
//! form and this module compiles in with [`include_str!`], exactly the way
//! [`super::language`] treats the language catalogue. One file, both sides, so
//! the form can never offer a country registration would refuse and
//! registration can never accept one the form does not know how to show.
//!
//! Registration is the only caller. It reads the row for the country the owner
//! picked and writes its five settings onto the new restaurant, so those five
//! values have exactly one source and nothing anywhere derives them a second
//! way.

use std::sync::LazyLock;

use serde::Deserialize;

use super::error::{DomainError, DomainResult};

/// The country list as committed, compiled into the binary.
///
/// A file rather than configuration, for the same reason the language
/// catalogue is: adding a country is a pull request that carries its currency,
/// its timezone, and a formatting locale the app can actually format with,
/// never an environment variable somebody sets to a row that does not exist.
const COUNTRIES_JSON: &str = include_str!("../../../locales/countries.json");

/// The list, parsed once, or the reason it could not be.
///
/// The error is kept rather than thrown away so the boot can report it, which
/// is what turns a malformed file into a refused start rather than a puzzling
/// `400` on the first registration.
static COUNTRIES: LazyLock<Result<CountryList, String>> =
    LazyLock::new(|| serde_json::from_str(COUNTRIES_JSON).map_err(|error| error.to_string()));

/// Borrows the parsed country list.
///
/// Call it once at startup so a malformed file fails the boot. Every later call
/// reads what that first one already parsed.
///
/// # Errors
///
/// Returns [`DomainError::Invalid`] if the committed file is not valid JSON of
/// the expected shape.
pub fn countries() -> DomainResult<&'static CountryList> {
    COUNTRIES.as_ref().map_err(|error| {
        DomainError::Invalid(format!("locales/countries.json is unreadable: {error}"))
    })
}

/// One country, and the settings a restaurant registering there starts with.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Country {
    /// The ISO 3166-1 alpha-2 code, upper case, such as `IN`.
    pub code: String,
    /// What to call it on the registration form.
    pub english_name: String,
    /// The ISO 4217 code the restaurant charges in, such as `INR`.
    pub currency_code: String,
    /// How many decimal places that currency uses.
    pub currency_decimals: u32,
    /// The country's primary IANA timezone. An owner in a country with several
    /// changes it afterwards in settings; registration is one short form, not a
    /// timezone picker.
    pub default_timezone: String,
    /// What the restaurant reads in until somebody changes it. A code from
    /// `locales/catalogue.json`.
    pub default_language: String,
    /// How the restaurant writes money, numbers, and dates. A code from
    /// `locales/catalogue.json`, and deliberately not derived from
    /// [`Self::default_language`].
    pub formatting_locale: String,
}

/// Every country the platform accepts a registration from.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CountryList {
    /// The list itself.
    pub countries: Vec<Country>,
}

impl CountryList {
    /// Finds a country by its upper case code.
    #[must_use]
    pub fn country(&self, code: &str) -> Option<&Country> {
        self.countries.iter().find(|country| country.code == code)
    }
}

/// A country code that is in the committed list.
///
/// The only constructor validates, so a value of this type always names a row
/// registration can read its five settings out of. It upper cases first, so a
/// form or a script sending `us` is accepted and `US` is what gets stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CountryCode(String);

impl CountryCode {
    /// Builds a country code, upper casing it and checking it against the list.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::Invalid`] if the code is not a country the
    /// platform serves, or if the committed list itself cannot be read.
    pub fn new(code: &str) -> DomainResult<Self> {
        let normalised = code.trim().to_ascii_uppercase();
        let countries = countries()?;

        if countries.country(&normalised).is_none() {
            return Err(DomainError::Invalid(format!(
                "{code:?} is not a country this platform serves"
            )));
        }

        Ok(Self(normalised))
    }

    /// The code itself, upper case, such as `IN`.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The settings a restaurant registering here starts with.
    ///
    /// Infallible in practice: the constructor already found this row. It still
    /// returns a result rather than panicking, because the alternative is an
    /// `expect` in a path that runs on every registration.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::Invalid`] if the committed list cannot be read.
    pub fn settings(&self) -> DomainResult<&'static Country> {
        countries()?.country(&self.0).ok_or_else(|| {
            DomainError::Invalid(format!("{} is no longer a country in the list", self.0))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::language::{FormattingLocale, LanguageCode};
    use super::*;

    #[test]
    fn the_committed_list_parses_and_is_not_empty() {
        let list = countries().expect("the committed country list should parse");

        assert!(
            !list.countries.is_empty(),
            "a country list with no countries in it means nobody can register"
        );
    }

    /// covers: AC-1
    ///
    /// The load bearing relationship between the two shared files. A country
    /// whose formatting locale is not in the catalogue would hand a brand new
    /// restaurant a locale every later write refuses and the browser cannot
    /// format with, and nothing would say so until somebody registered there.
    #[test]
    fn every_country_names_a_language_and_a_locale_the_catalogue_offers() {
        let list = countries().expect("the committed country list should parse");

        for country in &list.countries {
            LanguageCode::new(&country.default_language).unwrap_or_else(|_| {
                panic!(
                    "{} defaults to the language {:?}, which is not in locales/catalogue.json",
                    country.code, country.default_language
                )
            });
            FormattingLocale::new(&country.formatting_locale).unwrap_or_else(|_| {
                panic!(
                    "{} formats with {:?}, which is not in locales/catalogue.json",
                    country.code, country.formatting_locale
                )
            });
        }
    }

    /// The currency code goes into `restaurants.currency_code`, which carries a
    /// `~ '^[A-Z]{3}$'` check, and the decimals into a column checked to be
    /// between 0 and 4. A row that breaks either would fail at the insert,
    /// halfway through a registration, rather than here.
    #[test]
    fn every_country_carries_a_currency_the_schema_will_accept() {
        let list = countries().expect("the committed country list should parse");

        for country in &list.countries {
            assert!(
                country.currency_code.len() == 3
                    && country
                        .currency_code
                        .chars()
                        .all(|character| character.is_ascii_uppercase()),
                "{} charges in {:?}, which is not three upper case letters",
                country.code,
                country.currency_code
            );
            assert!(
                country.currency_decimals <= 4,
                "{} uses {} decimal places, which the schema refuses",
                country.code,
                country.currency_decimals
            );
        }
    }

    #[test]
    fn every_code_is_two_upper_case_letters_and_appears_once() {
        let list = countries().expect("the committed country list should parse");

        for country in &list.countries {
            assert!(
                country.code.len() == 2
                    && country
                        .code
                        .chars()
                        .all(|character| character.is_ascii_uppercase()),
                "{:?} is not a two letter upper case country code",
                country.code
            );

            let clashes = list
                .countries
                .iter()
                .filter(|other| other.code == country.code)
                .count();
            assert_eq!(clashes, 1, "{} appears more than once", country.code);
        }
    }

    #[test]
    fn every_country_names_a_timezone_that_looks_like_an_iana_zone() {
        let list = countries().expect("the committed country list should parse");

        for country in &list.countries {
            // The real check is Postgres, which is asked on write. This catches
            // the shape, which is what a hand edit to the file gets wrong.
            assert!(
                country.default_timezone.contains('/'),
                "{} names the timezone {:?}, which is not an IANA zone name",
                country.code,
                country.default_timezone
            );
        }
    }

    /// covers: AC-1
    #[test]
    fn a_country_code_is_accepted_in_any_case_and_stored_upper_case() {
        let lower = CountryCode::new("in").expect("a lower case code should be accepted");
        let upper = CountryCode::new("IN").expect("an upper case code should be accepted");
        let padded = CountryCode::new(" In ").expect("a padded code should be accepted");

        assert_eq!(lower.as_str(), "IN");
        assert_eq!(upper.as_str(), "IN");
        assert_eq!(padded.as_str(), "IN");
    }

    #[test]
    fn a_country_outside_the_list_is_refused() {
        let error = CountryCode::new("ZZ").expect_err("an unserved country should be refused");
        assert!(matches!(error, DomainError::Invalid(_)));

        // A real country the platform does not serve yet is refused the same
        // way, which is the case that actually happens.
        CountryCode::new("FR").expect_err("a country with no row should be refused");
    }

    /// covers: AC-1
    ///
    /// The five settings a new restaurant gets come from this row and from
    /// nowhere else. Reading them back through the code is what registration
    /// does, so it is what is checked.
    #[test]
    fn a_validated_code_leads_back_to_the_row_registration_reads() {
        let code = CountryCode::new("IN").expect("IN is in the list");
        let settings = code.settings().expect("a validated code has a row");

        assert_eq!(settings.code, "IN");
        assert_eq!(settings.currency_code, "INR");
        assert_eq!(settings.currency_decimals, 2);
        assert_eq!(settings.default_timezone, "Asia/Kolkata");
    }

    #[test]
    fn a_country_missing_a_field_is_refused() {
        // The web app parses the same file. A field optional on one side and
        // required on the other is how the two lists drift apart.
        let missing_currency = r#"{
            "countries": [
                { "code": "IN", "englishName": "India", "currencyDecimals": 2,
                  "defaultTimezone": "Asia/Kolkata", "defaultLanguage": "en",
                  "formattingLocale": "en-IN" }
            ]
        }"#;

        serde_json::from_str::<CountryList>(missing_currency)
            .expect_err("a country with no currency should be refused");
    }

    #[test]
    fn a_country_added_to_the_file_needs_no_code_change_here() {
        let two = r#"{
            "countries": [
                { "code": "IN", "englishName": "India", "currencyCode": "INR",
                  "currencyDecimals": 2, "defaultTimezone": "Asia/Kolkata",
                  "defaultLanguage": "en", "formattingLocale": "en-IN" },
                { "code": "JP", "englishName": "Japan", "currencyCode": "JPY",
                  "currencyDecimals": 0, "defaultTimezone": "Asia/Tokyo",
                  "defaultLanguage": "en", "formattingLocale": "en-US" }
            ]
        }"#;

        let list: CountryList = serde_json::from_str(two).expect("a second country should parse");
        let japan = list.country("JP").expect("the added country is found");

        assert_eq!(japan.currency_decimals, 0, "a zero decimal currency parses");
        assert_eq!(list.countries.len(), 2);
    }
}
