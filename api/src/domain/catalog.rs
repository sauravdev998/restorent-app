//! What a restaurant sets up before it can serve anybody: the restaurant's own
//! settings, its tax rules, its menu, and its floor.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

use super::enums::Diet;
use super::error::FieldError;
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
    /// Where it is, as an ISO 3166-1 alpha-2 code. Names the row in
    /// `locales/countries.json` that registration took the currency, the
    /// timezone, the default language, and the formatting locale from.
    pub country_code: String,
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
    /// How many seconds a kitchen ticket may wait before the pass draws it
    /// amber. Always below [`Self::kitchen_late_after_seconds`], which the
    /// database holds as a check constraint as well as the handler checking it.
    pub kitchen_warning_after_seconds: i32,
    /// How many seconds a kitchen ticket may wait before the pass draws it red.
    ///
    /// Per restaurant because fifteen minutes is a fast kitchen's disaster and a
    /// slow one's ordinary Tuesday, and only the restaurant knows which it is.
    pub kitchen_late_after_seconds: i32,
    /// Bumped by every write that changes the row. An edit naming an older one
    /// is refused rather than merged over somebody else's change.
    pub version: i32,
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
    /// Which edit of the row this is. A rename naming an older one is refused
    /// as stale rather than written over somebody else's change.
    pub version: i32,
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
    /// Whether it is veg, non veg, or egg.
    pub diet: Diet,
    /// Whether the kitchen can currently make it. Switching this off stops
    /// waiters ordering it and leaves dishes already on an open bill alone.
    pub is_available: bool,
    /// Where it sits in the printed order.
    pub position: i32,
    /// Which edit of the row this is. Every write that changes the dish bumps
    /// it, the availability switch included, so an edit form opened before the
    /// kitchen switched a dish off is refused rather than switching it back on.
    pub version: i32,
    /// When it was archived, if it was.
    pub archived_at: Option<DateTime<Utc>>,
}

/// A dish taken off the menu, with what the admin needs to put it back.
///
/// Its category is carried by name as well as by id, because the category may
/// have been archived too, and an archived category no longer appears anywhere
/// else the screen could look its name up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchivedDish {
    /// The dish itself, with `archived_at` set.
    pub dish: Dish,
    /// What its old category is called.
    pub category_name: String,
    /// Whether that category is still live, and so can take the dish back.
    pub category_live: bool,
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
    /// Which edit of the row this is. A rename naming an older one is refused
    /// as stale rather than written over somebody else's change.
    pub version: i32,
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
    /// Which edit of the row this is. An edit naming an older one is refused
    /// as stale. A reorder does not change it.
    pub version: i32,
    /// When it was archived, if it was.
    pub archived_at: Option<DateTime<Utc>>,
}

/// A live table as the admin's floor shows it: the table, and whether a party
/// is at it.
///
/// Occupancy is not a table state. It is whether an open visit exists, read in
/// the same snapshot as the table, so the admin can see why a table cannot be
/// removed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdminFloorTable {
    /// The table itself.
    pub table: DiningTable,
    /// Whether it has an open visit right now.
    pub occupied: bool,
}

/// A table taken off the floor, with what the admin needs to put it back.
///
/// Its section is carried by name as well as by id, because the section may
/// have been archived too, and then appears nowhere else the screen could look
/// its name up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchivedTable {
    /// The table itself, with `archived_at` set.
    pub table: DiningTable,
    /// What its old section is called, when it had one.
    pub section_name: Option<String>,
    /// Whether that section is still live, and so can take the table back.
    /// False when it had no section.
    pub section_live: bool,
}

// ===========================================================================
// The kitchen's two ageing thresholds
// ===========================================================================

/// The least a kitchen threshold may be, in seconds.
///
/// A minute. Below that every ticket is amber the moment it is sent, which turns
/// the one signal a chef acts on into wallpaper.
pub const KITCHEN_THRESHOLD_MIN_SECONDS: i32 = 60;

/// The most a kitchen threshold may be, in seconds. Four hours, which is longer
/// than any single dish a restaurant sends to one table.
pub const KITCHEN_THRESHOLD_MAX_SECONDS: i32 = 14_400;

/// When the pass turns a waiting ticket amber, and when it turns it red.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KitchenThresholds {
    /// Seconds after which a ticket reads amber.
    pub warning_after_seconds: i32,
    /// Seconds after which it reads red. Always the larger of the two.
    pub late_after_seconds: i32,
}

/// Which of the two thresholds an edit was refused over, and why.
///
/// Carried rather than a field name, because a wire field name is
/// `presentation`'s word for the box and this layer does not know it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThresholdProblem {
    /// The warning threshold was not accepted.
    Warning(FieldError),
    /// The late threshold was not accepted.
    Late(FieldError),
}

/// Works out what the pair becomes when an edit naming either one, or both, or
/// neither, is laid over what is stored, and checks the result as a pair.
///
/// The merge is the whole point. Both fields are independently optional while
/// the ordering rule spans both, so an edit raising only the warning can cross a
/// stored late value it never mentions. Checking each field alone would accept
/// it and leave the pass with amber starting after red.
///
/// A crossed pair is reported against the late threshold, with
/// [`FieldError::BeforeStart`]: the pair is a range, and what has gone wrong is
/// that its end comes before its start. That is the same field whichever of the
/// two the edit named, so the answer does not depend on which box was typed in,
/// and both boxes sit together on the settings screen.
///
/// # Errors
///
/// Returns [`ThresholdProblem::Warning`] or [`ThresholdProblem::Late`] naming
/// the threshold that was refused: [`FieldError::TooSmall`] or
/// [`FieldError::TooLarge`] for one outside the bounds, and
/// [`FieldError::BeforeStart`] on the late one for a crossed pair.
pub fn merge_kitchen_thresholds(
    submitted_warning: Option<i32>,
    submitted_late: Option<i32>,
    stored: KitchenThresholds,
) -> Result<KitchenThresholds, ThresholdProblem> {
    if let Some(warning) = submitted_warning {
        bounds(warning).map_err(ThresholdProblem::Warning)?;
    }

    if let Some(late) = submitted_late {
        bounds(late).map_err(ThresholdProblem::Late)?;
    }

    let merged = KitchenThresholds {
        warning_after_seconds: submitted_warning.unwrap_or(stored.warning_after_seconds),
        late_after_seconds: submitted_late.unwrap_or(stored.late_after_seconds),
    };

    if merged.warning_after_seconds >= merged.late_after_seconds {
        return Err(ThresholdProblem::Late(FieldError::BeforeStart));
    }

    Ok(merged)
}

/// Whether one threshold is inside the bounds both columns hold.
fn bounds(seconds: i32) -> Result<(), FieldError> {
    if seconds < KITCHEN_THRESHOLD_MIN_SECONDS {
        return Err(FieldError::TooSmall);
    }

    if seconds > KITCHEN_THRESHOLD_MAX_SECONDS {
        return Err(FieldError::TooLarge);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const STORED: KitchenThresholds = KitchenThresholds {
        warning_after_seconds: 600,
        late_after_seconds: 900,
    };

    /// covers: AC-20
    #[test]
    fn an_edit_naming_neither_threshold_keeps_both() {
        assert_eq!(merge_kitchen_thresholds(None, None, STORED), Ok(STORED));
    }

    /// covers: AC-20
    #[test]
    fn either_threshold_may_be_changed_on_its_own() {
        assert_eq!(
            merge_kitchen_thresholds(Some(300), None, STORED),
            Ok(KitchenThresholds {
                warning_after_seconds: 300,
                late_after_seconds: 900,
            })
        );
        assert_eq!(
            merge_kitchen_thresholds(None, Some(1_200), STORED),
            Ok(KitchenThresholds {
                warning_after_seconds: 600,
                late_after_seconds: 1_200,
            })
        );
    }

    /// covers: AC-20
    ///
    /// The case the merge exists for. Nothing about 1200 on its own is wrong; it
    /// is wrong only once it is laid over a stored late value of 900.
    #[test]
    fn raising_only_the_warning_over_the_stored_late_value_is_refused() {
        assert_eq!(
            merge_kitchen_thresholds(Some(1_200), None, STORED),
            Err(ThresholdProblem::Late(FieldError::BeforeStart))
        );
    }

    /// covers: AC-20
    #[test]
    fn lowering_only_the_late_value_under_the_stored_warning_is_refused() {
        assert_eq!(
            merge_kitchen_thresholds(None, Some(300), STORED),
            Err(ThresholdProblem::Late(FieldError::BeforeStart))
        );
    }

    /// covers: AC-20
    #[test]
    fn the_two_may_not_be_equal_because_amber_would_never_show() {
        assert_eq!(
            merge_kitchen_thresholds(Some(900), Some(900), STORED),
            Err(ThresholdProblem::Late(FieldError::BeforeStart))
        );
    }

    /// covers: AC-20
    #[test]
    fn each_threshold_is_refused_outside_the_bounds_naming_itself() {
        assert_eq!(
            merge_kitchen_thresholds(Some(59), None, STORED),
            Err(ThresholdProblem::Warning(FieldError::TooSmall))
        );
        assert_eq!(
            merge_kitchen_thresholds(Some(14_401), None, STORED),
            Err(ThresholdProblem::Warning(FieldError::TooLarge))
        );
        assert_eq!(
            merge_kitchen_thresholds(None, Some(59), STORED),
            Err(ThresholdProblem::Late(FieldError::TooSmall))
        );
        assert_eq!(
            merge_kitchen_thresholds(None, Some(14_401), STORED),
            Err(ThresholdProblem::Late(FieldError::TooLarge))
        );
    }

    /// covers: AC-20
    ///
    /// The bounds are checked before the pair, so an edit that is both out of
    /// range and crossed names the out of range field rather than the ordering.
    #[test]
    fn the_bounds_are_reported_before_the_ordering() {
        assert_eq!(
            merge_kitchen_thresholds(Some(14_401), Some(59), STORED),
            Err(ThresholdProblem::Warning(FieldError::TooLarge))
        );
    }

    /// covers: AC-20
    #[test]
    fn the_bounds_themselves_are_accepted() {
        assert_eq!(
            merge_kitchen_thresholds(
                Some(KITCHEN_THRESHOLD_MIN_SECONDS),
                Some(KITCHEN_THRESHOLD_MAX_SECONDS),
                STORED
            ),
            Ok(KitchenThresholds {
                warning_after_seconds: 60,
                late_after_seconds: 14_400,
            })
        );
    }
}
