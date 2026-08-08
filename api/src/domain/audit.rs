//! The record of consequential changes.
//!
//! Not optional, and not a general activity log. This schema holds money and
//! access control, so the changes that move either one are written down with who
//! did it and what the value was before: voids, bill closes, price edits, tax
//! edits, service charge edits, role changes, and deactivations.
//!
//! It has no retention policy, deliberately. It only records changes of
//! consequence, so it grows far more slowly than the bills do.

use chrono::{DateTime, Utc};
use serde_json::Value;

use super::ids::{AuditEntryId, StaffId};

/// What kind of change was recorded.
///
/// A closed list rather than free text, so that reading the log back is a match
/// rather than a string comparison somebody spells differently next year.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum AuditAction {
    /// A dish was cancelled off a ticket.
    LineVoided,
    /// A bill was closed, numbered, and totalled.
    BillClosed,
    /// A dish's price, name, or availability changed.
    DishEdited,
    /// A dish was archived off the menu.
    DishArchived,
    /// A tax component's name or rate changed.
    TaxComponentEdited,
    /// The restaurant's service charge changed.
    ServiceChargeEdited,
    /// A staff member's role changed.
    StaffRoleChanged,
    /// A staff member's account was switched off.
    StaffDeactivated,
}

impl AuditAction {
    /// The exact string stored in the `action` column.
    #[must_use]
    pub const fn as_label(self) -> &'static str {
        match self {
            Self::LineVoided => "line_voided",
            Self::BillClosed => "bill_closed",
            Self::DishEdited => "dish_edited",
            Self::DishArchived => "dish_archived",
            Self::TaxComponentEdited => "tax_component_edited",
            Self::ServiceChargeEdited => "service_charge_edited",
            Self::StaffRoleChanged => "staff_role_changed",
            Self::StaffDeactivated => "staff_deactivated",
        }
    }
}

/// One entry in the record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditEntry {
    /// Which entry this is.
    pub id: AuditEntryId,
    /// Who did it. Absent only for a change made by the system itself; every
    /// change the acceptance criteria name is staff initiated and carries one.
    pub actor_staff_id: Option<StaffId>,
    /// What kind of change it was.
    pub action: String,
    /// Which kind of thing changed, such as `dish`.
    pub entity_type: String,
    /// Which one changed.
    pub entity_id: uuid::Uuid,
    /// What it looked like before.
    pub before: Option<Value>,
    /// What it looks like now.
    pub after: Option<Value>,
    /// When it happened.
    pub occurred_at: DateTime<Utc>,
}
