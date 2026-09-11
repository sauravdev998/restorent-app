//! The wire shapes this feature adds.
//!
//! Nothing crosses the boundary as a domain entity, so these are separate types
//! rather than serde derives on `domain::people::Staff` and
//! `domain::catalog::Restaurant`. That is what keeps `utoipa` out of the inner
//! layers, and it is also what lets the bundle carry exactly the fields the
//! browser needs rather than every column those entities happen to hold.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::domain::billing::{Bill, BillTax};
use crate::domain::catalog::{Dish, Restaurant};
use crate::domain::enums::{Diet, LineStatus, RoundStatus, StaffRole};
use crate::domain::language::LanguageCode;
use crate::domain::people::Staff;
use crate::domain::service::{OrderLine, OrderRound};

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

// ===========================================================================
// The order thread
//
// Two rules run through every shape below.
//
// **Money is a decimal string, never a number.** It is `numeric(14,4)` in
// Postgres and `Decimal` in Rust precisely so no float ever touches it, and a
// JSON number is a float in every browser. The web formats the string exactly,
// with the currency's own decimals, so what a customer is charged and what is
// printed on their bill are the same value.
//
// **A status travels as the word the database stores.** These enums mirror the
// domain's, which mirror the Postgres types, and the tests at the foot of this
// file are what keep all three saying the same words.
// ===========================================================================

/// Where one dish has got to, on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum LineStatusDto {
    /// Sent to the kitchen, not yet cooked.
    Queued,
    /// Off the pass, waiting to be carried out.
    Ready,
    /// On the table.
    Served,
    /// Cancelled.
    Voided,
}

impl From<LineStatus> for LineStatusDto {
    fn from(status: LineStatus) -> Self {
        match status {
            LineStatus::Queued => Self::Queued,
            LineStatus::Ready => Self::Ready,
            LineStatus::Served => Self::Served,
            LineStatus::Voided => Self::Voided,
        }
    }
}

/// Where a whole ticket has got to, on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum RoundStatusDto {
    /// At least one dish is still on the pass.
    Queued,
    /// Every dish that counts is ready to be carried out.
    Ready,
    /// Every dish that counts has reached the table.
    Served,
    /// Every dish on it was cancelled.
    Voided,
}

impl From<RoundStatus> for RoundStatusDto {
    fn from(status: RoundStatus) -> Self {
        match status {
            RoundStatus::Queued => Self::Queued,
            RoundStatus::Ready => Self::Ready,
            RoundStatus::Served => Self::Served,
            RoundStatus::Voided => Self::Voided,
        }
    }
}

/// One dish on one ticket, as every screen reads it.
///
/// The name and the price are the ones copied onto the line when the round was
/// sent, not today's menu. That is what lets a bill from last Tuesday stay
/// true after a reprice.
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct OrderLineDto {
    /// Which line this is.
    pub id: Uuid,
    /// Which menu item it came from.
    pub dish_id: Uuid,
    /// What the dish was called when it was ordered.
    pub dish_name: String,
    /// How many.
    pub quantity: i32,
    /// What one of them cost, as an exact decimal string.
    pub unit_price: String,
    /// Quantity times unit price, as an exact decimal string.
    pub line_total: String,
    /// What the guest asked for, such as no onions.
    pub note: Option<String>,
    /// Where this one dish has got to.
    pub status: LineStatusDto,
}

impl From<OrderLine> for OrderLineDto {
    fn from(line: OrderLine) -> Self {
        Self {
            id: line.id.as_uuid(),
            dish_id: line.dish_id.as_uuid(),
            dish_name: line.dish_name,
            quantity: line.quantity,
            unit_price: line.unit_price.to_string(),
            line_total: line.line_total.to_string(),
            note: line.note,
            status: line.status.into(),
        }
    }
}

/// One ticket with its dishes.
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct OrderRoundDto {
    /// Which ticket this is.
    pub id: Uuid,
    /// Which visit it belongs to.
    pub visit_id: Uuid,
    /// Its number within the visit, starting at one.
    pub sequence_no: i32,
    /// Where the whole ticket has got to, recomputed from its dishes.
    pub status: RoundStatusDto,
    /// When it reached the kitchen.
    pub sent_at: DateTime<Utc>,
    /// When the last dish came off the pass.
    pub ready_at: Option<DateTime<Utc>>,
    /// When the last dish reached the table.
    pub served_at: Option<DateTime<Utc>>,
    /// Every dish on it, oldest first.
    pub lines: Vec<OrderLineDto>,
}

impl OrderRoundDto {
    /// Builds one from a ticket and the dishes read alongside it.
    #[must_use]
    pub fn new(round: &OrderRound, lines: Vec<OrderLine>) -> Self {
        Self {
            id: round.id.as_uuid(),
            visit_id: round.visit_id.as_uuid(),
            sequence_no: round.sequence_no,
            status: round.status.into(),
            sent_at: round.sent_at,
            ready_at: round.ready_at,
            served_at: round.served_at,
            lines: lines.into_iter().map(OrderLineDto::from).collect(),
        }
    }
}

/// One tax line copied onto a bill when it closed.
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BillTaxDto {
    /// What the tax was called at the time.
    pub name: String,
    /// What rate was charged, as an exact decimal string.
    pub rate_percent: String,
    /// What that came to, already rounded to the bill's currency.
    pub amount: String,
}

impl From<BillTax> for BillTaxDto {
    fn from(tax: BillTax) -> Self {
        Self {
            name: tax.name,
            rate_percent: tax.rate_percent.to_string(),
            amount: tax.amount.to_string(),
        }
    }
}

/// A bill, open or closed, with every figure it currently carries.
///
/// The currency travels with it rather than being read from the restaurant,
/// because a closed bill keeps the currency it was charged in even after the
/// restaurant changes. A screen formatting a bill must use these two and not
/// the restaurant's current pair.
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BillDto {
    /// Which bill this is.
    pub id: Uuid,
    /// Its number within the restaurant, allocated at close. `null` while open.
    pub number: Option<i64>,
    /// Whether it is still collecting dishes.
    pub status: String,
    /// What it is charged in.
    pub currency_code: String,
    /// How many decimal places that currency uses.
    pub currency_decimals: u32,
    /// The sum of its dishes that were not cancelled.
    pub subtotal: String,
    /// The service charge rate applied, or `null` when the restaurant charges
    /// none.
    pub service_charge_percent: Option<String>,
    /// What that rate came to. Zero rather than null when there is none.
    pub service_charge_amount: String,
    /// The sum of the tax lines.
    pub tax_total: String,
    /// Subtotal plus service charge plus taxes, with no residue.
    pub total: String,
    /// When it closed, if it has.
    pub closed_at: Option<DateTime<Utc>>,
    /// The tax breakdown, empty until it closes.
    pub taxes: Vec<BillTaxDto>,
}

impl BillDto {
    /// Builds one from a bill and the tax lines read alongside it.
    #[must_use]
    pub fn new(bill: &Bill, taxes: Vec<BillTax>) -> Self {
        Self {
            id: bill.id.as_uuid(),
            number: bill.number,
            status: bill.status.as_label().to_owned(),
            currency_code: bill.currency.code().to_owned(),
            currency_decimals: bill.currency.decimals(),
            subtotal: bill.subtotal.to_string(),
            service_charge_percent: bill
                .service_charge_percent
                .map(|percent| percent.to_string()),
            service_charge_amount: bill.service_charge_amount.to_string(),
            tax_total: bill.tax_total.to_string(),
            total: bill.total.to_string(),
            closed_at: bill.closed_at,
            taxes: taxes.into_iter().map(BillTaxDto::from).collect(),
        }
    }
}

// ===========================================================================
// The menu
// ===========================================================================

/// Whether a dish is veg, non veg, or egg, on the wire.
///
/// The same three words the `dish_diet` enum stores, kept that way by the test
/// at the foot of this file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum DietDto {
    /// No meat, no fish, no egg.
    Veg,
    /// Meat or fish.
    NonVeg,
    /// Egg, and nothing else that is not vegetarian.
    Egg,
}

impl From<Diet> for DietDto {
    fn from(diet: Diet) -> Self {
        match diet {
            Diet::Veg => Self::Veg,
            Diet::NonVeg => Self::NonVeg,
            Diet::Egg => Self::Egg,
        }
    }
}

impl From<DietDto> for Diet {
    fn from(diet: DietDto) -> Self {
        match diet {
            DietDto::Veg => Self::Veg,
            DietDto::NonVeg => Self::NonVeg,
            DietDto::Egg => Self::Egg,
        }
    }
}

/// One dish, with everything an admin's edit form needs.
///
/// What every menu write answers with, and what the admin menu lists. The
/// `version` is the one the form sends back, which is how an edit made from a
/// stale form is told apart from a current one.
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DishDto {
    /// Which dish this is.
    pub id: Uuid,
    /// Which category it sits under.
    pub category_id: Uuid,
    /// What it is called.
    pub name: String,
    /// What it is, for the waiter to read out. `null` when there is none.
    pub description: Option<String>,
    /// What it costs, as an exact decimal string. Never a JSON number.
    pub price: String,
    /// Whether it is veg, non veg, or egg.
    pub diet: DietDto,
    /// Whether the kitchen can make it right now.
    pub available: bool,
    /// Which edit of the dish this is. Send it back with an edit.
    pub version: i32,
    /// When it was taken off the menu, or `null` while it is on it.
    pub archived_at: Option<DateTime<Utc>>,
}

impl From<Dish> for DishDto {
    fn from(dish: Dish) -> Self {
        Self {
            id: dish.id.as_uuid(),
            category_id: dish.category_id.as_uuid(),
            name: dish.name,
            description: dish.description,
            price: dish.price.to_string(),
            diet: dish.diet.into(),
            available: dish.is_available,
            version: dish.version,
            archived_at: dish.archived_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// covers: AC-13 (spec 0008)
    ///
    /// Three layers say these words: the Postgres enum, the domain enum, and
    /// this one. A divergence would be a screen drawing the wrong mark on a
    /// dish, which on this particular value is a vegetarian served chicken.
    #[test]
    fn every_diet_travels_as_the_word_the_database_stores() {
        for diet in [Diet::Veg, Diet::NonVeg, Diet::Egg] {
            let on_the_wire = serde_json::to_value(DietDto::from(diet)).expect("a diet serialises");

            assert_eq!(
                on_the_wire,
                serde_json::Value::String(diet.as_label().to_owned()),
                "{diet:?} is {:?} in the database and {on_the_wire} on the wire",
                diet.as_label()
            );
            assert_eq!(Diet::from(DietDto::from(diet)), diet);
        }
    }

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

    /// covers: AC-5, AC-7
    ///
    /// The same pairing the role test above checks, for the two statuses the
    /// kitchen and waiter screens branch on. Three layers say these words: the
    /// Postgres enum, the domain enum, and this one. Nothing but this makes the
    /// outer two agree, and a divergence would be a kitchen screen filtering on
    /// a status the server never sends.
    #[test]
    fn every_line_status_travels_as_the_word_the_database_stores() {
        for status in [
            LineStatus::Queued,
            LineStatus::Ready,
            LineStatus::Served,
            LineStatus::Voided,
        ] {
            let on_the_wire = serde_json::to_value(LineStatusDto::from(status))
                .expect("a line status serialises");

            assert_eq!(
                on_the_wire,
                serde_json::Value::String(status.as_label().to_owned()),
                "{status:?} is {:?} in the database and {on_the_wire} on the wire",
                status.as_label()
            );
        }
    }

    /// covers: AC-5, AC-7
    #[test]
    fn every_round_status_travels_as_the_word_the_database_stores() {
        for status in [
            RoundStatus::Queued,
            RoundStatus::Ready,
            RoundStatus::Served,
            RoundStatus::Voided,
        ] {
            let on_the_wire = serde_json::to_value(RoundStatusDto::from(status))
                .expect("a round status serialises");

            assert_eq!(
                on_the_wire,
                serde_json::Value::String(status.as_label().to_owned()),
                "{status:?} is {:?} in the database and {on_the_wire} on the wire",
                status.as_label()
            );
        }
    }

    /// covers: AC-10, AC-11
    ///
    /// The rule the whole money path rests on. A JSON number is a float in
    /// every browser, so a total serialised as one is a total that can come
    /// back as very nearly itself. Nothing downstream can recover from that,
    /// which is why the check is here rather than left to a screen noticing.
    #[test]
    fn every_money_figure_leaves_as_an_exact_decimal_string() {
        use std::str::FromStr as _;

        use crate::domain::billing::Bill;
        use crate::domain::enums::BillStatus;
        use crate::domain::ids::{BillId, StaffId, VisitId};
        use crate::domain::money::Currency;
        use rust_decimal::Decimal;

        let exact = Decimal::from_str("1234567.85").expect("a decimal literal");

        let bill = Bill {
            id: BillId::new(),
            visit_id: VisitId::new(),
            number: Some(7),
            status: BillStatus::Closed,
            currency: Currency::new("INR", 2).expect("a currency"),
            subtotal: exact,
            service_charge_percent: None,
            service_charge_amount: Decimal::ZERO,
            tax_total: Decimal::ZERO,
            total: exact,
            opened_by_staff_id: StaffId::new(),
            closed_by_staff_id: None,
            closed_at: None,
        };

        let json =
            serde_json::to_value(BillDto::new(&bill, Vec::new())).expect("a bill serialises");

        assert_eq!(
            json["total"], "1234567.85",
            "a total left as {} rather than an exact decimal string",
            json["total"]
        );
        assert!(
            json["subtotal"].is_string(),
            "a money figure left as a JSON number, which is a float in every browser"
        );
    }
}
