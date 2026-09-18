//! The record of consequential changes.
//!
//! Not optional, and not a general activity log. This schema holds money and
//! access control, so the changes that move either one are written down with who
//! did it and what the value was before: voids, bill closes, price edits, tax
//! edits, service charge edits, role changes, and deactivations. Every change to
//! the menu except a reorder joins them, because a menu edit is a price edit
//! more often than not and the admin screen makes every kind of one. Every
//! change to the floor except a reorder joins them too, so "who removed table
//! 7" has an answer.
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
    /// An owner registered a restaurant and became its admin.
    RestaurantRegistered,
    /// Somebody changed their own password.
    PasswordChanged,
    /// An admin edited the restaurant's settings.
    RestaurantSettingsUpdated,
    /// A dish was cancelled off a ticket.
    LineVoided,
    /// A bill was closed, numbered, and totalled.
    BillClosed,
    /// A dish's name, description, price, diet marker, or category changed.
    DishEdited,
    /// A dish was archived off the menu.
    DishArchived,
    /// A dish was added to the menu.
    DishCreated,
    /// An archived dish was put back on the menu.
    DishRestored,
    /// A dish was switched on or off, most often because the kitchen ran out.
    DishAvailabilityChanged,
    /// A menu category was added.
    MenuCategoryCreated,
    /// A menu category was renamed.
    MenuCategoryRenamed,
    /// A menu category was archived off the menu.
    MenuCategoryArchived,
    /// An archived menu category was put back on the menu.
    MenuCategoryRestored,
    /// A tax component's name or rate changed.
    TaxComponentEdited,
    /// The restaurant's service charge changed.
    ServiceChargeEdited,
    /// An admin created a member of staff.
    StaffCreated,
    /// An admin changed a staff member's display name.
    StaffEdited,
    /// A staff member's role changed.
    StaffRoleChanged,
    /// An admin wrote a new password onto somebody else's row.
    StaffPasswordReset,
    /// A staff member's account was switched off.
    StaffDeactivated,
    /// A switched off account was brought back.
    StaffReactivated,
    /// A table was added to the floor.
    DiningTableCreated,
    /// A table's label, seats, or section changed.
    DiningTableEdited,
    /// A table was taken off the floor.
    DiningTableArchived,
    /// An archived table was put back on the floor.
    DiningTableRestored,
    /// A table section was added.
    TableSectionCreated,
    /// A table section was renamed.
    TableSectionRenamed,
    /// A table section was taken off the floor.
    TableSectionArchived,
    /// An archived table section was put back on the floor.
    TableSectionRestored,
}

impl AuditAction {
    /// The exact string stored in the `action` column.
    #[must_use]
    pub const fn as_label(self) -> &'static str {
        match self {
            Self::RestaurantRegistered => "restaurant_registered",
            Self::PasswordChanged => "password_changed",
            Self::RestaurantSettingsUpdated => "restaurant_settings_updated",
            Self::LineVoided => "line_voided",
            Self::BillClosed => "bill_closed",
            Self::DishEdited => "dish_edited",
            Self::DishArchived => "dish_archived",
            Self::DishCreated => "dish_created",
            Self::DishRestored => "dish_restored",
            Self::DishAvailabilityChanged => "dish_availability_changed",
            Self::MenuCategoryCreated => "menu_category_created",
            Self::MenuCategoryRenamed => "menu_category_renamed",
            Self::MenuCategoryArchived => "menu_category_archived",
            Self::MenuCategoryRestored => "menu_category_restored",
            Self::TaxComponentEdited => "tax_component_edited",
            Self::ServiceChargeEdited => "service_charge_edited",
            Self::StaffCreated => "staff_created",
            Self::StaffEdited => "staff_edited",
            Self::StaffRoleChanged => "staff_role_changed",
            Self::StaffPasswordReset => "staff_password_reset",
            Self::StaffDeactivated => "staff_deactivated",
            Self::StaffReactivated => "staff_reactivated",
            Self::DiningTableCreated => "dining_table_created",
            Self::DiningTableEdited => "dining_table_edited",
            Self::DiningTableArchived => "dining_table_archived",
            Self::DiningTableRestored => "dining_table_restored",
            Self::TableSectionCreated => "table_section_created",
            Self::TableSectionRenamed => "table_section_renamed",
            Self::TableSectionArchived => "table_section_archived",
            Self::TableSectionRestored => "table_section_restored",
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Every action, in one place, so the tests below cover all of them.
    ///
    /// Kept honest by [`position`]: adding a variant stops that match compiling,
    /// and filling it in stops this array being the right length.
    const ALL: [AuditAction; 30] = [
        AuditAction::RestaurantRegistered,
        AuditAction::PasswordChanged,
        AuditAction::RestaurantSettingsUpdated,
        AuditAction::LineVoided,
        AuditAction::BillClosed,
        AuditAction::DishEdited,
        AuditAction::DishArchived,
        AuditAction::DishCreated,
        AuditAction::DishRestored,
        AuditAction::DishAvailabilityChanged,
        AuditAction::MenuCategoryCreated,
        AuditAction::MenuCategoryRenamed,
        AuditAction::MenuCategoryArchived,
        AuditAction::MenuCategoryRestored,
        AuditAction::TaxComponentEdited,
        AuditAction::ServiceChargeEdited,
        AuditAction::StaffCreated,
        AuditAction::StaffEdited,
        AuditAction::StaffRoleChanged,
        AuditAction::StaffPasswordReset,
        AuditAction::StaffDeactivated,
        AuditAction::StaffReactivated,
        AuditAction::DiningTableCreated,
        AuditAction::DiningTableEdited,
        AuditAction::DiningTableArchived,
        AuditAction::DiningTableRestored,
        AuditAction::TableSectionCreated,
        AuditAction::TableSectionRenamed,
        AuditAction::TableSectionArchived,
        AuditAction::TableSectionRestored,
    ];

    /// Where each action sits in [`ALL`].
    ///
    /// The match is exhaustive on purpose. A new variant fails to compile here,
    /// which is the only thing that stops [`ALL`] silently falling behind the
    /// enum and the tests quietly covering less than they claim to.
    const fn position(action: AuditAction) -> usize {
        match action {
            AuditAction::RestaurantRegistered => 0,
            AuditAction::PasswordChanged => 1,
            AuditAction::RestaurantSettingsUpdated => 2,
            AuditAction::LineVoided => 3,
            AuditAction::BillClosed => 4,
            AuditAction::DishEdited => 5,
            AuditAction::DishArchived => 6,
            AuditAction::DishCreated => 7,
            AuditAction::DishRestored => 8,
            AuditAction::DishAvailabilityChanged => 9,
            AuditAction::MenuCategoryCreated => 10,
            AuditAction::MenuCategoryRenamed => 11,
            AuditAction::MenuCategoryArchived => 12,
            AuditAction::MenuCategoryRestored => 13,
            AuditAction::TaxComponentEdited => 14,
            AuditAction::ServiceChargeEdited => 15,
            AuditAction::StaffCreated => 16,
            AuditAction::StaffEdited => 17,
            AuditAction::StaffRoleChanged => 18,
            AuditAction::StaffPasswordReset => 19,
            AuditAction::StaffDeactivated => 20,
            AuditAction::StaffReactivated => 21,
            AuditAction::DiningTableCreated => 22,
            AuditAction::DiningTableEdited => 23,
            AuditAction::DiningTableArchived => 24,
            AuditAction::DiningTableRestored => 25,
            AuditAction::TableSectionCreated => 26,
            AuditAction::TableSectionRenamed => 27,
            AuditAction::TableSectionArchived => 28,
            AuditAction::TableSectionRestored => 29,
        }
    }

    #[test]
    fn the_list_the_other_tests_run_over_holds_every_action() {
        for (index, action) in ALL.into_iter().enumerate() {
            assert_eq!(
                position(action),
                index,
                "{action:?} is not where the list says it is"
            );
        }
    }

    /// covers: AC-14
    ///
    /// These strings are the `action` column. Nothing else writes it, and the
    /// only way to read the log back is to match on them, so changing one turns
    /// every row written before the change into a row that no longer answers the
    /// question it was written for. Spelled out one by one rather than derived,
    /// because a rule that generates them would change its output alongside any
    /// rename and pin nothing.
    #[test]
    fn every_action_writes_the_exact_string_the_log_is_read_back_by() {
        assert_eq!(
            AuditAction::RestaurantRegistered.as_label(),
            "restaurant_registered"
        );
        assert_eq!(AuditAction::PasswordChanged.as_label(), "password_changed");
        assert_eq!(
            AuditAction::RestaurantSettingsUpdated.as_label(),
            "restaurant_settings_updated"
        );
        assert_eq!(AuditAction::LineVoided.as_label(), "line_voided");
        assert_eq!(AuditAction::BillClosed.as_label(), "bill_closed");
        assert_eq!(AuditAction::DishEdited.as_label(), "dish_edited");
        assert_eq!(AuditAction::DishArchived.as_label(), "dish_archived");
        assert_eq!(AuditAction::DishCreated.as_label(), "dish_created");
        assert_eq!(AuditAction::DishRestored.as_label(), "dish_restored");
        assert_eq!(
            AuditAction::DishAvailabilityChanged.as_label(),
            "dish_availability_changed"
        );
        assert_eq!(
            AuditAction::MenuCategoryCreated.as_label(),
            "menu_category_created"
        );
        assert_eq!(
            AuditAction::MenuCategoryRenamed.as_label(),
            "menu_category_renamed"
        );
        assert_eq!(
            AuditAction::MenuCategoryArchived.as_label(),
            "menu_category_archived"
        );
        assert_eq!(
            AuditAction::MenuCategoryRestored.as_label(),
            "menu_category_restored"
        );
        assert_eq!(
            AuditAction::TaxComponentEdited.as_label(),
            "tax_component_edited"
        );
        assert_eq!(
            AuditAction::ServiceChargeEdited.as_label(),
            "service_charge_edited"
        );
        assert_eq!(AuditAction::StaffCreated.as_label(), "staff_created");
        assert_eq!(AuditAction::StaffEdited.as_label(), "staff_edited");
        assert_eq!(
            AuditAction::StaffRoleChanged.as_label(),
            "staff_role_changed"
        );
        assert_eq!(
            AuditAction::StaffPasswordReset.as_label(),
            "staff_password_reset"
        );
        assert_eq!(
            AuditAction::StaffDeactivated.as_label(),
            "staff_deactivated"
        );
        assert_eq!(
            AuditAction::StaffReactivated.as_label(),
            "staff_reactivated"
        );
        assert_eq!(
            AuditAction::DiningTableCreated.as_label(),
            "dining_table_created"
        );
        assert_eq!(
            AuditAction::DiningTableEdited.as_label(),
            "dining_table_edited"
        );
        assert_eq!(
            AuditAction::DiningTableArchived.as_label(),
            "dining_table_archived"
        );
        assert_eq!(
            AuditAction::DiningTableRestored.as_label(),
            "dining_table_restored"
        );
        assert_eq!(
            AuditAction::TableSectionCreated.as_label(),
            "table_section_created"
        );
        assert_eq!(
            AuditAction::TableSectionRenamed.as_label(),
            "table_section_renamed"
        );
        assert_eq!(
            AuditAction::TableSectionArchived.as_label(),
            "table_section_archived"
        );
        assert_eq!(
            AuditAction::TableSectionRestored.as_label(),
            "table_section_restored"
        );
    }

    /// covers: AC-14
    ///
    /// Two actions sharing a label would merge two kinds of change into one in
    /// the log, and the merge would be invisible: every row still writes, every
    /// row still reads, and the count of voids quietly includes bill closes.
    #[test]
    fn no_two_actions_share_a_label() {
        for action in ALL {
            let clashes = ALL
                .into_iter()
                .filter(|other| other.as_label() == action.as_label())
                .count();

            assert_eq!(
                clashes,
                1,
                "{action:?} shares its label {:?} with another action",
                action.as_label()
            );
        }
    }

    /// The `audit_log_action_not_blank` check refuses a blank action, so a label
    /// that was empty, or padded, would fail at the insert rather than here.
    #[test]
    fn no_label_is_blank_or_padded() {
        for action in ALL {
            let label = action.as_label();

            assert!(!label.trim().is_empty(), "{action:?} has a blank label");
            assert_eq!(
                label,
                label.trim(),
                "{action:?} has a label with space around it"
            );
        }
    }
}
