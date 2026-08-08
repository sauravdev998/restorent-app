//! Identifiers used across the domain.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Which restaurant a piece of data belongs to.
///
/// This is the single most load bearing value in the system. It scopes every
/// query, every row level security policy, and every event stream. It is a
/// newtype rather than a bare [`Uuid`] so it cannot be swapped by accident with
/// some other identifier at a call site.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RestaurantId(Uuid);

impl RestaurantId {
    /// Wraps a raw UUID that is already known to identify a restaurant.
    #[must_use]
    pub const fn from_uuid(id: Uuid) -> Self {
        Self(id)
    }

    /// The underlying UUID, for handing to the database or to a wire format.
    #[must_use]
    pub const fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl fmt::Display for RestaurantId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for RestaurantId {
    type Err = uuid::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Uuid::from_str(value).map(Self)
    }
}
