//! The one Postgres listen connection this instance holds.
//!
//! One per process, not one per connected screen. A connection per screen would
//! exhaust Postgres by about the second restaurant. It is taken from outside
//! the main pool and held for the life of the process, because a pooled
//! connection cannot be parked on LISTEN.

use std::convert::Infallible;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use sqlx::postgres::{PgListener, PgPoolOptions};

use crate::domain::event::DomainEvent;

use super::registry::EventRegistry;

/// The channel every instance listens on.
const CHANNEL: &str = "entity_changed";

/// How long to wait before trying to reconnect, and the ceiling on that wait.
const RECONNECT_DELAY: Duration = Duration::from_secs(1);
const RECONNECT_DELAY_MAX: Duration = Duration::from_secs(30);

/// How long one attempt to open the listen connection may take.
///
/// This bounds the attempt itself, which is a different thing from [`Backoff`],
/// which is only the wait between attempts. The two are easy to confuse, and
/// confusing them hid a bug: SQLx's pool does not fail a refused connection
/// straight away, it retries internally until its acquire timeout runs out, and
/// that timeout defaults to 30 seconds. So an unbounded attempt cost 30 seconds
/// no matter what the backoff ladder said. Measured against a database that was
/// down, an attempt took 30.0 seconds every time, which is why the first
/// `/check verify` run saw retries 31 seconds apart while the delay itself
/// reported 1 second and then 2.
///
/// Comfortably under [`RECONNECT_DELAY_MAX`] so the ladder stays the thing that
/// decides how often this instance tries, rather than being swamped by a floor
/// underneath it.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// How long to wait between reconnect attempts, and how that wait grows.
///
/// A type of its own so the reset is something a test can watch. Left as a bare
/// `Duration` threaded through the loop, the reset sat in a branch that never
/// ran and nothing noticed for as long as the code existed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Backoff {
    delay: Duration,
}

impl Backoff {
    /// A backoff at its shortest wait.
    const fn new() -> Self {
        Self {
            delay: RECONNECT_DELAY,
        }
    }

    /// How long to wait before the next attempt.
    const fn delay(self) -> Duration {
        self.delay
    }

    /// Doubles the wait, up to the ceiling. Called after each failed attempt.
    fn worsen(&mut self) {
        self.delay = (self.delay * 2).min(RECONNECT_DELAY_MAX);
    }

    /// Back to the shortest wait. Called when a connection is established.
    ///
    /// The trigger has to be "connected", not "the connect function returned
    /// success", because it never returns success.
    fn reset(&mut self) {
        self.delay = RECONNECT_DELAY;
    }
}

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
        let mut backoff = Backoff::new();

        loop {
            // Holding a connection is the only thing that resets the backoff, and
            // `connect_and_listen` does it itself the moment it is connected. It
            // cannot report success here: its return type says it only ever ends
            // by failing, so a reset written on this side would never run.
            match connect_and_listen(&database_url, &registry, &alive, &mut backoff).await {
                Ok(never) => match never {},
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
                retry_in_secs = backoff.delay().as_secs(),
                "reconnecting the postgres listener"
            );
            tokio::time::sleep(backoff.delay()).await;
            backoff.worsen();
        }
    });

    handle
}

/// Opens the listen connection, giving up after [`CONNECT_TIMEOUT`].
///
/// [`PgListener::connect`] is deliberately not used. It builds a pool of its
/// own and leaves SQLx's 30 second default acquire timeout on it, and nothing
/// outside that function can reach the pool to change it. Building the pool
/// here instead puts that bound back under this module's control.
///
/// Handing the listener our pool rather than wrapping the call in a timeout is
/// what makes this cover both ways in. The listener keeps the pool, so the same
/// bound also applies to the reconnect it performs internally when the
/// connection drops mid stream: that path goes through `pool.acquire` too, and
/// a timeout around this function would not have touched it. Left unbounded it
/// is the worse of the two, because for those 30 seconds the health endpoint
/// still reports this listener as alive while it delivers nothing.
///
/// The other options match what [`PgListener::connect`] would have built: a
/// single connection that is never reaped, because it is parked on LISTEN
/// rather than pooled and handed round.
async fn connect_bounded(database_url: &str) -> Result<PgListener, sqlx::Error> {
    // Lazy, so building the pool is not itself an unbounded connect attempt.
    // The bound below only applies once something asks the pool for a
    // connection, which is what `connect_with` does next.
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .max_lifetime(None)
        .idle_timeout(None)
        .acquire_timeout(CONNECT_TIMEOUT)
        .connect_lazy(database_url)?;

    let mut listener = PgListener::connect_with(&pool).await?;

    // What `PgListener::connect` sets on its own pool, kept here so this
    // behaves the same. Nothing ever closes this pool, so the close event is
    // noise the receive loop below should not be waiting on.
    listener.ignore_pool_close_event(true);

    Ok(listener)
}

/// Holds one connection until it breaks.
///
/// Returns only by failing. The success type says so: there is no value of
/// [`Infallible`] to return, and the loop below never leaves except through `?`.
/// That is why resetting the backoff is this function's job rather than the
/// caller's.
///
/// `backoff` is the caller's, reset here the moment the connection is
/// established. The reset has to hang off "we are connected now", because "this
/// function returned success" never happens.
async fn connect_and_listen(
    database_url: &str,
    registry: &EventRegistry,
    alive: &AtomicBool,
    backoff: &mut Backoff,
) -> Result<Infallible, sqlx::Error> {
    let mut listener = connect_bounded(database_url).await?;
    listener.listen(CHANNEL).await?;

    alive.store(true, Ordering::Release);

    // Connected, so the next outage starts its backoff from the short delay
    // again. Without this a run of failures leaves the delay pinned at its
    // ceiling for the life of the process, and every later blip takes the
    // full 30 seconds to recover from instead of about a second.
    backoff.reset();

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_reconnect_delay_grows_but_stops_at_the_ceiling() {
        let mut backoff = Backoff::new();
        assert_eq!(backoff.delay(), Duration::from_secs(1));

        for expected in [2, 4, 8, 16] {
            backoff.worsen();
            assert_eq!(backoff.delay(), Duration::from_secs(expected));
        }

        // Doubling 16 would overshoot, and a hundred more failures must not
        // push it past the ceiling.
        for _ in 0..100 {
            backoff.worsen();
        }
        assert_eq!(backoff.delay(), RECONNECT_DELAY_MAX);
    }

    /// Guards the bug where an attempt to connect was bounded by nothing this
    /// module owned, so it inherited SQLx's 30 second default and every attempt
    /// cost 30 seconds regardless of what the backoff ladder said. A real run
    /// measured exactly that: `PgListener::connect` against a database that was
    /// down took 30.0 seconds, while the delay reported 1 second and then 2.
    ///
    /// Nothing listens on port 1, so the connection is refused at once and the
    /// only thing left to measure is the giving up. The clock is paused, so the
    /// wait is virtual and proving a five second bound costs no real seconds.
    #[tokio::test(start_paused = true)]
    async fn one_connect_attempt_gives_up_well_before_the_backoff_ceiling() {
        let started = tokio::time::Instant::now();
        let result = connect_bounded("postgres://nobody:nobody@127.0.0.1:1/nothing").await;
        let elapsed = started.elapsed();

        assert!(
            result.is_err(),
            "a database that is not there cannot produce a listener"
        );
        assert!(
            elapsed <= CONNECT_TIMEOUT,
            "an attempt took {elapsed:?}, which is past the {CONNECT_TIMEOUT:?} this module \
             asks for; it is being bounded by something else, which is the original bug"
        );
        assert!(
            CONNECT_TIMEOUT < RECONNECT_DELAY_MAX,
            "an attempt that can outlast the longest wait between attempts makes the backoff \
             ladder meaningless, because the attempt itself becomes the delay"
        );
    }

    /// Guards the bug where the reset lived in a match arm that could never run,
    /// so one bad patch left the delay pinned at 30 seconds for the life of the
    /// process and every later blip took 30 seconds to recover from.
    #[test]
    fn a_connection_puts_the_delay_back_to_the_short_wait() {
        let mut backoff = Backoff::new();

        // A long outage: the delay climbs to its ceiling.
        for _ in 0..10 {
            backoff.worsen();
        }
        assert_eq!(backoff.delay(), RECONNECT_DELAY_MAX);

        // Connected again. The next outage must start from the short wait, not
        // carry the last one's penalty.
        backoff.reset();
        assert_eq!(backoff.delay(), RECONNECT_DELAY);
    }
}
