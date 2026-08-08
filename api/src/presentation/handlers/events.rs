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
