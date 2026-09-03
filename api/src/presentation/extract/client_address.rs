//! Where a request came from, as far as anything here can honestly tell.
//!
//! Two sources, one per environment, and the difference is not cosmetic.
//!
//! **Outside development** the address comes from the `CloudFront-Viewer-Address` header,
//! which `CloudFront` sets itself and overwrites if the client sent one. The
//! socket address is useless there: every request arrives from the load
//! balancer, so it would put the whole internet in one throttle bucket.
//!
//! **In development** there is no `CloudFront`, so the socket peer is the client
//! and is the honest answer.
//!
//! The header is only trustworthy because of an `infra/` change that has
//! nothing to do with this crate: the load balancer's security group accepts
//! traffic only from `CloudFront`'s managed prefix list. Without that, anybody who
//! found the load balancer's own address could set the header to whatever they
//! liked and the address bucket would count somebody else. That dependency is
//! written down here because nothing in this file can enforce it and nothing
//! here will notice if it goes.

use std::net::{IpAddr, SocketAddr};

use axum::extract::{ConnectInfo, FromRequestParts};
use axum::http::request::Parts;

use crate::infrastructure::config::Environment;
use crate::presentation::error::ApiError;
use crate::presentation::state::AppState;

/// The header `CloudFront` sets, and overwrites if a client sent one.
const VIEWER_ADDRESS: &str = "cloudfront-viewer-address";

/// Where this request came from, or [`None`] when nothing could say.
///
/// [`None`] is a real answer rather than a failure. A request with no usable
/// address is still served; it simply is not counted in the per address throttle
/// bucket, because counting it in a bucket shared with every other unattributable
/// request would let one of them lock out all the rest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClientAddress(Option<IpAddr>);

impl ClientAddress {
    /// The address, if there is one.
    #[must_use]
    pub const fn address(self) -> Option<IpAddr> {
        self.0
    }
}

impl FromRequestParts<AppState> for ClientAddress {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        if state.environment == Environment::Development {
            let peer = parts
                .extensions
                .get::<ConnectInfo<SocketAddr>>()
                .map(|ConnectInfo(socket)| socket.ip());

            return Ok(Self(peer));
        }

        let header = parts
            .headers
            .get(VIEWER_ADDRESS)
            .and_then(|value| value.to_str().ok());

        Ok(Self(header.and_then(parse_viewer_address)))
    }
}

/// The address out of what `CloudFront` wrote.
///
/// The header is `address:port`, and the address half may be IPv6, which itself
/// contains colons. So the split is on the last colon and not the first, and an
/// IPv6 address arrives without the brackets a URL would put round it.
fn parse_viewer_address(raw: &str) -> Option<IpAddr> {
    let trimmed = raw.trim();

    // Try the whole thing first: a bare address with no port is not the
    // documented shape, but accepting it costs nothing and refusing it would
    // mean a whole class of request quietly falling out of the bucket.
    if let Ok(address) = trimmed.parse::<IpAddr>() {
        return Some(address);
    }

    let (address, port) = trimmed.rsplit_once(':')?;

    // The port half has to look like one. Without this check the last group of
    // a bare IPv6 address is mistaken for a port and the rest is parsed as a
    // truncated address, which usually fails, but not always.
    if port.is_empty() || !port.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }

    address.trim_matches(['[', ']']).parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// covers: AC-11
    #[test]
    fn a_viewer_address_yields_the_address_without_its_port() {
        assert_eq!(
            parse_viewer_address("203.0.113.7:52384"),
            "203.0.113.7".parse::<IpAddr>().ok()
        );
    }

    /// covers: AC-11
    ///
    /// The case a first colon split gets wrong, and gets wrong silently: an
    /// IPv6 address is mostly colons, so splitting on the first one hands back
    /// `2001` and the throttle bucket becomes one bucket for the whole IPv6
    /// internet.
    #[test]
    fn an_ipv6_viewer_address_survives_having_colons_of_its_own() {
        assert_eq!(
            parse_viewer_address("2001:db8::1:52384"),
            "2001:db8::1".parse::<IpAddr>().ok()
        );
        assert_eq!(
            parse_viewer_address("[2001:db8::1]:52384"),
            "2001:db8::1".parse::<IpAddr>().ok()
        );
    }

    #[test]
    fn a_bare_address_with_no_port_is_still_read() {
        assert_eq!(
            parse_viewer_address("198.51.100.4"),
            "198.51.100.4".parse::<IpAddr>().ok()
        );
        assert_eq!(
            parse_viewer_address("2001:db8::1"),
            "2001:db8::1".parse::<IpAddr>().ok()
        );
    }

    /// Anything unreadable yields nothing rather than a guess. A wrong address
    /// in the bucket throttles somebody who did nothing.
    #[test]
    fn nonsense_yields_no_address_at_all() {
        for raw in [
            "",
            "   ",
            "not-an-address",
            "203.0.113.7:",
            "203.0.113.7:abc",
        ] {
            assert_eq!(
                parse_viewer_address(raw),
                None,
                "{raw:?} was read as an address"
            );
        }
    }
}
