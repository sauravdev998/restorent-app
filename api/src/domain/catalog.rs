//! What a restaurant sets up before it can serve anybody: the restaurant's own
//! settings, its tax rules, its menu, and its floor.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

use super::ids::{
    DiningTableId, DishId, MenuCategoryId, RestaurantId, TableSectionId, TaxComponentId,
};
use super::language::{FormattingLocale, LanguageCode};
use super::money::Currency;

/// One restaurant on the platform, and the settings every bill it prints
/// depends on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Restaurant {
    /// Which restaurant this is.
    pub id: RestaurantId,
    /// What it is called.
    pub name: String,
    /// What it charges in.
    pub currency: Currency,
    /// Its own timezone, as an IANA name such as `Europe/Berlin`. The local day
    /// a bill belongs to is worked out from this, never from the server's clock,
    /// because a restaurant closing at one in the morning would otherwise post
    /// half its evening to the wrong day. Every timestamp on a screen is
    /// converted with it too, so a chef and an owner in different rooms read the
    /// same clock.
    pub timezone: String,
    /// What the kitchen screen reads, what a printed bill is written in, and
    /// what a member of staff with no personal setting sees.
    pub default_language: LanguageCode,
    /// How money, numbers, dates, and times are written here. Deliberately not
    /// derived from [`Self::default_language`]: an owner in India reading
    /// English still wants Indian grouping on their own figures, and a bill has
    /// to look identical to every member of staff whatever each of them reads.
    pub formatting_locale: FormattingLocale,
    /// The service charge it adds, if any. [`None`] means none, which yields an
    /// amount of zero on a bill rather than a null.
    pub service_charge_percent: Option<Decimal>,
    /// Where it is, for printing on a bill.
    pub address: Option<String>,
    /// Its tax registration number, for printing on a bill.
    pub tax_registration_number: Option<String>,
    /// When it was switched off, if it was. Deactivating leaves every row intact
    /// and reachable; only deleting the restaurant removes anything.
    pub deactivated_at: Option<DateTime<Utc>>,
}

/// One tax the restaurant charges.
///
/// A restaurant may have several, such as a sales tax and a local levy, and each
/// appears as its own line on a bill so the customer can see the breakdown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaxComponent {
    /// Which tax this is.
    pub id: TaxComponentId,
    /// What it is called on a bill.
    pub name: String,
    /// What rate it charges.
    pub rate_percent: Decimal,
    /// Where it sits in the printed order.
    pub position: i32,
    /// When it was archived, if it was. An archived component stops appearing on
    /// new bills and stays readable on old ones.
    pub archived_at: Option<DateTime<Utc>>,
}

/// A group of dishes on the menu, such as starters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuCategory {
    /// Which category this is.
    pub id: MenuCategoryId,
    /// What it is called.
    pub name: String,
    /// Where it sits in the printed order.
    pub position: i32,
    /// When it was archived, if it was.
    pub archived_at: Option<DateTime<Utc>>,
}

/// One item on the menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dish {
    /// Which dish this is.
    pub id: DishId,
    /// Which category it sits under.
    pub category_id: MenuCategoryId,
    /// What it is called today. A line copies this when it is ordered, so
    /// renaming the dish never rewrites an old bill.
    pub name: String,
    /// What it is, for the waiter to read out.
    pub description: Option<String>,
    /// What it costs today. Copied onto a line when it is ordered.
    pub price: Decimal,
    /// Whether the kitchen can currently make it. Switching this off stops
    /// waiters ordering it and leaves dishes already on an open bill alone.
    pub is_available: bool,
    /// Where it sits in the printed order.
    pub position: i32,
    /// When it was archived, if it was.
    pub archived_at: Option<DateTime<Utc>>,
}

/// A named group of tables, such as a terrace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableSection {
    /// Which section this is.
    pub id: TableSectionId,
    /// What it is called.
    pub name: String,
    /// Where it sits in the displayed order.
    pub position: i32,
    /// When it was archived, if it was.
    pub archived_at: Option<DateTime<Utc>>,
}

/// A physical table a party sits at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiningTable {
    /// Which table this is.
    pub id: DiningTableId,
    /// Which section it is in, if the restaurant groups its tables.
    pub section_id: Option<TableSectionId>,
    /// What the staff call it, such as `12` or `Bar 3`.
    pub label: String,
    /// How many it seats.
    pub seats: Option<i16>,
    /// Where it sits in the displayed order.
    pub position: i32,
    /// When it was archived, if it was.
    pub archived_at: Option<DateTime<Utc>>,
}
