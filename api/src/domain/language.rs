//! What languages exist, and the two values a caller is allowed to store.
//!
//! The list itself is not written here. It lives in `locales/catalogue.json` at
//! the repository root, which the web app imports directly and this module
//! compiles in with [`include_str!`]. One file, both sides, so the switcher can
//! never offer a language the API refuses and the API can never accept one the
//! switcher cannot load.
//!
//! That does couple this crate's build to a path outside `api/`. The Docker
//! build already runs from the repository root, so it works, and it is written
//! down in the spec's consequences as a thing to remember when that build
//! changes.

use std::sync::LazyLock;

use serde::Deserialize;

use super::error::{DomainError, DomainResult};

/// The catalogue as committed, compiled into the binary.
///
/// A file rather than configuration on purpose: adding a language is a pull
/// request that ships translation files with it, never an environment variable
/// somebody can set to a language that has no words behind it.
const CATALOGUE_JSON: &str = include_str!("../../../locales/catalogue.json");

/// The catalogue, parsed once, or the reason it could not be.
///
/// The error is kept rather than thrown away so the boot can report it. Reading
/// this is what turns a malformed catalogue into a refused start rather than a
/// puzzling `400` on the first request that happens to write a language.
static CATALOGUE: LazyLock<Result<Catalogue, String>> =
    LazyLock::new(|| serde_json::from_str(CATALOGUE_JSON).map_err(|error| error.to_string()));

/// Borrows the parsed catalogue.
///
/// Call it once at startup so a malformed catalogue fails the boot. Every later
/// call reads what that first one already parsed.
///
/// # Errors
///
/// Returns [`DomainError::Invalid`] if the committed catalogue is not valid
/// JSON of the expected shape.
pub fn catalogue() -> DomainResult<&'static Catalogue> {
    CATALOGUE.as_ref().map_err(|error| {
        DomainError::Invalid(format!("locales/catalogue.json is unreadable: {error}"))
    })
}

/// Which way a language is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    /// Left to right, such as English or Hindi.
    Ltr,
    /// Right to left, such as Arabic or Hebrew.
    Rtl,
}

/// One language the platform offers.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Language {
    /// The code stored in the database and used in a file path, such as `hi`.
    pub code: String,
    /// What to call it in English, for an English speaking admin.
    pub english_name: String,
    /// What to call it in itself, which is what the switcher shows. Somebody
    /// who cannot read the current language has to be able to find their own.
    pub native_name: String,
    /// Which way it is written, which becomes the `dir` attribute.
    pub direction: Direction,
}

/// What a fresh restaurant gets before anybody configures it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Defaults {
    /// The default interface language. Matches the column default in migration
    /// `0003`.
    pub language: String,
    /// The default formatting locale. Matches the column default in migration
    /// `0003`.
    pub formatting_locale: String,
}

/// The whole list of what languages exist, and how figures may be written.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Catalogue {
    /// Every language the interface can be read in.
    pub languages: Vec<Language>,
    /// Every locale a restaurant may format its money, numbers, and dates with.
    /// Deliberately a separate list: it is a superset shaped by where
    /// restaurants are, not by what their staff read.
    pub formatting_locales: Vec<String>,
    /// What an unconfigured restaurant falls back to.
    pub defaults: Defaults,
}

impl Catalogue {
    /// Finds a language by its code.
    #[must_use]
    pub fn language(&self, code: &str) -> Option<&Language> {
        self.languages.iter().find(|language| language.code == code)
    }
}

/// A language code that is in the catalogue.
///
/// The only constructor validates, so a value of this type is a language the
/// switcher offers and the loader has files for. A code that is not in the
/// catalogue is refused on write rather than stored and quietly ignored by
/// every later read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguageCode(String);

impl LanguageCode {
    /// Builds a language code, checking it against the catalogue.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::Invalid`] if the code is not in the catalogue, or
    /// if the catalogue itself cannot be read.
    pub fn new(code: &str) -> DomainResult<Self> {
        let catalogue = catalogue()?;

        if catalogue.language(code).is_none() {
            return Err(DomainError::Invalid(format!(
                "{code:?} is not a language this platform offers"
            )));
        }

        Ok(Self(code.to_owned()))
    }

    /// The code itself, such as `hi`.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A formatting locale that is in the catalogue.
///
/// Separate from [`LanguageCode`] because the two are separate values that are
/// never derived from each other. A restaurant reading English may well format
/// in `en-IN`, and the type system is where that stops being a comment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormattingLocale(String);

impl FormattingLocale {
    /// Builds a formatting locale, checking it against the catalogue.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::Invalid`] if the locale is not in the catalogue,
    /// or if the catalogue itself cannot be read.
    pub fn new(locale: &str) -> DomainResult<Self> {
        let catalogue = catalogue()?;

        if !catalogue
            .formatting_locales
            .iter()
            .any(|known| known == locale)
        {
            return Err(DomainError::Invalid(format!(
                "{locale:?} is not a formatting locale this platform offers"
            )));
        }

        Ok(Self(locale.to_owned()))
    }

    /// The locale itself, such as `en-IN`.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_committed_catalogue_parses() {
        let catalogue = catalogue().expect("the committed catalogue should parse");

        assert!(
            catalogue.language("en").is_some(),
            "english is the fallback every other language falls through to"
        );
        assert!(catalogue.language("hi").is_some());
    }

    #[test]
    fn the_defaults_are_themselves_in_the_catalogue() {
        let catalogue = catalogue().expect("the committed catalogue should parse");

        // A default outside the list would be a value every fresh restaurant
        // gets and every write then refuses.
        LanguageCode::new(&catalogue.defaults.language)
            .expect("the default language should be in the catalogue");
        FormattingLocale::new(&catalogue.defaults.formatting_locale)
            .expect("the default formatting locale should be in the catalogue");
    }

    #[test]
    fn the_defaults_match_the_column_defaults_in_migration_0003() {
        let catalogue = catalogue().expect("the committed catalogue should parse");

        // The migration cannot read this file, so the two are pinned here. A
        // restaurant created before anybody configures it has to land on the
        // same values the application thinks it landed on.
        assert_eq!(catalogue.defaults.language, "en");
        assert_eq!(catalogue.defaults.formatting_locale, "en-US");
    }

    #[test]
    fn a_language_outside_the_catalogue_is_refused() {
        let error = LanguageCode::new("xx").expect_err("an unknown code should be refused");
        assert!(matches!(error, DomainError::Invalid(_)));
    }

    #[test]
    fn a_formatting_locale_outside_the_catalogue_is_refused() {
        // A real locale, and still refused: the catalogue is the list, not
        // whatever `Intl` happens to accept.
        let error =
            FormattingLocale::new("fr-FR").expect_err("an unlisted locale should be refused");
        assert!(matches!(error, DomainError::Invalid(_)));
    }

    #[test]
    fn a_language_code_is_not_a_formatting_locale() {
        // The two lists are separate on purpose, and mixing them up is the
        // mistake this pair of types exists to make impossible.
        FormattingLocale::new("en").expect_err("a bare language is not a formatting locale");
        LanguageCode::new("en-US").expect_err("a formatting locale is not a language");
    }

    #[test]
    fn every_catalogue_language_carries_a_native_name() {
        let catalogue = catalogue().expect("the committed catalogue should parse");

        for language in &catalogue.languages {
            assert!(
                !language.native_name.trim().is_empty(),
                "{} has no native name, and somebody who cannot read the current language has to find their own",
                language.code
            );
        }
    }
}
