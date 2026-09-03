//! The one cookie this product sets, and every attribute on it.
//!
//! Written in one place because each attribute is a security control and a
//! missing one is silent: the cookie still works, the product still behaves,
//! and the hole is only visible to somebody reading the header.
//!
//! - `HttpOnly` so a cross site scripting bug cannot read the token.
//! - `SameSite=Lax` so a cross site form post does not carry it. The `Origin`
//!   check on mutating methods closes the top level navigation gap `Lax` leaves.
//! - `Path=/` because every surface uses it.
//! - No `Domain` at all, which keeps it a host only cookie no subdomain
//!   receives. Setting `Domain` to the apex would hand it to anything anybody
//!   ever hosts on a subdomain.
//! - `Secure` everywhere except development, read from the environment the
//!   config already validated, so local http works and production cannot ship
//!   an insecure cookie by leaving something out.

use axum_extra::extract::cookie::{Cookie, SameSite};

use crate::domain::session::SESSION_ABSOLUTE_LIFETIME;
use crate::infrastructure::config::Environment;

/// What the session cookie is called.
///
/// Deliberately dull. A name like `restaurant_admin_session` tells anybody
/// looking at a request what the product is and what the holder can do.
pub const SESSION_COOKIE: &str = "session";

/// Builds the cookie that carries a freshly issued session.
///
/// `Max-Age` is the session's absolute ceiling rather than its sliding window.
/// The cookie is written once at sign in and never rewritten on a slide, so a
/// `Max-Age` of the sliding window would delete it from the browser fourteen
/// days after sign in even though the server had been sliding the session all
/// along. The ceiling is exactly as long as the session can possibly live, which
/// leaves the server the only authority on when it actually ends.
#[must_use]
pub fn issue(value: String, environment: Environment) -> Cookie<'static> {
    let mut cookie = base(value, environment);

    // The conversion cannot fail for a constant measured in days, and the
    // fallback is a session cookie, which is the safe direction: the person is
    // signed out when the browser closes rather than kept signed in longer than
    // intended.
    if let Ok(max_age) = time::Duration::try_from(SESSION_ABSOLUTE_LIFETIME) {
        cookie.set_max_age(max_age);
    }

    cookie
}

/// Builds the cookie that clears the session on sign out.
///
/// Every attribute has to match the one that was set, or the browser treats it
/// as a different cookie and quietly keeps the old one. That is why this is
/// built from the same function rather than written out again.
#[must_use]
pub fn clear(environment: Environment) -> Cookie<'static> {
    let mut cookie = base(String::new(), environment);
    cookie.make_removal();
    cookie
}

/// Every attribute the two share, so neither can drift from the other.
fn base(value: String, environment: Environment) -> Cookie<'static> {
    Cookie::build((SESSION_COOKIE, value))
        .http_only(true)
        .same_site(SameSite::Lax)
        .path("/")
        // No `.domain(...)` call. Its absence is the control: a cookie with no
        // Domain attribute is host only, and nothing on a subdomain receives it.
        .secure(environment != Environment::Development)
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// covers: AC-3
    ///
    /// Every one of these is a security control whose absence is invisible in
    /// use. Checked one by one rather than against a whole rendered header, so
    /// a failure names which attribute went.
    #[test]
    fn the_session_cookie_carries_every_attribute_that_protects_it() {
        let cookie = issue("a-token".to_owned(), Environment::Production);

        assert_eq!(cookie.name(), SESSION_COOKIE);
        assert_eq!(cookie.http_only(), Some(true), "a script could read it");
        assert_eq!(
            cookie.same_site(),
            Some(SameSite::Lax),
            "a cross site form post would carry it"
        );
        assert_eq!(cookie.path(), Some("/"), "some surfaces would not send it");
        assert_eq!(cookie.secure(), Some(true), "it would travel over http");
        assert_eq!(
            cookie.domain(),
            None,
            "a Domain attribute would hand this cookie to every subdomain"
        );
    }

    /// covers: AC-3
    ///
    /// The one attribute that differs by environment, and the direction of the
    /// difference is what matters: local http development works, and production
    /// cannot end up insecure by anybody forgetting anything.
    #[test]
    fn the_cookie_is_secure_everywhere_except_development() {
        assert_eq!(
            issue("a-token".to_owned(), Environment::Production).secure(),
            Some(true)
        );
        assert_eq!(
            issue("a-token".to_owned(), Environment::Development).secure(),
            Some(false),
            "a secure cookie is never sent over plain http, so local development would \
             sign nobody in"
        );
    }

    /// A session that outlives its cookie signs somebody out for no reason
    /// anybody could explain. The cookie has to last as long as the session
    /// possibly can.
    #[test]
    fn the_cookie_lives_as_long_as_a_session_possibly_can() {
        let cookie = issue("a-token".to_owned(), Environment::Production);
        let max_age = cookie.max_age().expect("the cookie carries a Max-Age");

        assert_eq!(
            u64::try_from(max_age.whole_seconds()).expect("a positive age"),
            SESSION_ABSOLUTE_LIFETIME.as_secs(),
            "the cookie and the session's ceiling have drifted apart"
        );
    }

    /// covers: AC-13
    ///
    /// A removal cookie only removes anything if every attribute matches the
    /// one that was set. A mismatched path in particular leaves the original
    /// cookie in place and adds a second, empty one, and the person stays
    /// signed in with nothing on screen to say so.
    #[test]
    fn clearing_matches_the_cookie_it_is_clearing() {
        let issued = issue("a-token".to_owned(), Environment::Production);
        let cleared = clear(Environment::Production);

        assert_eq!(cleared.name(), issued.name());
        assert_eq!(cleared.path(), issued.path());
        assert_eq!(cleared.domain(), issued.domain());
        assert_eq!(
            cleared.value(),
            "",
            "a cleared cookie still carries a value"
        );
        assert!(
            cleared
                .max_age()
                .is_some_and(|age| age <= time::Duration::ZERO),
            "a removal cookie has to be already expired"
        );
    }
}
