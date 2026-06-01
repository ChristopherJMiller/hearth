//! SSE endpoint for the per-machine push fast-path (RFC-001).
//!
//! `GET /api/v1/machines/{id}/events`
//!
//! Requires a machine token whose `machine_id` claim matches the path
//! `{id}`. On accept, holds the HTTP connection open and emits one
//! newline-delimited JSON event per state change, plus a `:keepalive`
//! comment every 30s so intermediate proxies (Caddy / oauth2-proxy /
//! NGINX) don't reap an idle connection.
//!
//! Falling back cleanly to polling is the agent's job; this endpoint
//! makes no promises about delivery — see RFC §"Cadence and
//! back-pressure".

use std::convert::Infallible;
use std::time::Duration;

use axum::extract::{Path, State};
use axum::response::sse::{Event, KeepAlive, Sse};
use futures::Stream;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::BroadcastStream;
use tracing::{debug, warn};
use uuid::Uuid;

use crate::AppState;
use crate::auth::{AuthError, MachineIdentity};
use crate::events::MachineEvent;

pub async fn machine_events_stream(
    MachineIdentity(token_machine_id): MachineIdentity,
    State(state): State<AppState>,
    Path(path_machine_id): Path<Uuid>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, AuthError> {
    // In dev mode (auth disabled) MachineIdentity returns Uuid::nil();
    // accept whatever machine_id is in the path. In prod, require the
    // path id to match the token's claim — otherwise any agent could
    // tap any other agent's event stream.
    if token_machine_id != Uuid::nil() && token_machine_id != path_machine_id {
        warn!(
            token = %token_machine_id,
            path = %path_machine_id,
            "SSE subscribe rejected: machine_id mismatch"
        );
        return Err(AuthError(
            axum::http::StatusCode::FORBIDDEN,
            "machine_id in path does not match machine token".into(),
        ));
    }

    let rx = state.event_bus.subscribe(path_machine_id);
    debug!(machine_id = %path_machine_id, "SSE subscriber attached");

    let stream = BroadcastStream::new(rx).filter_map(|res| match res {
        Ok(event) => Some(Ok(format_event(&event))),
        Err(tokio_stream::wrappers::errors::BroadcastStreamRecvError::Lagged(skipped)) => {
            // We lagged the broadcast — synthesize a single state_changed
            // so the agent re-polls anyway. Same correctness as catching
            // up to every individual event for the current event set.
            warn!(skipped, "SSE broadcast lagged, coalescing to state_changed");
            Some(Ok(format_event(&MachineEvent::StateChanged)))
        }
    });

    // 30s keepalive — Caddy's default idle timeout is 5 minutes and
    // most reverse proxies sit somewhere in 60-300s. 30s leaves
    // generous headroom under all of them and matches the RFC.
    let keepalive = KeepAlive::new()
        .interval(Duration::from_secs(30))
        .text("keepalive");

    Ok(Sse::new(stream).keep_alive(keepalive))
}

fn format_event(event: &MachineEvent) -> Event {
    // `Event::json_data` cannot fail for serde-derived enums; default
    // to `data: {}` if it ever does so the stream stays alive.
    Event::default()
        .json_data(event)
        .unwrap_or_else(|_| Event::default().data("{}"))
}
