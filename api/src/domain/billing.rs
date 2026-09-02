//! Bills, the tax lines copied onto them, and the record that they were paid.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

use super::enums::{BillStatus, PaymentMethod};
use super::ids::{BillId, BillTaxId, PaymentId, StaffId, VisitId};
use super::money::Currency;

/// A payment document drawn from a visit's lines.
///
/// Once closed it is a self contained record: the currency, the dish names, the
/// prices, the tax rates, and the service charge are all copied onto it, so a
/// reprint a year later shows exactly what the customer paid rather than what
/// the menu says today.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bill {
    /// Which bill this is.
    pub id: BillId,
    /// Which visit it draws its lines from.
    pub visit_id: VisitId,
    /// Its number within the restaurant. Allocated at close, never before, so an
    /// abandoned bill consumes none.
    pub number: Option<i64>,
    /// Whether it is still collecting lines.
    pub status: BillStatus,
    /// What currency it is in, copied from the restaurant.
    pub currency: Currency,
    /// The sum of its assigned lines that were not voided.
    pub subtotal: Decimal,
    /// The service charge rate applied, copied from the restaurant at close.
    /// [`None`] means the restaurant charges none.
    pub service_charge_percent: Option<Decimal>,
    /// What that rate came to. Zero when there is no service charge, never null.
    pub service_charge_amount: Decimal,
    /// The sum of the tax lines below.
    pub tax_total: Decimal,
    /// Subtotal plus service charge plus taxes, with no residue.
    pub total: Decimal,
    /// Who opened it.
    pub opened_by_staff_id: StaffId,
    /// Who closed it.
    pub closed_by_staff_id: Option<StaffId>,
    /// When it closed.
    pub closed_at: Option<DateTime<Utc>>,
}

/// One tax line copied onto a bill when it closed.
///
/// The name and the rate are copies, not references. Editing the restaurant's
/// tax rules afterwards cannot reach back and change what a closed bill charged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BillTax {
    /// Which tax line this is.
    pub id: BillTaxId,
    /// Which bill it belongs to.
    pub bill_id: BillId,
    /// What the tax was called at the time.
    pub name: String,
    /// What rate was charged.
    pub rate_percent: Decimal,
    /// What that came to, already rounded to the bill's currency.
    pub amount: Decimal,
}

/// A record that a closed bill was paid.
///
/// The product takes no payment itself, so this says what happened at the till
/// or the card machine and nothing more. A method and an amount: no card data,
/// no provider, nothing in payment card industry scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Payment {
    /// Which payment this is.
    pub id: PaymentId,
    /// Which bill it settles.
    pub bill_id: BillId,
    /// How it was taken.
    pub method: PaymentMethod,
    /// How much.
    pub amount: Decimal,
    /// Which staff member took it.
    pub taken_by_staff_id: StaffId,
    /// When.
    pub taken_at: DateTime<Utc>,
    /// Anything worth writing down, such as a card machine reference.
    pub note: Option<String>,
}
