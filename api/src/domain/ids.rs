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
