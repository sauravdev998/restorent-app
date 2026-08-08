//! What travels down a live stream to a kitchen or waiter screen.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::ids::RestaurantId;

/// Which kind of thing changed.
///
/// Kept as a closed enum on purpose: a screen reacting to an event it does not
/// understand is a bug, and a typo in a string would be silent.
///
/// These variants are the closed vocabulary the live pipeline runs on, and they
/// match the entity strings in spec 0003's interface surface one for one.
/// Nothing invents a string: adding an entity means adding a variant here and a
/// row there, in the same change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum EntityKind {
    /// A party sat down, moved table, or left.
    Visit,
    /// A ticket reached the kitchen or changed how far along it is.
    OrderRound,
    /// One dish on a ticket changed.
    OrderLine,
    /// A bill was opened, had lines assigned, closed, or was paid.
    Bill,
    /// A menu item changed, most often its availability.
    Dish,
    /// A table was added, renamed, or archived.
    DiningTable,
    /// A staff account changed.
    Staff,

    /// Carries no product meaning. It exists so the scaffold can prove the
    /// whole path (HTTP, then Postgres NOTIFY, then this process, then the
    /// browser) without needing a real entity, and the development only
    /// `/api/dev/notify` endpoint still uses it.
    Probe,
}

impl EntityKind {
    /// The exact string that travels in a `NOTIFY` payload for this kind.
    ///
    /// The database side of this pairing is `notify_entity_change`, which takes
    /// the string as text. This method is the only place that string is written,
    /// so a caller cannot pass one the listener does not recognise.
    #[must_use]
    pub const fn as_label(self) -> &'static str {
        match self {
            Self::Visit => "visit",
            Self::OrderRound => "order_round",
            Self::OrderLine => "order_line",
            Self::Bill => "bill",
            Self::Dish => "dish",
            Self::DiningTable => "dining_table",
            Self::Staff => "staff",
            Self::Probe => "probe",
        }
    }
}

/// A change worth telling connected screens about.
///
/// It carries a kind and an id, never row content. A client that receives one
/// goes and asks for the row, and row level security decides whether it may
/// have it. Putting the row in the event would make this stream a second,
/// unguarded way to read data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DomainEvent {
    /// Whose event this is. The server routes on this and never sends an event
    /// to a stream belonging to a different restaurant.
    pub restaurant_id: RestaurantId,
    /// What kind of thing changed.
    pub entity: EntityKind,
    /// Which one changed.
    pub entity_id: Uuid,
}
