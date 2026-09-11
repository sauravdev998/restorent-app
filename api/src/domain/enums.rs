//! The seven closed vocabularies the schema is built on.
//!
//! Each one mirrors a Postgres enum type of the same name, value for value. The
//! two must stay in step: adding a value means a one line `ALTER TYPE` in a
//! migration and a variant here, in the same change.
//!
//! These types are deliberately free of any database machinery. The layer rule
//! is that no `sqlx` type appears in `domain/`, so the mapping onto the Postgres
//! enums lives in `infrastructure::db::pg_enum`, which wires `SQLx` up to the
//! [`Self::as_label`] and [`Self::from_label`] pairs generated below.

use serde::{Deserialize, Serialize};

/// Declares an enum together with the exact strings Postgres stores for it.
///
/// Keeping the labels beside the variants is what makes the mapping checkable by
/// eye: a typo is visible here rather than surfacing as a decode failure at
/// three in the morning.
macro_rules! labelled_enum {
    (
        $(#[$enum_doc:meta])*
        $name:ident {
            $(
                $(#[$variant_doc:meta])*
                $variant:ident => $label:literal
            ),* $(,)?
        }
    ) => {
        $(#[$enum_doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum $name {
            $(
                $(#[$variant_doc])*
                $variant
            ),*
        }

        impl $name {
            /// The exact string Postgres stores for this value.
            #[must_use]
            pub const fn as_label(self) -> &'static str {
                match self {
                    $( Self::$variant => $label ),*
                }
            }

            /// Reads a value back from the string Postgres stored.
            ///
            /// Returns [`None`] for anything else, so an enum value added to the
            /// database without being added here fails loudly at the row that
            /// carries it rather than being quietly treated as some other value.
            #[must_use]
            pub fn from_label(label: &str) -> Option<Self> {
                match label {
                    $( $label => Some(Self::$variant), )*
                    _ => None,
                }
            }
        }
    };
}

labelled_enum! {
    /// What a member of staff is allowed to be.
    ///
    /// Which role may perform which action is feature 7's decision and lives
    /// above this layer. This schema carries the role and nothing more.
    StaffRole {
        /// Runs the restaurant: menu, staff, tables, settings, reports.
        Admin => "admin",
        /// Takes orders at the table and closes bills.
        Waiter => "waiter",
        /// Works the kitchen screen and marks dishes done.
        Chef => "chef",
    }
}

labelled_enum! {
    /// Whether a party is still at the table.
    VisitStatus {
        /// The party is seated. At most one open visit per table, ever.
        Open => "open",
        /// The party has left and the table is free again.
        Closed => "closed",
    }
}

labelled_enum! {
    /// Where a whole ticket has got to.
    ///
    /// Never set directly: it is a total function of the ticket's lines, given by
    /// [`super::service::round_status_from_lines`].
    RoundStatus {
        /// At least one dish is still waiting on the pass.
        Queued => "queued",
        /// Every dish that counts is ready and the waiter can collect them.
        Ready => "ready",
        /// Every dish that counts has reached the table.
        Served => "served",
        /// Every dish on the ticket was voided, so the ticket itself is void.
        Voided => "voided",
    }
}

labelled_enum! {
    /// Where one dish has got to. The same four words as a ticket, on purpose.
    LineStatus {
        /// Sent to the kitchen, not yet cooked.
        Queued => "queued",
        /// Off the pass, waiting to be carried out.
        Ready => "ready",
        /// On the table.
        Served => "served",
        /// Cancelled, with a reason and the staff member who cancelled it.
        Voided => "voided",
    }
}

labelled_enum! {
    /// Where a payment document has got to.
    BillStatus {
        /// Still collecting lines. Figures other than the subtotal stay zero.
        Open => "open",
        /// Numbered, totalled, and never edited again.
        Closed => "closed",
        /// Abandoned before closing. Consumes no bill number.
        Voided => "voided",
    }
}

labelled_enum! {
    /// What a dish is, as the square mark on an Indian menu says it.
    ///
    /// Required on every dish. There is deliberately no "unknown": a dish
    /// nobody chose a marker for would be drawn as something, and drawing a
    /// chicken dish as vegetarian is the one mistake on a menu that is not
    /// merely embarrassing.
    Diet {
        /// No meat, no fish, no egg. A filled circle in the square.
        Veg => "veg",
        /// Meat or fish. A filled triangle in the square.
        NonVeg => "non_veg",
        /// Egg, and nothing else that is not vegetarian. A filled oval.
        Egg => "egg",
    }
}

labelled_enum! {
    /// How a closed bill was paid.
    ///
    /// The product takes no payment itself, so this records what happened at the
    /// till or the card machine. No card data, nothing in payment card industry
    /// scope.
    PaymentMethod {
        /// Notes and coins.
        Cash => "cash",
        /// A card machine the restaurant already owns.
        Card => "card",
        /// Anything else, such as a voucher or a bank transfer.
        Other => "other",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every label survives a trip out to Postgres and back.
    ///
    /// A mismatch here would not fail to compile and would not fail to insert.
    /// It would write one string and read back another, which is the kind of bug
    /// that only shows up as a row that has quietly become the wrong status.
    #[test]
    fn every_label_round_trips() {
        macro_rules! check {
            ($ty:ty, [$($variant:expr),* $(,)?]) => {
                $(
                    let label = $variant.as_label();
                    assert_eq!(
                        <$ty>::from_label(label),
                        Some($variant),
                        "{:?} writes {label:?} but does not read back from it",
                        $variant
                    );
                )*
            };
        }

        check!(
            StaffRole,
            [StaffRole::Admin, StaffRole::Waiter, StaffRole::Chef]
        );
        check!(VisitStatus, [VisitStatus::Open, VisitStatus::Closed]);
        check!(
            RoundStatus,
            [
                RoundStatus::Queued,
                RoundStatus::Ready,
                RoundStatus::Served,
                RoundStatus::Voided
            ]
        );
        check!(
            LineStatus,
            [
                LineStatus::Queued,
                LineStatus::Ready,
                LineStatus::Served,
                LineStatus::Voided
            ]
        );
        check!(
            BillStatus,
            [BillStatus::Open, BillStatus::Closed, BillStatus::Voided]
        );
        check!(
            PaymentMethod,
            [
                PaymentMethod::Cash,
                PaymentMethod::Card,
                PaymentMethod::Other
            ]
        );
        check!(Diet, [Diet::Veg, Diet::NonVeg, Diet::Egg]);
    }

    /// A value the database knows about and this enum does not must not decode.
    #[test]
    fn an_unknown_label_is_refused_rather_than_guessed() {
        assert_eq!(LineStatus::from_label("plated"), None);
        assert_eq!(StaffRole::from_label("owner"), None);
        assert_eq!(Diet::from_label("vegan"), None);
    }
}
