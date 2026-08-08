//! Which open streams belong to which restaurant.
//!
//! Every instance keeps its own copy of this. A notification arrives on the one
//! global Postgres channel, every instance receives it, and each one forwards it
//! only to the streams here that belong to the restaurant named in the payload.
//!
//! The alternative, broadcasting every event to every open stream and letting
//! the browser filter, is the obvious shortcut and it is wrong. It leaks ticket
//! volume, timing, and identifiers across restaurants over the wire, and it
//! leaks them even though the follow up fetch would be refused.

use std::collections::HashMap;

use tokio::sync::{RwLock, broadcast};

use crate::domain::event::DomainEvent;
use crate::domain::ids::RestaurantId;

/// How many events a slow screen may fall behind before it is told it lagged.
const CHANNEL_CAPACITY: usize = 256;

/// The open streams on this instance, grouped by restaurant.
#[derive(Debug, Default)]
pub struct EventRegistry {
    channels: RwLock<HashMap<RestaurantId, broadcast::Sender<DomainEvent>>>,
}

impl EventRegistry {
    /// An empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Opens a stream for one restaurant.
    ///
    /// Screens for the same restaurant share one channel, so a busy kitchen
    /// with six screens still costs one entry here.
    pub async fn subscribe(&self, restaurant_id: RestaurantId) -> broadcast::Receiver<DomainEvent> {
        let mut channels = self.channels.write().await;

        // Drop channels nobody is listening to any more, so a long lived
        // process does not accumulate one entry per restaurant it has ever
        // served.
        channels.retain(|_, sender| sender.receiver_count() > 0);

        channels
            .entry(restaurant_id)
            .or_insert_with(|| broadcast::channel(CHANNEL_CAPACITY).0)
            .subscribe()
    }

    /// Forwards an event to the streams belonging to its restaurant, and to no
    /// others.
    pub async fn publish(&self, event: DomainEvent) {
        let channels = self.channels.read().await;

        if let Some(sender) = channels.get(&event.restaurant_id) {
            // An error here only means the last screen closed between the
            // lookup and the send. Nothing to do about that.
            let _ = sender.send(event);
        }
    }

    /// Ends every open stream on this instance.
    ///
    /// Called when the listen connection drops. A process that is no longer
    /// receiving notifications is no longer delivering tickets, and a screen
    /// that looks connected while receiving nothing is far worse than a screen
    /// that reconnects: on reconnect the client refetches and catches up.
    pub async fn close_all(&self) {
        let mut channels = self.channels.write().await;
        let closed = channels.len();
        channels.clear();

        if closed > 0 {
            tracing::warn!(restaurants = closed, "closed every open event stream");
        }
    }
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;
    use crate::domain::event::EntityKind;

    fn event_for(restaurant_id: RestaurantId) -> DomainEvent {
        DomainEvent {
            restaurant_id,
            entity: EntityKind::Probe,
            entity_id: Uuid::nil(),
        }
    }

    #[tokio::test]
    async fn an_event_never_reaches_another_restaurants_stream() {
        let registry = EventRegistry::new();
        let ours = RestaurantId::from_uuid(Uuid::from_u128(1));
        let theirs = RestaurantId::from_uuid(Uuid::from_u128(2));

        let mut our_stream = registry.subscribe(ours).await;
        let mut their_stream = registry.subscribe(theirs).await;

        registry.publish(event_for(ours)).await;

        assert_eq!(our_stream.recv().await.unwrap(), event_for(ours));
        assert!(
            their_stream.try_recv().is_err(),
            "an event for one restaurant reached another restaurant's stream"
        );
    }

    #[tokio::test]
    async fn closing_all_streams_ends_them() {
        let registry = EventRegistry::new();
        let restaurant = RestaurantId::from_uuid(Uuid::from_u128(1));
        let mut stream = registry.subscribe(restaurant).await;

        registry.close_all().await;

        assert!(
            matches!(
                stream.recv().await,
                Err(broadcast::error::RecvError::Closed)
            ),
            "the stream should have ended so the client reconnects and resynchronises"
        );
    }
}
