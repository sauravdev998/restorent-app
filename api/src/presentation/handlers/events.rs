//! `GET /api/events`, the live stream a kitchen or waiter screen holds open.
//!
//! Server sent events rather than a socket, because every push in this product
//! travels one way, server to client. Actions go up as ordinary POSTs. That
//! means plain HTTP, browser native reconnection, no upgrade handshake, and no
//! keepalive protocol of our own to get wrong.

use std::convert::Infallible;
use std::time::Duration;

use axum::extract::State;
use axum::response::Sse;
use axum::response::sse::{Event, KeepAlive};
use futures::Stream;
use serde::Serialize;
use tokio::sync::broadcast::error::RecvError;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::domain::event::DomainEvent;
use crate::presentation::extract::RestaurantScope;
use crate::presentation::state::AppState;

/// How often to send a comment line so nothing between here and the browser
/// decides the connection is idle.
///
/// The Application Load Balancer's idle timeout is raised to 300 seconds in
/// `infra/`, and this sits far enough inside it that a quiet dinner service
/// never drops a kitchen screen.
const HEARTBEAT: Duration = Duration::from_secs(15);

/// The SSE event name every change is published under.
const EVENT_NAME: &str = "entity_changed";

/// What one message on the stream looks like.
///
/// A kind and an id, never row content. The client uses it to refetch or to
/// patch its query cache, which keeps row level security the single authority
/// on who may see what: a client that should not see a row simply gets nothing
/// back when it asks for it.
#[derive(Debug, Serialize, ToSchema)]
pub struct StreamEvent {
    /// What kind of thing changed, for example `probe`.
    #[schema(example = "probe")]
    pub entity: String,
    /// Which one changed.
    pub entity_id: Uuid,
}

impl From<DomainEvent> for StreamEvent {
    fn from(event: DomainEvent) -> Self {
        // The restaurant id is deliberately dropped here. The server already
        // routed on it, and the client has no use for it.
        Self {
            entity: serde_json::to_value(event.entity)
                .ok()
                .and_then(|value| value.as_str().map(str::to_owned))
                .unwrap_or_else(|| "unknown".to_owned()),
            entity_id: event.entity_id,
        }
    }
}

/// Opens the live stream for the caller's restaurant.
///
/// The stream carries only this restaurant's events, because the server routes
/// on the restaurant id before it ever writes to a socket. Sending everything
/// and letting the browser filter would leak ticket volume, timing, and
/// identifiers between restaurants over the wire, even though the follow up
/// fetch would be refused.
///
/// The client must refetch its active queries every time this opens, including
/// every reconnect. Postgres queues nothing for a listener that is not
/// connected, so anything sent during a gap is gone, and without a refetch one
/// network blip becomes a ticket that never appears in the kitchen.
#[utoipa::path(
    get,
    path = "/api/events",
    tag = "system",
    responses(
        (
            status = 200,
            description = "A server sent event stream. Each message is named `entity_changed` and its data is a StreamEvent.",
            content_type = "text/event-stream",
            body = StreamEvent,
        ),
        (status = 401, description = "No session, so no restaurant to stream.", body = crate::presentation::error::ErrorBody),
    )
)]
pub async fn events(
    State(state): State<AppState>,
    scope: RestaurantScope,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let restaurant_id = scope.restaurant_id();
    let mut receiver = state.events.subscribe(restaurant_id).await;

    tracing::info!(restaurant_id = %restaurant_id, "event stream opened");

    let stream = async_stream::stream! {
        // Say something the instant the stream is subscribed, before waiting on
        // anything. A quiet stream otherwise produces no body byte until the
        // first heartbeat 15 seconds later, and an intermediary that holds the
        // response headers back until the first byte of body (the Vite dev proxy
        // does exactly this, and CloudFront is entitled to) delays the browser's
        // `onopen` by that much.
        //
        // That delay is not only a stale badge. The client refetches its active
        // queries from `onopen`, so a late open is a window in which the screen
        // is live but has not resynchronised, which is the one gap this stream
        // exists to close.
        yield Ok(Event::default().comment("open"));

        loop {
            match receiver.recv().await {
                Ok(event) => {
                    let payload = StreamEvent::from(event);

                    match Event::default().event(EVENT_NAME).json_data(&payload) {
                        Ok(sse_event) => yield Ok(sse_event),
                        Err(error) => {
                            tracing::error!(error = %error, "could not serialise a stream event");
                        }
                    }
                }
                Err(RecvError::Lagged(missed)) => {
                    // This screen fell too far behind to be sure of its state.
                    // Telling it to resynchronise is the only honest move.
                    tracing::warn!(
                        restaurant_id = %restaurant_id,
                        missed,
                        "a screen lagged behind; asking it to resynchronise"
                    );
                    if let Ok(sse_event) = Event::default().event("resync").json_data(()) {
                        yield Ok(sse_event);
                    }
                }
                Err(RecvError::Closed) => {
                    // The listener died, so this instance is delivering
                    // nothing. Ending the stream makes the browser reconnect
                    // and refetch, which beats a screen that looks fine.
                    tracing::warn!(restaurant_id = %restaurant_id, "event stream closed");
                    break;
                }
            }
        }
    };

    Sse::new(stream).keep_alive(KeepAlive::new().interval(HEARTBEAT))
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::domain::event::EntityKind;
    use crate::domain::ids::RestaurantId;

    fn probe_event(restaurant: u128, entity: u128) -> DomainEvent {
        DomainEvent {
            restaurant_id: RestaurantId::from_uuid(Uuid::from_u128(restaurant)),
            entity: EntityKind::Probe,
            entity_id: Uuid::from_u128(entity),
        }
    }

    #[test]
    fn a_message_carries_the_kind_and_the_id_of_what_changed() {
        let payload = StreamEvent::from(probe_event(1, 7));

        assert_eq!(payload.entity, "probe");
        assert_eq!(payload.entity_id, Uuid::from_u128(7));
    }

    /// The restaurant id is routed on by the server and then deliberately
    /// dropped. This checks the wire shape rather than the struct, because the
    /// struct having no field is only half of it: what matters is that the id
    /// appears nowhere in the bytes a browser receives.
    ///
    /// Sending it would leak which restaurant a screen belongs to onto the
    /// wire, and it would do so even though the follow up fetch would be
    /// refused, which is the whole reason events carry a kind and an id rather
    /// than row content.
    #[test]
    fn a_message_never_names_the_restaurant_it_belongs_to() {
        let restaurant = Uuid::from_u128(0xabc_def);
        let payload = StreamEvent::from(probe_event(0xabc_def, 7));

        let json = serde_json::to_string(&payload).expect("a stream event serialises");

        assert!(
            !json.contains(&restaurant.to_string()),
            "the restaurant id reached the browser in {json}, which leaks tenancy over the wire"
        );
        assert!(
            !json.contains("restaurant"),
            "no field naming a restaurant belongs on the wire, found in {json}"
        );
    }

    /// A kind the client cannot read is reported as `unknown` rather than
    /// dropping the message, because the id is still worth acting on: the
    /// client refetches and row level security stays the authority on what it
    /// may see.
    #[test]
    fn a_message_keeps_its_id_even_when_the_kind_means_nothing_to_the_client() {
        let payload = StreamEvent::from(probe_event(1, 9));

        assert_eq!(payload.entity_id, Uuid::from_u128(9));
        assert!(
            !payload.entity.is_empty(),
            "an empty kind tells the client nothing to invalidate"
        );
    }

    /// Pins the heartbeat against the two things between this stream and a
    /// kitchen screen that hang up on a quiet connection.
    ///
    /// Read what this does and does not prove. It pins the relationship only,
    /// against copies of the two numbers, because those live in
    /// `infra/lib/platform-stack.ts` (`CloudFront` `readTimeout`, 60 seconds, and
    /// the load balancer `idleTimeout`, 300 seconds) and this crate cannot
    /// import from the CDK stack. Change either number there and this test
    /// stays green. It is still worth having: it catches someone raising
    /// `HEARTBEAT` here, which is the far likelier edit.
    ///
    /// The margin matters more than the ordering. `CloudFront` is the tighter of
    /// the two, so a heartbeat that merely fits inside it once would drop a
    /// kitchen screen on a single delayed comment.
    #[test]
    // Seconds, not minutes, and clippy is overruled on purpose. These two are
    // copies of `Duration.seconds(60)` and `Duration.seconds(300)` in
    // `infra/lib/platform-stack.ts`. Written as `from_mins` they stop looking
    // like the lines they mirror, which is the only thing holding the two
    // files together.
    #[allow(clippy::duration_suboptimal_units)]
    fn a_quiet_stream_heartbeats_well_inside_what_would_hang_up_on_it() {
        const CLOUDFRONT_READ_TIMEOUT: Duration = Duration::from_secs(60);
        const LOAD_BALANCER_IDLE_TIMEOUT: Duration = Duration::from_secs(300);

        assert!(
            HEARTBEAT * 3 <= CLOUDFRONT_READ_TIMEOUT,
            "a stream must survive a couple of missed heartbeats; {HEARTBEAT:?} leaves no room \
             inside CloudFront's {CLOUDFRONT_READ_TIMEOUT:?} read timeout"
        );
        assert!(
            HEARTBEAT < LOAD_BALANCER_IDLE_TIMEOUT,
            "the load balancer closes an idle connection before the next heartbeat would arrive"
        );
    }
}
