//! Per-machine event bus for the SSE push fast-path.
//!
//! Backs the `GET /api/v1/machines/{id}/events` endpoint. Each API replica
//! owns its in-process subscribers; cross-replica fan-out goes through a
//! Postgres `LISTEN/NOTIFY` channel (`hearth_machine_events`) so a write
//! from any replica (or the build worker) reaches every subscriber.
//!
//! Design lives in `docs/rfc-001-push-fast-path.md`. Push is an
//! optimisation, never a correctness primitive — the agent's 60s poll
//! still runs whenever the stream is down.
//!
//! Payload on the wire (Postgres NOTIFY): the machine UUID as a plain
//! string. Keeps us well under the 8000-byte NOTIFY payload limit and
//! avoids JSON parse cost in the listener hot path.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use sqlx::postgres::PgListener;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Postgres channel used for cross-replica fan-out of per-machine events.
pub const NOTIFY_CHANNEL: &str = "hearth_machine_events";

/// Capacity of each per-machine broadcast channel.
///
/// One slot per pending event; if an agent is so slow it can't drain
/// 16 events the receiver lags and skips intermediate events. That's
/// fine — the only event today is "poll now", which coalesces naturally.
const BROADCAST_CAPACITY: usize = 16;

/// Wire event sent to agents over SSE.
///
/// Kept intentionally small for v1. Future extensions (e.g.
/// `target_closure` to skip the `/state` round-trip) tag-on as new
/// variants — agents must ignore unknown variants.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MachineEvent {
    /// Something changed on this machine's target state. Agent should
    /// re-poll `/state`.
    StateChanged,
}

/// In-process per-machine subscriber registry.
///
/// Senders are created lazily on first subscribe and never proactively
/// removed — `broadcast::Sender::send` is a cheap no-op when there are
/// no receivers, and the slot is bounded in size. A periodic pruner
/// could be added if fleet churn ever makes this a problem.
#[derive(Default)]
pub struct EventBus {
    senders: Mutex<HashMap<Uuid, broadcast::Sender<MachineEvent>>>,
}

impl EventBus {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Subscribe to events for `machine_id`. Creates the slot on demand.
    pub fn subscribe(&self, machine_id: Uuid) -> broadcast::Receiver<MachineEvent> {
        let mut senders = self.senders.lock().unwrap();
        let sender = senders
            .entry(machine_id)
            .or_insert_with(|| broadcast::channel(BROADCAST_CAPACITY).0);
        sender.subscribe()
    }

    /// Publish `event` to all subscribers for `machine_id`.
    ///
    /// Returns the number of receivers the event reached. Returns 0
    /// when no local subscriber exists — that's expected when the
    /// agent is offline, or when this replica simply isn't holding
    /// the SSE connection (a sibling replica is).
    pub fn publish(&self, machine_id: Uuid, event: MachineEvent) -> usize {
        let senders = self.senders.lock().unwrap();
        match senders.get(&machine_id) {
            Some(sender) => sender.send(event).unwrap_or(0),
            None => 0,
        }
    }

    /// Number of machines with at least one in-process subscriber.
    /// Exposed for metrics / tests.
    pub fn machine_count(&self) -> usize {
        self.senders.lock().unwrap().len()
    }

    /// Drop slots that no longer have subscribers. Called by the
    /// periodic prune task; safe to call from tests too.
    pub fn prune(&self) {
        let mut senders = self.senders.lock().unwrap();
        senders.retain(|_, s| s.receiver_count() > 0);
    }
}

/// Notify a single machine via Postgres `NOTIFY`. The local replica
/// will hear it back via `LISTEN` and forward to local subscribers
/// (so it's safe to call this from anywhere with a pool — repo
/// functions, route handlers, the build worker).
pub async fn notify_machine(pool: &PgPool, machine_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query("SELECT pg_notify($1, $2)")
        .bind(NOTIFY_CHANNEL)
        .bind(machine_id.to_string())
        .execute(pool)
        .await?;
    Ok(())
}

/// Notify every machine that hosts an environment for the given
/// username. Used by the user-env build completion path so all
/// devices a user is logged into start picking up the new closure
/// without waiting for the next poll.
pub async fn notify_user_machines(pool: &PgPool, username: &str) -> Result<usize, sqlx::Error> {
    let rows: Vec<(Uuid,)> =
        sqlx::query_as("SELECT DISTINCT machine_id FROM user_environments WHERE username = $1")
            .bind(username)
            .fetch_all(pool)
            .await?;

    for (machine_id,) in &rows {
        // Best-effort: a notify failure is logged but does not roll back
        // the caller. The 60s poll will still pick it up.
        if let Err(e) = notify_machine(pool, *machine_id).await {
            warn!(%machine_id, error = %e, "pg_notify failed during user-machine fan-out");
        }
    }
    Ok(rows.len())
}

/// Background task: open a long-lived `LISTEN` on Postgres and forward
/// every notification to the local in-process `EventBus`.
///
/// Reconnects on connection loss with a 1s sleep — `PgListener` already
/// auto-reconnects, so this is just the outer-loop safety net.
pub async fn run_listener(pool: PgPool, bus: Arc<EventBus>, cancel: CancellationToken) {
    info!("starting LISTEN hearth_machine_events");

    loop {
        if cancel.is_cancelled() {
            info!("event listener shutting down");
            return;
        }

        let mut listener = match PgListener::connect_with(&pool).await {
            Ok(l) => l,
            Err(e) => {
                warn!(error = %e, "failed to open PgListener, retrying in 1s");
                tokio::select! {
                    () = cancel.cancelled() => return,
                    () = tokio::time::sleep(Duration::from_secs(1)) => continue,
                }
            }
        };

        if let Err(e) = listener.listen(NOTIFY_CHANNEL).await {
            warn!(channel = NOTIFY_CHANNEL, error = %e, "LISTEN failed, retrying");
            tokio::select! {
                () = cancel.cancelled() => return,
                () = tokio::time::sleep(Duration::from_secs(1)) => continue,
            }
        }

        loop {
            tokio::select! {
                () = cancel.cancelled() => {
                    info!("event listener shutting down");
                    return;
                }
                result = listener.recv() => match result {
                    Ok(notif) => {
                        let payload = notif.payload();
                        match payload.parse::<Uuid>() {
                            Ok(machine_id) => {
                                let delivered = bus.publish(machine_id, MachineEvent::StateChanged);
                                debug!(%machine_id, delivered, "forwarded machine event from pg_notify");
                            }
                            Err(e) => {
                                warn!(payload, error = %e, "malformed pg_notify payload, ignoring");
                            }
                        }
                    }
                    Err(e) => {
                        error!(error = %e, "PgListener recv failed, reconnecting");
                        break;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn subscribe_then_publish_delivers_event() {
        let bus = EventBus::new();
        let machine_id = Uuid::new_v4();

        let mut rx = bus.subscribe(machine_id);
        let delivered = bus.publish(machine_id, MachineEvent::StateChanged);
        assert_eq!(delivered, 1);

        let event = tokio::time::timeout(Duration::from_millis(100), rx.recv())
            .await
            .expect("receiver should get the event within 100ms")
            .expect("send succeeded so recv must too");
        assert!(matches!(event, MachineEvent::StateChanged));
    }

    #[tokio::test]
    async fn publish_without_subscribers_returns_zero() {
        let bus = EventBus::new();
        let machine_id = Uuid::new_v4();
        let delivered = bus.publish(machine_id, MachineEvent::StateChanged);
        assert_eq!(delivered, 0);
    }

    #[tokio::test]
    async fn multiple_subscribers_all_receive() {
        let bus = EventBus::new();
        let machine_id = Uuid::new_v4();
        let mut rx1 = bus.subscribe(machine_id);
        let mut rx2 = bus.subscribe(machine_id);

        let delivered = bus.publish(machine_id, MachineEvent::StateChanged);
        assert_eq!(delivered, 2);

        assert!(rx1.try_recv().is_ok());
        assert!(rx2.try_recv().is_ok());
    }

    #[test]
    fn wire_format_is_stable() {
        // The agent parses this — pin it to fail loudly on accidental rename.
        let json = serde_json::to_string(&MachineEvent::StateChanged).unwrap();
        assert_eq!(json, r#"{"type":"state_changed"}"#);
    }

    #[test]
    fn prune_drops_slots_without_receivers() {
        let bus = EventBus::new();
        let machine_id = Uuid::new_v4();
        let rx = bus.subscribe(machine_id);
        assert_eq!(bus.machine_count(), 1);
        drop(rx);
        bus.prune();
        assert_eq!(bus.machine_count(), 0);
    }
}
