//! The one Postgres listen connection this instance holds.
//!
//! One per process, not one per connected screen. A connection per screen would
//! exhaust Postgres by about the second restaurant. It is taken from outside
//! the main pool and held for the life of the process, because a pooled
//! connection cannot be parked on LISTEN.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use sqlx::postgres::PgListener;

use crate::domain::event::DomainEvent;

use super::registry::EventRegistry;

/// The channel every instance listens on.
const CHANNEL: &str = "entity_changed";

/// How long to wait before trying to reconnect, and the ceiling on that wait.
const RECONNECT_DELAY: Duration = Duration::from_secs(1);
const RECONNECT_DELAY_MAX: Duration = Duration::from_secs(30);

/// A handle to the background listen task.
///
/// Cloneable and cheap: it is just a shared flag. The health endpoint reads it
/// on every check, so it must never block.
#[derive(Debug, Clone)]
pub struct ListenerHandle {
    alive: Arc<AtomicBool>,
}

impl ListenerHandle {
    /// Whether the listen connection is currently up.
    ///
    /// False means this instance is still serving HTTP but is delivering
    /// nothing, which is exactly the state the health endpoint must report as
    /// unhealthy so the load balancer replaces the task.
    #[must_use]
    pub fn is_alive(&self) -> bool {
        self.alive.load(Ordering::Acquire)
    }
}

/// Starts the listen loop in the background and returns a handle to its state.
///
/// The task owns its own connection and reconnects on its own. It never returns
/// while the process is running.
#[must_use]
pub fn spawn(database_url: String, registry: Arc<EventRegistry>) -> ListenerHandle {
    let alive = Arc::new(AtomicBool::new(false));
    let handle = ListenerHandle {
        alive: Arc::clone(&alive),
    };

    tokio::spawn(async move {
        let mut delay = RECONNECT_DELAY;

        loop {
            match connect_and_listen(&database_url, &registry, &alive).await {
                Ok(()) => delay = RECONNECT_DELAY,
                Err(error) => {
                    tracing::error!(error = %error, "postgres listener failed");
                }
            }

            // Whatever went wrong, this instance is not delivering anything
            // right now. Say so, and end the streams so clients reconnect and
            // refetch rather than sitting on a screen that quietly stopped.
            alive.store(false, Ordering::Release);
            registry.close_all().await;

            tracing::warn!(
                retry_in_secs = delay.as_secs(),
                "reconnecting the postgres listener"
            );
            tokio::time::sleep(delay).await;
            delay = (delay * 2).min(RECONNECT_DELAY_MAX);
        }
    });

    handle
}

/// Holds one connection until it breaks.
async fn connect_and_listen(
    database_url: &str,
    registry: &EventRegistry,
    alive: &AtomicBool,
) -> Result<(), sqlx::Error> {
    let mut listener = PgListener::connect(database_url).await?;
    listener.listen(CHANNEL).await?;

    alive.store(true, Ordering::Release);
    tracing::info!(channel = CHANNEL, "postgres listener connected");

    loop {
        // try_recv, not recv: it reports a dropped and re established
        // connection as Ok(None) instead of hiding it. That distinction matters,
        // because Postgres queues nothing for a listener that is not connected,
        // so anything sent during the gap is simply gone.
        match listener.try_recv().await? {
            Some(notification) => match serde_json::from_str::<DomainEvent>(notification.payload())
            {
                Ok(event) => {
                    tracing::debug!(
                        restaurant_id = %event.restaurant_id,
                        entity_id = %event.entity_id,
                        "forwarding change to this instance's streams"
                    );
                    registry.publish(event).await;
                }
                Err(error) => {
                    // Never fatal. A payload this process cannot read must not
                    // take down delivery for every screen on the instance.
                    tracing::error!(
                        error = %error,
                        payload = notification.payload(),
                        "ignoring an unreadable notification payload"
                    );
                }
            },
            None => {
                // The connection dropped and came back. Notifications sent in
                // between were never queued, so every screen on this instance
                // may now be stale. End the streams; the client reconnects and
                // refetches its active queries, which is the only way to be
                // sure it has not silently missed a ticket.
                tracing::warn!(
                    "postgres listener reconnected, closing streams so clients resynchronise"
                );
                registry.close_all().await;
            }
        }
    }
}
