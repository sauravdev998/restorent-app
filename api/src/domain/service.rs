//! The service loop: a party at a table, the tickets they send, and the dishes
//! on them.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

use super::enums::{LineStatus, RoundStatus, VisitStatus};
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
    /// Why it was cancelled. Always present on a voided line.
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
}
