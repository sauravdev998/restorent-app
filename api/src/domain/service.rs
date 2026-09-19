//! The service loop: a party at a table, the tickets they send, and the dishes
//! on them.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

use super::catalog::DiningTable;
use super::enums::{LineStatus, RoundStatus, VisitStatus, VoidReason};
use super::error::FieldError;
use super::ids::{BillId, DiningTableId, DishId, OrderLineId, OrderRoundId, StaffId, VisitId};

/// One party's stay at one table, from sitting down to leaving.
///
/// A visit owns table occupancy; a bill is a payment document drawn from its
/// lines. They are separate because occupancy and payment genuinely have
/// different lifetimes: a party can pay on two bills, or sit down and leave
/// without ordering at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Visit {
    /// Which visit this is.
    pub id: VisitId,
    /// Where the party is sitting.
    pub table_id: DiningTableId,
    /// Whether they are still there.
    pub status: VisitStatus,
    /// How many people, when the waiter recorded it.
    pub guest_count: Option<i16>,
    /// Who seated them.
    pub opened_by_staff_id: StaffId,
    /// The waiter who hears the ready chime for this table. Whoever opened it,
    /// until a colleague takes it over.
    pub responsible_staff_id: StaffId,
    /// When they sat down.
    pub opened_at: DateTime<Utc>,
    /// When they left, if they have.
    pub closed_at: Option<DateTime<Utc>>,
}

/// One ticket sent to the kitchen.
///
/// A meal is a sequence of these: starters, then mains, then whatever somebody
/// remembered they wanted. Each goes to the kitchen on its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderRound {
    /// Which ticket this is.
    pub id: OrderRoundId,
    /// Which visit it belongs to.
    pub visit_id: VisitId,
    /// Its number within the visit, starting at one.
    pub sequence_no: i32,
    /// Where the whole ticket has got to. Never set directly; see
    /// [`round_status_from_lines`].
    pub status: RoundStatus,
    /// Who sent it.
    pub sent_by_staff_id: StaffId,
    /// When it reached the kitchen. The kitchen queue is ordered by this.
    pub sent_at: DateTime<Utc>,
    /// When the last dish came off the pass.
    pub ready_at: Option<DateTime<Utc>>,
    /// When the last dish reached the table.
    pub served_at: Option<DateTime<Utc>>,
    /// The key the phone made for the send that created it, so a retried
    /// send finds this ticket instead of making a second one.
    pub client_key: Option<uuid::Uuid>,
}

/// One dish on one ticket.
///
/// The price and the dish name are copied here when the round is sent and never
/// read from the menu again. That is what lets a restaurant reprice a dish, or
/// rename it, or take it off the menu entirely, without changing what a bill
/// from last Tuesday says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderLine {
    /// Which line this is.
    pub id: OrderLineId,
    /// Which ticket it is on.
    pub round_id: OrderRoundId,
    /// Which menu item it came from.
    pub dish_id: DishId,
    /// Which bill it has been assigned to, if any. A line with no bill is one
    /// nobody has decided who is paying for yet.
    pub bill_id: Option<BillId>,
    /// How many.
    pub quantity: i32,
    /// What one of them cost when the round was sent.
    pub unit_price: Decimal,
    /// What the dish was called when the round was sent.
    pub dish_name: String,
    /// `quantity` times `unit_price`, held as a column so a bill never has to
    /// trust that every reader multiplies the same way.
    pub line_total: Decimal,
    /// What the guest asked for, such as no onions. Reaches the kitchen ticket
    /// unchanged.
    pub note: Option<String>,
    /// Where this one dish has got to.
    pub status: LineStatus,
    /// Which chef marked it ready.
    pub ready_by_staff_id: Option<StaffId>,
    /// When it came off the pass.
    pub ready_at: Option<DateTime<Utc>>,
    /// When it reached the table.
    pub served_at: Option<DateTime<Utc>>,
    /// Which staff member cancelled it.
    pub voided_by_staff_id: Option<StaffId>,
    /// When it was cancelled.
    pub voided_at: Option<DateTime<Utc>>,
    /// Why it was cancelled, from the closed list. Always present on a voided
    /// line.
    pub void_reason_code: Option<VoidReason>,
    /// The waiter's own words about why. Always present when the code is
    /// [`VoidReason::Other`], optional otherwise.
    pub void_reason: Option<String>,
}

/// What one dish being added to a ticket asks for.
///
/// The price and the name are absent on purpose: the caller does not get to
/// choose them. They are read from the dish and copied when the round is sent,
/// so a client cannot name its own price.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewOrderLine {
    /// Which menu item.
    pub dish_id: DishId,
    /// How many.
    pub quantity: i32,
    /// What the guest asked for, such as no onions.
    pub note: Option<String>,
}

/// One table on the floor, and whoever is sitting at it.
///
/// A read model rather than an entity: it is the answer to one question a
/// waiter's screen asks, assembled from three tables. Kept in the domain
/// because what a floor is made of is a product fact, not a database detail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FloorTable {
    /// The table itself.
    pub table: DiningTable,
    /// Who is at it, or [`None`] when it is free.
    pub occupancy: Option<TableOccupancy>,
}

/// What a waiter needs to know about an occupied table without opening it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableOccupancy {
    /// The one open visit on this table, which the partial unique index
    /// guarantees is at most one.
    pub visit_id: VisitId,
    /// What to call the waiter who seated them.
    pub opened_by: String,
    /// The waiter who hears the ready chime for this table. Responsibility,
    /// not permission: any waiter may act on any table.
    pub responsible_staff_id: StaffId,
    /// What to call that waiter.
    pub responsible_name: String,
    /// When they sat down.
    pub opened_at: DateTime<Utc>,
    /// How many of them, when the waiter recorded it.
    pub guest_count: Option<i16>,
    /// How many dishes on this visit are waiting to be carried out. This is
    /// what puts a table in front of a waiter who is not watching the alert.
    pub ready_dish_count: i64,
}

/// One open visit as the waiter's Orders list reads it.
///
/// A read model, like [`FloorTable`]: every open table with every round on it,
/// so a waiter can see where to walk next without opening each table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenOrder {
    /// The visit.
    pub visit_id: VisitId,
    /// Where the party is sitting.
    pub table_id: DiningTableId,
    /// What the staff call that table.
    pub table_label: String,
    /// When they sat down.
    pub opened_at: DateTime<Utc>,
    /// The waiter who hears the ready chime for this table.
    pub responsible_staff_id: StaffId,
    /// What to call that waiter.
    pub responsible_name: String,
    /// Every ticket on the visit, oldest first, each with its dishes.
    pub rounds: Vec<(OrderRound, Vec<OrderLine>)>,
}

/// One ticket as the kitchen screen reads it.
///
/// The table label rides along because a chef needs to know where the food is
/// going and has no other way to find out: a ticket names a visit, and a visit
/// names a table, neither of which means anything on a kitchen screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KitchenTicket {
    /// The ticket.
    pub round: OrderRound,
    /// Where the food is going.
    pub table_label: String,
    /// Every dish on it, oldest first.
    pub lines: Vec<OrderLine>,
}

/// The most characters a dish note may hold, counted as Unicode code points the
/// way Postgres `char_length` counts them. The database holds the same ceiling.
pub const NOTE_MAX_CHARS: usize = 140;

/// The most characters a void explanation may hold, counted the same way.
pub const VOID_REASON_MAX_CHARS: usize = 200;

/// Tidies a dish note the way it is stored: surrounding spaces trimmed, and a
/// note that is blank after trimming stored as none.
///
/// # Errors
///
/// Returns [`FieldError::TooLong`] if what is left is longer than
/// [`NOTE_MAX_CHARS`].
pub fn normalize_note(note: Option<&str>) -> Result<Option<String>, FieldError> {
    let Some(trimmed) = note.map(str::trim).filter(|text| !text.is_empty()) else {
        return Ok(None);
    };

    if trimmed.chars().count() > NOTE_MAX_CHARS {
        return Err(FieldError::TooLong);
    }

    Ok(Some(trimmed.to_owned()))
}

/// Tidies the words a waiter gave for cancelling a dish, and checks them
/// against the reason code they chose.
///
/// Text is required for [`VoidReason::Other`], because "other" alone tells a
/// manager reading the log nothing, and optional for the three that already say
/// what happened.
///
/// # Errors
///
/// Returns [`FieldError::Required`] if the code is `other` and no text was
/// given, and [`FieldError::TooLong`] if the text is longer than
/// [`VOID_REASON_MAX_CHARS`].
pub fn void_reason_text(
    code: VoidReason,
    text: Option<&str>,
) -> Result<Option<String>, FieldError> {
    let trimmed = text.map(str::trim).filter(|text| !text.is_empty());

    match trimmed {
        None if code == VoidReason::Other => Err(FieldError::Required),
        None => Ok(None),
        Some(words) if words.chars().count() > VOID_REASON_MAX_CHARS => Err(FieldError::TooLong),
        Some(words) => Ok(Some(words.to_owned())),
    }
}

/// Works out where a whole ticket has got to from the dishes on it.
///
/// The ticket's status is stored, for the kitchen queue's sake, but it is never
/// decided: it is exactly this function of its lines, recomputed inside the same
/// transaction as every line write.
///
/// The order of the tests is load bearing, and the all voided case comes first
/// for a reason. A ticket whose dishes were every one cancelled satisfies "no
/// line is still queued" perfectly well, so checking readiness first would call
/// a cancelled ticket ready and put it in front of a waiter to collect.
#[must_use]
pub fn round_status_from_lines(lines: &[LineStatus]) -> RoundStatus {
    // A ticket with no lines cannot exist in practice, because a ticket is
    // created by sending at least one dish and a line is never deleted. Answered
    // explicitly anyway, because the alternative is that "every line is voided"
    // holds vacuously and an empty ticket reads as cancelled.
    if lines.is_empty() {
        return RoundStatus::Queued;
    }

    if lines.iter().all(|line| *line == LineStatus::Voided) {
        return RoundStatus::Voided;
    }

    let counts = lines.iter().filter(|line| **line != LineStatus::Voided);

    if counts.clone().all(|line| *line == LineStatus::Served) {
        return RoundStatus::Served;
    }

    if counts.clone().any(|line| *line == LineStatus::Queued) {
        return RoundStatus::Queued;
    }

    RoundStatus::Ready
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_ticket_whose_dishes_are_all_cancelled_is_cancelled_and_never_ready() {
        // The case the ordering exists for. Every non voided line is trivially
        // not queued here, so a readiness first implementation would announce a
        // cancelled ticket to the waiter.
        assert_eq!(
            round_status_from_lines(&[LineStatus::Voided, LineStatus::Voided]),
            RoundStatus::Voided
        );
    }

    #[test]
    fn one_dish_still_on_the_pass_keeps_the_whole_ticket_queued() {
        assert_eq!(
            round_status_from_lines(&[LineStatus::Ready, LineStatus::Queued]),
            RoundStatus::Queued
        );
    }

    #[test]
    fn a_ticket_is_ready_when_nothing_that_counts_is_still_queued() {
        assert_eq!(
            round_status_from_lines(&[LineStatus::Ready, LineStatus::Ready]),
            RoundStatus::Ready
        );
        assert_eq!(
            round_status_from_lines(&[LineStatus::Ready, LineStatus::Voided]),
            RoundStatus::Ready
        );
        // A dish already carried out does not hold the ticket back either.
        assert_eq!(
            round_status_from_lines(&[LineStatus::Ready, LineStatus::Served]),
            RoundStatus::Ready
        );
    }

    #[test]
    fn a_ticket_is_served_once_every_dish_that_counts_has_reached_the_table() {
        assert_eq!(
            round_status_from_lines(&[LineStatus::Served, LineStatus::Served]),
            RoundStatus::Served
        );
        assert_eq!(
            round_status_from_lines(&[LineStatus::Served, LineStatus::Voided]),
            RoundStatus::Served
        );
    }

    #[test]
    fn a_ticket_with_no_dishes_is_queued_rather_than_cancelled() {
        assert_eq!(round_status_from_lines(&[]), RoundStatus::Queued);
    }

    /// covers: AC-6 (spec 0011)
    #[test]
    fn a_note_is_trimmed_and_a_blank_one_is_none() {
        assert_eq!(
            normalize_note(Some("  no onions  ")),
            Ok(Some("no onions".to_owned()))
        );
        assert_eq!(normalize_note(Some("   ")), Ok(None));
        assert_eq!(normalize_note(None), Ok(None));
    }

    /// covers: AC-6 (spec 0011)
    ///
    /// Counted in code points, not bytes: 140 Devanagari letters are three
    /// bytes each and must still fit.
    #[test]
    fn a_note_is_measured_in_characters_not_bytes() {
        let hindi = "प".repeat(NOTE_MAX_CHARS);
        assert_eq!(normalize_note(Some(&hindi)), Ok(Some(hindi.clone())));

        let too_long = "प".repeat(NOTE_MAX_CHARS + 1);
        assert_eq!(normalize_note(Some(&too_long)), Err(FieldError::TooLong));
    }

    /// covers: AC-13 (spec 0011)
    #[test]
    fn other_needs_words_and_the_rest_do_not() {
        assert_eq!(
            void_reason_text(VoidReason::Other, None),
            Err(FieldError::Required)
        );
        assert_eq!(
            void_reason_text(VoidReason::Other, Some("  ")),
            Err(FieldError::Required)
        );
        assert_eq!(
            void_reason_text(VoidReason::Other, Some(" spilled ")),
            Ok(Some("spilled".to_owned()))
        );
        assert_eq!(
            void_reason_text(VoidReason::GuestChangedMind, None),
            Ok(None)
        );
        assert_eq!(
            void_reason_text(
                VoidReason::KitchenUnavailable,
                Some("x".repeat(201).as_str())
            ),
            Err(FieldError::TooLong)
        );
    }
}
