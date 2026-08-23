//! Identifiers used across the domain.
//!
//! Every entity gets its own newtype rather than passing bare [`Uuid`] values
//! around. A visit id and a bill id are both 128 bits of nothing in particular,
//! so the compiler is the only thing that can stop one being handed to a
//! function expecting the other.
//!
//! All of them are version 7 UUIDs, generated here rather than by the database,
//! because Postgres 17 has no built in `uuidv7()`. Version 7 sorts by creation
//! time, so a primary key index keeps appending rather than scattering writes
//! across itself the way version 4 does.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Declares an identifier newtype with the same small surface on each one.
///
/// Written as a macro because fifteen hand copied versions of the same six
/// methods is fifteen chances to get one of them subtly different.
macro_rules! define_ids {
    ($(
        $(#[$doc:meta])*
        $name:ident
    ),* $(,)?) => {
        $(
            $(#[$doc])*
            #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
            #[serde(transparent)]
            pub struct $name(Uuid);

            impl $name {
                /// Mints a new identifier.
                ///
                /// Version 7, so it carries its creation time and sorts by it.
                #[must_use]
                pub fn new() -> Self {
                    Self(Uuid::now_v7())
                }

                /// Wraps a raw UUID that is already known to identify this kind
                /// of thing.
                #[must_use]
                pub const fn from_uuid(id: Uuid) -> Self {
                    Self(id)
                }

                /// The underlying UUID, for handing to the database or to a
                /// wire format.
                ///
                /// Takes `self` rather than `&self` because these are all
                /// [`Copy`], which lets them be used as a function value, as in
                /// `option.map(StaffId::as_uuid)`.
                #[must_use]
                pub const fn as_uuid(self) -> Uuid {
                    self.0
                }
            }

            impl Default for $name {
                fn default() -> Self {
                    Self::new()
                }
            }

            impl fmt::Display for $name {
                fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                    self.0.fmt(f)
                }
            }

            impl FromStr for $name {
                type Err = uuid::Error;

                fn from_str(value: &str) -> Result<Self, Self::Err> {
                    Uuid::from_str(value).map(Self)
                }
            }
        )*
    };
}

define_ids! {
    /// Which restaurant a piece of data belongs to.
    ///
    /// This is the single most load bearing value in the system. It scopes every
    /// query, every row level security policy, and every event stream. It is a
    /// newtype rather than a bare [`Uuid`] so it cannot be swapped by accident
    /// with some other identifier at a call site.
    RestaurantId,

    /// One tax the restaurant charges, such as a value added tax line.
    TaxComponentId,

    /// A member of staff: an admin, a waiter, or a chef.
    StaffId,

    /// One signed in session belonging to one member of staff.
    SessionId,

    /// A named group of tables, such as a terrace.
    TableSectionId,

    /// A physical table a party sits at.
    DiningTableId,

    /// A group of dishes on the menu.
    MenuCategoryId,

    /// One item on the menu.
    DishId,

    /// One party's stay at one table, from sitting down to leaving.
    VisitId,

    /// One ticket sent to the kitchen.
    OrderRoundId,

    /// One dish on one ticket.
    OrderLineId,

    /// A payment document drawn from a visit's lines.
    BillId,

    /// One tax line copied onto a bill when it closed.
    BillTaxId,

    /// A record that a closed bill was paid.
    PaymentId,

    /// One entry in the record of consequential changes.
    AuditEntryId,
}

#[cfg(test)]
mod tests {
    use std::thread::sleep;
    use std::time::Duration;

    use super::*;

    /// One representative is enough for most of what follows.
    ///
    /// All fifteen come out of the same macro, so they cannot differ from one
    /// another. [`RestaurantId`] is the one to test with, because it is the one
    /// every scope, every policy, and every event stream is keyed by.
    ///
    /// The part that cannot be tested here is the part the newtypes exist for:
    /// that a [`VisitId`] cannot be passed where a [`BillId`] belongs. That is
    /// refused by the compiler, so a test for it would not build.
    #[test]
    fn a_fresh_identifier_is_version_seven() {
        assert_eq!(
            RestaurantId::new().as_uuid().get_version_num(),
            7,
            "a fresh identifier is not version 7"
        );
        assert_eq!(
            VisitId::new().as_uuid().get_version_num(),
            7,
            "the macro does not mint every identifier the same way"
        );
    }

    /// The whole reason for version 7 over version 4.
    ///
    /// Version 7 puts the creation time in the leading bits, so identifiers
    /// minted in order sort in that order and a primary key index keeps
    /// appending. Version 4 would still be unique, still work, and scatter every
    /// write across the index instead: nothing would fail, it would just get
    /// slower and stay that way. This test is what notices.
    ///
    /// The waits are needed. Version 7 carries milliseconds, and identifiers
    /// minted inside one millisecond are ordered by random bits, so a run
    /// without them would pass or fail on timing.
    #[test]
    fn identifiers_minted_later_sort_after_earlier_ones() {
        let first = RestaurantId::new();
        sleep(Duration::from_millis(2));
        let second = RestaurantId::new();
        sleep(Duration::from_millis(2));
        let third = RestaurantId::new();

        assert!(first < second, "{first} was minted before {second}");
        assert!(second < third, "{second} was minted before {third}");
    }

    /// A nil identifier is the one value that must never come out of thin air.
    ///
    /// `Default` mints a real identifier today because it calls `new`. Deriving
    /// it instead would compile, read as tidier, and hand out
    /// `00000000-0000-0000-0000-000000000000`: a restaurant id matching no row,
    /// scoping a transaction to nothing, and doing it silently.
    #[test]
    fn the_default_identifier_is_a_fresh_one_rather_than_a_nil_one() {
        let id = RestaurantId::default();

        assert_ne!(id.as_uuid(), Uuid::nil(), "the default identifier is nil");
        assert_ne!(
            id,
            RestaurantId::default(),
            "two default identifiers are the same value"
        );
    }

    /// covers: AC-7
    ///
    /// `#[serde(transparent)]` is what makes an identifier travel as a plain
    /// uuid string. It matters most on the way in: the `NOTIFY` payload is built
    /// in SQL, where `restaurant_id` is a bare uuid, so an identifier that
    /// expected a wrapper around it would make every notification unreadable.
    #[test]
    fn an_identifier_travels_as_a_bare_uuid_string() {
        let raw = Uuid::from_str("018f3f4a-0000-7000-8000-000000000001")
            .expect("the test constant is a valid uuid");
        let id = RestaurantId::from_uuid(raw);

        assert_eq!(
            serde_json::to_value(id).expect("an identifier serialises"),
            serde_json::Value::String("018f3f4a-0000-7000-8000-000000000001".to_owned())
        );

        let read_back: RestaurantId =
            serde_json::from_str("\"018f3f4a-0000-7000-8000-000000000001\"")
                .expect("an identifier reads back from the bare string the database sends");

        assert_eq!(read_back, id);
    }

    /// Both ways out of an identifier lead back to the same value.
    #[test]
    fn an_identifier_round_trips_through_its_uuid_and_through_its_text() {
        let id = RestaurantId::new();

        assert_eq!(RestaurantId::from_uuid(id.as_uuid()), id);
        assert_eq!(
            RestaurantId::from_str(&id.to_string()).expect("an identifier parses its own text"),
            id
        );
        assert_eq!(id.to_string(), id.as_uuid().to_string());
    }

    #[test]
    fn text_that_is_not_a_uuid_is_refused() {
        assert!(RestaurantId::from_str("").is_err(), "empty text parsed");
        assert!(
            RestaurantId::from_str("018f3f4a-0000-7000-8000").is_err(),
            "a truncated uuid parsed"
        );
    }
}
