//! The two values somebody types to prove who they are.
//!
//! Both are newtypes that validate on construction, following the same pattern
//! as [`LanguageCode`](super::language::LanguageCode). A value of either type
//! has already passed its rule, so no handler, use case, or repository ever
//! re checks it, and no path can forget to.
//!
//! The password rule is the interesting one, because it decides what gets
//! hashed. Whitespace is trimmed first and the trimmed value is what is hashed
//! and what every later verification runs against, so registration, sign in,
//! and a password change can never disagree about what the person typed.

use super::error::{DomainError, DomainResult};

/// The shortest password accepted, in characters.
///
/// Length is the only rule. Composition rules (a digit, a symbol, a capital)
/// push people towards `Password1!` and towards writing it on the till, and
/// every current guideline has dropped them.
pub const MINIMUM_PASSWORD_CHARACTERS: usize = 10;

/// The longest password accepted, in bytes.
///
/// Not a preference: `argon2` is fine with any length, but this is the bcrypt
/// era limit that the rest of the industry standardised on, and pinning it here
/// means a password that is accepted today cannot become one that is refused by
/// a future hash. Bytes rather than characters, because that is what a hash
/// function counts.
pub const MAXIMUM_PASSWORD_BYTES: usize = 72;

/// The longest email address accepted, in bytes.
///
/// The practical ceiling every mail system enforces. The column itself is
/// `text`, so this exists to refuse an address that is obviously not one before
/// it is hashed against, indexed, and stored.
pub const MAXIMUM_EMAIL_BYTES: usize = 254;

/// An address somebody signs in with.
///
/// Stored exactly as it was typed, and matched case insensitively. Both halves
/// matter: the unique index and every lookup lower it, so `Ada@Example.com` and
/// `ada@example.com` are one account, while the identity bundle hands the
/// person back their own capitalisation rather than a flattened version of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmailAddress(String);

impl EmailAddress {
    /// Builds an email address, checking the little that is worth checking.
    ///
    /// Deliberately not a pattern that tries to express RFC 5322. The only
    /// authority on whether an address exists is sending mail to it, which this
    /// product does not do, so the check refuses what is plainly not an address
    /// and accepts the rest rather than turning away somebody's real address on
    /// a technicality.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::Invalid`] if the address is empty, too long, or
    /// not shaped like an address at all.
    pub fn new(raw: &str) -> DomainResult<Self> {
        let trimmed = raw.trim();

        if trimmed.is_empty() {
            return Err(DomainError::Invalid(
                "an email address is required".to_owned(),
            ));
        }

        if trimmed.len() > MAXIMUM_EMAIL_BYTES {
            return Err(DomainError::Invalid(format!(
                "an email address may not be longer than {MAXIMUM_EMAIL_BYTES} bytes"
            )));
        }

        let (local, domain) = trimmed
            .split_once('@')
            .ok_or_else(|| DomainError::Invalid("an email address needs an @".to_owned()))?;

        if local.is_empty() || domain.is_empty() {
            return Err(DomainError::Invalid(
                "an email address needs something either side of the @".to_owned(),
            ));
        }

        if domain.contains('@') || !domain.contains('.') || trimmed.contains(char::is_whitespace) {
            return Err(DomainError::Invalid(
                "that does not look like an email address".to_owned(),
            ));
        }

        Ok(Self(trimmed.to_owned()))
    }

    /// The address exactly as the person typed it, which is what is stored and
    /// what is handed back to them.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The lowered form, which is what every lookup and every throttle bucket
    /// keys on.
    #[must_use]
    pub fn lowered(&self) -> String {
        self.0.to_lowercase()
    }
}

/// A password somebody typed, already known to satisfy the rules.
///
/// It carries the trimmed value, and the trimmed value is the one hashed. A
/// password pasted from a manager with a trailing newline is therefore the same
/// password on the day it is set and on every day it is used.
///
/// The inner value is never printed. [`Debug`] is written by hand for exactly
/// that reason: a derived one would put a password into the first log line that
/// formatted a request struct.
#[derive(Clone, PartialEq, Eq)]
pub struct Password(String);

impl std::fmt::Debug for Password {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Password(<redacted>)")
    }
}

impl Password {
    /// Builds a password, trimming it and checking its length.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::Invalid`] if the trimmed password is shorter than
    /// [`MINIMUM_PASSWORD_CHARACTERS`] characters or longer than
    /// [`MAXIMUM_PASSWORD_BYTES`] bytes.
    pub fn new(raw: &str) -> DomainResult<Self> {
        let trimmed = raw.trim();

        if trimmed.chars().count() < MINIMUM_PASSWORD_CHARACTERS {
            return Err(DomainError::Invalid(format!(
                "a password must be at least {MINIMUM_PASSWORD_CHARACTERS} characters"
            )));
        }

        if trimmed.len() > MAXIMUM_PASSWORD_BYTES {
            return Err(DomainError::Invalid(format!(
                "a password may not be longer than {MAXIMUM_PASSWORD_BYTES} bytes"
            )));
        }

        Ok(Self(trimmed.to_owned()))
    }

    /// The trimmed value, for hashing or verifying against a stored hash.
    ///
    /// The only way out, and it is named so that a call site that leaks one is
    /// visible in a review.
    #[must_use]
    pub fn exposed(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// covers: AC-1
    #[test]
    fn an_address_keeps_the_capitalisation_it_was_typed_with() {
        let email = EmailAddress::new("Ada@Example.Com").expect("a normal address");

        assert_eq!(email.as_str(), "Ada@Example.Com");
        assert_eq!(email.lowered(), "ada@example.com");
    }

    #[test]
    fn an_address_is_trimmed_but_not_otherwise_touched() {
        let email = EmailAddress::new("  ada@example.com  ").expect("a padded address");
        assert_eq!(email.as_str(), "ada@example.com");
    }

    #[test]
    fn something_that_is_not_an_address_is_refused() {
        for raw in [
            "",
            "   ",
            "ada",
            "@example.com",
            "ada@",
            "ada@example",
            "ada@@example.com",
            "ada example@example.com",
        ] {
            EmailAddress::new(raw)
                .expect_err(&format!("{raw:?} should not be accepted as an address"));
        }
    }

    #[test]
    fn an_absurdly_long_address_is_refused() {
        let long = format!("{}@example.com", "a".repeat(MAXIMUM_EMAIL_BYTES));
        EmailAddress::new(&long).expect_err("an address past the ceiling should be refused");
    }

    /// covers: AC-5
    #[test]
    fn a_password_shorter_than_the_floor_is_refused() {
        let error = Password::new("short").expect_err("nine characters or fewer is refused");
        assert!(matches!(error, DomainError::Invalid(_)));

        Password::new(&"a".repeat(MINIMUM_PASSWORD_CHARACTERS - 1))
            .expect_err("one character under the floor is refused");
        Password::new(&"a".repeat(MINIMUM_PASSWORD_CHARACTERS))
            .expect("exactly the floor is accepted");
    }

    /// covers: AC-5
    #[test]
    fn a_password_past_the_byte_ceiling_is_refused() {
        Password::new(&"a".repeat(MAXIMUM_PASSWORD_BYTES))
            .expect("exactly the ceiling is accepted");
        Password::new(&"a".repeat(MAXIMUM_PASSWORD_BYTES + 1))
            .expect_err("one byte over the ceiling is refused");
    }

    /// covers: AC-5
    ///
    /// The ceiling counts bytes and the floor counts characters, and the two
    /// genuinely differ. A passphrase of emoji is ten characters and forty
    /// bytes; one of thirty Devanagari characters is well past seventy two
    /// bytes. Counting the wrong unit at either end silently changes who can
    /// sign up.
    #[test]
    fn the_floor_counts_characters_and_the_ceiling_counts_bytes() {
        // Ten characters, four bytes each. Long enough by the floor.
        let emoji = "🍜".repeat(10);
        assert_eq!(emoji.chars().count(), 10);
        assert_eq!(emoji.len(), 40);
        Password::new(&emoji).expect("ten characters is ten characters whatever they weigh");

        // Twenty five Devanagari characters, three bytes each, so seventy five
        // bytes: comfortably past the ceiling while looking short.
        let devanagari = "अ".repeat(25);
        assert!(devanagari.len() > MAXIMUM_PASSWORD_BYTES);
        Password::new(&devanagari).expect_err("past the byte ceiling, however few characters");
    }

    /// covers: AC-5
    ///
    /// The load bearing half of the rule. What is hashed is the trimmed value,
    /// so a password pasted with a trailing newline on the day it is set still
    /// verifies on the day it is used.
    #[test]
    fn the_trimmed_value_is_the_one_that_gets_hashed() {
        let padded = Password::new("  correct horse  ").expect("a padded password");
        let plain = Password::new("correct horse").expect("the same password, unpadded");

        assert_eq!(padded.exposed(), "correct horse");
        assert_eq!(padded, plain);
    }

    /// A password that is only whitespace is refused rather than trimmed to
    /// nothing and hashed as an empty string.
    #[test]
    fn a_password_of_nothing_but_spaces_is_refused() {
        Password::new("            ").expect_err("whitespace is not a password");
    }

    /// covers: AC-5
    ///
    /// The one thing that must never be printed. A derived `Debug` would put
    /// the password into the first `tracing` line that formatted a request
    /// struct, and nothing would fail.
    #[test]
    fn a_password_never_prints_itself() {
        let password = Password::new("correct horse battery").expect("a normal password");
        let printed = format!("{password:?}");

        assert!(
            !printed.contains("correct horse"),
            "the password appeared in {printed:?}"
        );
        assert_eq!(printed, "Password(<redacted>)");
    }
}
