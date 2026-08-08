//! What travels down a live stream to a kitchen or waiter screen.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::ids::RestaurantId;

/// Which kind of thing changed.
///
/// Kept as a closed enum on purpose: a screen reacting to an event it does not
/// understand is a bug, and a typo in a string would be silent. Feature 4 adds
/// the real entities as the schema lands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum EntityKind {
    /// Carries no product meaning. It exists so the scaffold can prove the
    /// whole path (HTTP, then Postgres NOTIFY, then this process, then the
    /// browser) before any real entity exists.
    Probe,
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
