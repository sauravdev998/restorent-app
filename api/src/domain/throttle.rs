//! How many times somebody may try to sign in, and over what window.
//!
//! Two buckets, both counted in the database rather than in a process. That is
//! the load bearing part: there are several containers behind one load balancer,
//! and an in process limiter would grant every caller the full allowance on each
//! of them, so the real ceiling would quietly be the limit times the instance
//! count. Spec 0001 named a crate for this; it holds its state in the process,
//! which is why it is not used.
//!
//! Nothing here locks permanently and nothing needs an admin to unlock it. Both
//! buckets are time windows that clear themselves, so the worst an attacker can
//! do to somebody else's account is make them wait a quarter of an hour.
//!
//! An attempt is recorded before the password is checked, because a flood that
//! each cost a full `argon2` verification before being refused is the thing a
//! throttle exists to stop. What makes the count a count of *failures* is the
//! other end: proving you hold the account empties the bucket for that address,
//! so a row only survives an attempt that did not work. Without that half, five
//! ordinary sign ins on five devices would lock somebody out of their own
//! restaurant.

use std::time::Duration;

/// How far back both buckets count.
#[allow(clippy::duration_suboptimal_units)]
pub const THROTTLE_WINDOW: Duration = Duration::from_secs(15 * 60);

/// How many failed attempts one email address gets inside the window.
///
/// Small, because it is per account and a person who has typed their own
/// password wrong five times in a quarter of an hour is not going to get it on
/// the sixth. It is also the number an attacker guessing one account's password
/// runs into, which is the point.
///
/// Failed is the word that matters. A sign in or a registration that works
/// clears every row for that address, so this is five wrong guesses in a row,
/// not five uses of the product.
pub const MAX_ATTEMPTS_PER_EMAIL: i64 = 5;

/// How many attempts one client address gets inside the window.
///
/// Far more generous, because a whole restaurant can sit behind one address:
/// the house phone, the kitchen screen, and the owner's laptop all share it, and
/// a shift change is a burst of sign ins from one place. This is here to stop
/// somebody working through a list of addresses at speed, not to police a busy
/// evening.
pub const MAX_ATTEMPTS_PER_IP: i64 = 100;

// The two limits only mean anything in relation to each other, and a runtime
// assertion on two constants proves nothing at run time. These are compile time
// facts, so a bad edit is a failed build rather than a failed test.
const _: () = assert!(
    MAX_ATTEMPTS_PER_IP > MAX_ATTEMPTS_PER_EMAIL,
    "the address bucket is no more generous than the account one, so one restaurant sharing \
     an address would lock its own staff out at a shift change"
);
const _: () = assert!(
    MAX_ATTEMPTS_PER_EMAIL >= 3,
    "fewer than three attempts locks people out for ordinary typing"
);

#[cfg(test)]
mod tests {
    use super::*;

    /// A window nobody waits out is a lockout, and one nobody notices is not a
    /// throttle.
    #[test]
    fn the_window_is_long_enough_to_matter_and_short_enough_to_wait_out() {
        assert!(
            THROTTLE_WINDOW.as_secs() >= 60,
            "too short to be a throttle"
        );
        assert!(
            THROTTLE_WINDOW.as_secs() <= 60 * 60,
            "too long to be a wait rather than a lockout"
        );
    }
}
