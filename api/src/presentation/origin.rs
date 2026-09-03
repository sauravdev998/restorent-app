//! Refusing a mutating request that came from somewhere else.
//!
//! `SameSite=Lax` on the session cookie already stops a cross site form post
//! carrying it. What `Lax` deliberately does not stop is a top level
//! navigation, which is how a link somebody clicks in an email can arrive here
//! with the cookie attached. This closes that gap.
//!
//! There is nothing to configure. Same origin means the `Origin` header's host
//! equals the request's own `Host` header, which is true by definition and needs
//! no value kept in step across three environments. A configured allowed origin
//! is a thing that gets out of date, and the day it does, the product breaks in
//! a way that reads like a browser bug.
//!
//! Safe methods are not checked. A `GET` that changes something would be a
//! different bug, and checking `GET` would break every link into the app.

use axum::extract::Request;
use axum::http::{Method, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::domain::error::DomainError;
use crate::presentation::error::ApiError;

/// The header a browser sets to say the request is to the site it came from.
///
/// Accepted as equivalent proof where the browser sent it, because a browser
/// that sends this one is telling us the same thing more directly. Not every
/// browser does, which is why it is an alternative and not the check itself.
const SEC_FETCH_SITE: &str = "sec-fetch-site";

/// Refuses a mutating request that cannot prove it came from this site.
///
/// A request carrying neither header is refused rather than allowed. That is
/// the direction that matters: every browser this product supports sends at
/// least one of them on a mutating request, so what is left is something that is
/// not a browser, and something that is not a browser has no session cookie to
/// attach anyway.
pub async fn same_origin(request: Request, next: Next) -> Response {
    if !mutating(request.method()) {
        return next.run(request).await;
    }

    if proves_same_origin(&request) {
        return next.run(request).await;
    }

    tracing::warn!(
        method = %request.method(),
        path = request.uri().path(),
        "refusing a mutating request that could not prove it came from this site"
    );

    ApiError::from(DomainError::Forbidden).into_response()
}

/// Whether this method may change anything.
fn mutating(method: &Method) -> bool {
    matches!(
        *method,
        Method::POST | Method::PATCH | Method::PUT | Method::DELETE
    )
}

/// Whether the request carries proof it came from this site.
fn proves_same_origin(request: &Request) -> bool {
    let headers = request.headers();

    if headers
        .get(SEC_FETCH_SITE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.eq_ignore_ascii_case("same-origin"))
    {
        return true;
    }

    let Some(origin) = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) else {
        return false;
    };
    let Some(host) = headers.get(header::HOST).and_then(|v| v.to_str().ok()) else {
        return false;
    };

    host_of(origin).is_some_and(|from| from == host.trim())
}

/// The `host:port` out of an origin like `https://example.com:8443`.
///
/// Written by hand rather than through a URL parser, because what is wanted is
/// exactly the part that `Host` carries: the authority with no scheme, no path,
/// and no trailing slash. A parser would also happily accept things that are not
/// origins.
fn host_of(origin: &str) -> Option<&str> {
    let trimmed = origin.trim();
    let after_scheme = trimmed
        .split_once("://")
        .map_or(trimmed, |(_scheme, rest)| rest);

    // `null` is what a browser sends from a sandboxed frame or a `file://` page.
    // It is not this site.
    if after_scheme.is_empty() || after_scheme == "null" {
        return None;
    }

    Some(after_scheme.split('/').next().unwrap_or(after_scheme))
}

#[cfg(test)]
mod tests {
    use super::*;

    use axum::body::Body;
    use axum::http::HeaderValue;

    fn request(method: Method, headers: &[(&str, &str)]) -> Request {
        let mut builder = Request::builder().method(method).uri("/api/me");

        for (name, value) in headers {
            builder = builder.header(*name, HeaderValue::from_str(value).expect("a header"));
        }

        builder.body(Body::empty()).expect("a request")
    }

    /// covers: AC-12
    #[test]
    fn a_request_from_this_very_site_is_allowed() {
        assert!(proves_same_origin(&request(
            Method::POST,
            &[
                ("origin", "https://app.example.com"),
                ("host", "app.example.com")
            ],
        )));
    }

    /// covers: AC-12
    ///
    /// The whole reason this exists. `SameSite=Lax` lets a top level navigation
    /// carry the cookie, so without this a link in an email could post.
    #[test]
    fn a_request_from_another_site_is_refused() {
        assert!(!proves_same_origin(&request(
            Method::POST,
            &[
                ("origin", "https://elsewhere.example"),
                ("host", "app.example.com")
            ],
        )));

        // A prefix match would let `app.example.com.elsewhere.example` through,
        // which is a domain anybody can register.
        assert!(!proves_same_origin(&request(
            Method::POST,
            &[
                ("origin", "https://app.example.com.elsewhere.example"),
                ("host", "app.example.com"),
            ],
        )));
    }

    /// covers: AC-12
    ///
    /// The port is part of the origin. Two services on one host are two
    /// origins, and treating them as one is exactly the mistake that makes a
    /// development machine a way in.
    #[test]
    fn a_different_port_on_the_same_host_is_another_origin() {
        assert!(!proves_same_origin(&request(
            Method::POST,
            &[
                ("origin", "http://localhost:3000"),
                ("host", "localhost:5173")
            ],
        )));

        assert!(proves_same_origin(&request(
            Method::POST,
            &[
                ("origin", "http://localhost:5173"),
                ("host", "localhost:5173")
            ],
        )));
    }

    /// covers: AC-12
    #[test]
    fn a_request_carrying_neither_header_is_refused() {
        assert!(!proves_same_origin(&request(Method::POST, &[])));
        assert!(!proves_same_origin(&request(
            Method::POST,
            &[("host", "app.example.com")],
        )));
    }

    /// covers: AC-12
    #[test]
    fn a_browser_that_says_same_origin_directly_is_believed() {
        assert!(proves_same_origin(&request(
            Method::POST,
            &[("sec-fetch-site", "same-origin")],
        )));

        for value in ["cross-site", "same-site", "none"] {
            assert!(
                !proves_same_origin(&request(Method::POST, &[("sec-fetch-site", value)])),
                "{value:?} was accepted as proof of same origin"
            );
        }
    }

    /// covers: AC-12
    ///
    /// A sandboxed frame and a `file://` page both send `Origin: null`, and
    /// `null` is not this site however the comparison is written.
    #[test]
    fn an_opaque_origin_is_not_this_site() {
        assert_eq!(host_of("null"), None);
        assert!(!proves_same_origin(&request(
            Method::POST,
            &[("origin", "null"), ("host", "app.example.com")],
        )));
    }

    /// covers: AC-12
    ///
    /// Checking safe methods would refuse every link into the app, because a
    /// top level navigation carries no `Origin` at all.
    #[test]
    fn safe_methods_are_not_checked() {
        assert!(!mutating(&Method::GET));
        assert!(!mutating(&Method::HEAD));
        assert!(!mutating(&Method::OPTIONS));

        assert!(mutating(&Method::POST));
        assert!(mutating(&Method::PATCH));
        assert!(mutating(&Method::PUT));
        assert!(mutating(&Method::DELETE));
    }
}
