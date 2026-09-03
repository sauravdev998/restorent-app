//! The wire shapes this feature adds.
//!
//! Nothing crosses the boundary as a domain entity, so these are separate types
//! rather than serde derives on `domain::people::Staff` and
//! `domain::catalog::Restaurant`. That is what keeps `utoipa` out of the inner
//! layers, and it is also what lets the bundle carry exactly the fields the
//! browser needs rather than every column those entities happen to hold.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::domain::catalog::Restaurant;
use crate::domain::enums::StaffRole;
use crate::domain::language::LanguageCode;
use crate::domain::people::Staff;

/// What a member of staff is allowed to be, on the wire.
///
/// A separate enum from [`StaffRole`] because that one lives in the domain and
/// may not learn that `utoipa` exists. The strings are identical, and the two
/// tests below are what keep them that way.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum RoleDto {
    /// Runs the restaurant.
    Admin,
    /// Takes orders at the table.
    Waiter,
    /// Works the kitchen screen.
    Chef,
}

impl From<StaffRole> for RoleDto {
    fn from(role: StaffRole) -> Self {
        match role {
            StaffRole::Admin => Self::Admin,
            StaffRole::Waiter => Self::Waiter,
            StaffRole::Chef => Self::Chef,
        }
    }
}

/// Who the signed in person is.
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StaffDto {
    /// Which staff member this is.
    ///
    /// A bare uuid rather than the domain's `StaffId`, because a newtype in
    /// `domain/` may not learn that `utoipa` exists. The wire has always been
    /// where identifiers lose their types.
    pub id: Uuid,
    /// What to call them on screen.
    pub display_name: String,
    /// The address they sign in with, exactly as they typed it when they
    /// registered. Handed back unflattened so the account screen shows them
    /// their own capitalisation.
    pub email: String,
    /// What they are allowed to be.
    pub role: RoleDto,
    /// Their own interface language, or `null` for "whatever the restaurant
    /// uses", which is what a new account has.
    pub language: Option<String>,
}

impl From<Staff> for StaffDto {
    fn from(staff: Staff) -> Self {
        Self {
            id: staff.id.as_uuid(),
            display_name: staff.display_name,
            email: staff.email,
            role: staff.role.into(),
            language: staff
                .language
                .as_ref()
                .map(LanguageCode::as_str)
                .map(str::to_owned),
        }
    }
}

/// The restaurant the signed in person works at, as every screen reads it.
///
/// Carries the five settings spec 0005's formatting layer needs and nothing
/// about money beyond the currency: figures on a bill come from the bill.
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RestaurantDto {
    /// Which restaurant this is.
    pub id: Uuid,
    /// What it is called.
    pub name: String,
    /// Where it is, for printing on a bill. `null` when nobody has set one.
    pub address: Option<String>,
    /// Where it is, as an ISO 3166-1 alpha-2 code.
    pub country_code: String,
    /// What it charges in.
    pub currency_code: String,
    /// How many decimal places that currency uses.
    pub currency_decimals: u32,
    /// Its own IANA timezone. Every timestamp on a screen is converted with it,
    /// never with the device's.
    pub timezone: String,
    /// What the kitchen screen reads, and the fallback for anybody with no
    /// personal setting.
    pub default_language: String,
    /// How money, numbers, dates, and times are written here. Deliberately not
    /// derived from the language.
    pub formatting_locale: String,
}

impl From<Restaurant> for RestaurantDto {
    fn from(restaurant: Restaurant) -> Self {
        Self {
            id: restaurant.id.as_uuid(),
            name: restaurant.name,
            address: restaurant.address,
            country_code: restaurant.country_code,
            currency_code: restaurant.currency.code().to_owned(),
            currency_decimals: restaurant.currency.decimals(),
            timezone: restaurant.timezone,
            default_language: restaurant.default_language.as_str().to_owned(),
            formatting_locale: restaurant.formatting_locale.as_str().to_owned(),
        }
    }
}

/// Everything the browser needs to know who it is talking for.
///
/// Returned identically by register, sign in, `GET /api/me`, `PATCH /api/me`,
/// and `PATCH /api/restaurant`, so the browser has one type and one cache
/// entry. Five endpoints returning five nearly identical shapes is how a client
/// ends up with five slightly different ideas of who is signed in.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct IdentityBundle {
    /// Who is signed in.
    pub staff: StaffDto,
    /// Where they work.
    pub restaurant: RestaurantDto,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The wire strings and the database strings have to be the same words.
    /// They are two enums in two layers, so nothing but this makes them agree,
    /// and a divergence would be a client branching on a role the server never
    /// sends.
    #[test]
    fn every_role_travels_as_the_word_the_database_stores() {
        for role in [StaffRole::Admin, StaffRole::Waiter, StaffRole::Chef] {
            let on_the_wire = serde_json::to_value(RoleDto::from(role))
                .expect("a role serialises")
                .as_str()
                .map(str::to_owned)
                .expect("a role is a string on the wire");

            assert_eq!(
                on_the_wire,
                role.as_label(),
                "{role:?} is {:?} in the database and {on_the_wire:?} on the wire",
                role.as_label()
            );
        }
    }

    /// A person with no personal language is `null` rather than absent, so the
    /// client can tell "follow the restaurant" from "the server did not say".
    #[test]
    fn a_staff_member_with_no_personal_language_says_so_explicitly() {
        let staff = Staff {
            id: crate::domain::ids::StaffId::new(),
            email: "Ada@Example.com".to_owned(),
            display_name: "Ada".to_owned(),
            role: StaffRole::Admin,
            language: None,
            deactivated_at: None,
        };

        let json = serde_json::to_value(StaffDto::from(staff)).expect("a staff member serialises");

        assert_eq!(json["language"], serde_json::Value::Null);
        assert_eq!(
            json["email"], "Ada@Example.com",
            "the address came back flattened, so the account screen shows something the \
             person did not type"
        );
    }
}
