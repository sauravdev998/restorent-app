//! Carries the domain's seven enums across to the Postgres enum types of the same
//! names.
//!
//! This module exists because of the layer rule. The obvious way to do this is
//! `#[derive(sqlx::Type)]` on each enum, but that would put a `sqlx` type in
//! `domain/`, which is exactly what the architecture forbids: the whole point of
//! the inner layers is that they do not know a database exists.
//!
//! Everything is in one crate, so there is no orphan rule to fight. The domain
//! enums stay plain Rust with a label pair, and the three `SQLx` traits are
//! implemented out here, next to the pool, where database machinery belongs.

use sqlx::Postgres;
use sqlx::encode::IsNull;
use sqlx::error::BoxDynError;
use sqlx::postgres::{PgArgumentBuffer, PgHasArrayType, PgTypeInfo, PgValueRef};

use crate::domain::enums::{
    BillStatus, Diet, LineStatus, PaymentMethod, RoundStatus, StaffRole, VisitStatus,
};

/// Wires one domain enum up to the Postgres enum type it mirrors.
///
/// A Postgres enum travels on the wire as the label text, in both directions,
/// which is why encoding is a byte copy and decoding is a lookup.
macro_rules! map_pg_enum {
    ($ty:ty, $pg_type:literal) => {
        impl sqlx::Type<Postgres> for $ty {
            fn type_info() -> PgTypeInfo {
                PgTypeInfo::with_name($pg_type)
            }
        }

        impl PgHasArrayType for $ty {
            fn array_type_info() -> PgTypeInfo {
                PgTypeInfo::with_name(concat!("_", $pg_type))
            }
        }

        impl sqlx::Encode<'_, Postgres> for $ty {
            fn encode_by_ref(&self, buf: &mut PgArgumentBuffer) -> Result<IsNull, BoxDynError> {
                buf.extend_from_slice(self.as_label().as_bytes());
                Ok(IsNull::No)
            }
        }

        impl sqlx::Decode<'_, Postgres> for $ty {
            fn decode(value: PgValueRef<'_>) -> Result<Self, BoxDynError> {
                let label = value.as_str()?;

                // A value the database has and this enum does not is a bug in a
                // migration, not a row to guess at. Failing here names the type
                // and the value, which is what makes it a two minute fix.
                Self::from_label(label).ok_or_else(|| {
                    BoxDynError::from(format!("{label:?} is not a known {} value", $pg_type))
                })
            }
        }
    };
}

map_pg_enum!(StaffRole, "staff_role");
map_pg_enum!(VisitStatus, "visit_status");
map_pg_enum!(RoundStatus, "round_status");
map_pg_enum!(LineStatus, "line_status");
map_pg_enum!(BillStatus, "bill_status");
map_pg_enum!(PaymentMethod, "payment_method");
map_pg_enum!(Diet, "dish_diet");
