//! Agent-side consumer of the SSE push fast-path (RFC-001).
//!
//! Holds a long-lived `GET /api/v1/machines/{id}/events` connection
//! and pings the poll loop on every event. Reconnects on EOF/timeout
//! with capped exponential backoff (1s → 30s).
//!
//! Push is an optimisation, never a correctness primitive — when the
//! stream is down the agent's regular poll cadence continues to fetch
//! target state.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use futures::StreamExt;
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Shared state between the event stream and the poll loop.
///
/// `poll_now` fires once per received event. The poll loop uses it as
/// a select branch alongside the regular timer — multiple notifications
/// while the loop is busy coalesce into a single permit (the desired
/// semantics: at most one immediate poll per event burst).
///
/// `stream_healthy` is consulted by the poll loop to pick its
/// inter-poll sleep duration: short (`poll_interval_secs`, default 60s)
/// when down, long (`pushed_poll_interval_secs`, default 300s) when up.
#[derive(Debug)]
pub struct PushState {
    poll_now: Notify,
    stream_healthy: AtomicBool,
}

impl PushState {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            poll_now: Notify::new(),
            stream_healthy: AtomicBool::new(false),
        })
    }

    pub fn notify_poll(&self) {
        self.poll_now.notify_one();
    }

    pub async fn wait_poll(&self) {
        self.poll_now.notified().await;
    }

    pub fn is_healthy(&self) -> bool {
        self.stream_healthy.load(Ordering::Relaxed)
    }

    fn mark_healthy(&self) {
        self.stream_healthy.store(true, Ordering::Relaxed);
    }

    fn mark_unhealthy(&self) {
        self.stream_healthy.store(false, Ordering::Relaxed);
    }
}

/// Compute reconnect backoff for `attempt` (0-indexed):
/// 0 → 1s, 1 → 2s, 2 → 4s, 3 → 8s, 4 → 16s, 5+ → 30s.
fn backoff_for(attempt: u32) -> Duration {
    let secs: u64 = 1u64.checked_shl(attempt).unwrap_or(u64::MAX).min(30);
    Duration::from_secs(secs.max(1))
}

/// Run the SSE event stream forever, reconnecting on failure.
///
/// `machine_token_path` is read from disk on each reconnect so the
/// stream picks up rotated tokens — the heartbeat path persists fresh
/// machine tokens to the same file (see `crates/hearth-agent/src/poller.rs`).
pub async fn run_event_stream(
    base_url: String,
    machine_id: Uuid,
    machine_token_path: PathBuf,
    push: Arc<PushState>,
    shutdown: CancellationToken,
) {
    info!(%machine_id, server = %base_url, "starting SSE event stream");

    // Build the HTTP client once. No request-level timeout — the body
    // stream is long-lived by design. A connect timeout still applies.
    let client = match reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        // Dev: self-signed certs in `dev/fleet-vm.nix` and similar.
        // Same posture as the existing JWKS fetch in
        // `crates/hearth-api/src/auth.rs::refresh_jwks`.
        .danger_accept_invalid_certs(true)
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            warn!(error = %e, "failed to build SSE HTTP client, push fast-path disabled");
            return;
        }
    };

    let url = format!(
        "{}/api/v1/machines/{machine_id}/events",
        base_url.trim_end_matches('/')
    );

    let mut attempt: u32 = 0;
    loop {
        if shutdown.is_cancelled() {
            return;
        }

        let token = read_token(&machine_token_path);

        let stream_result =
            connect_and_consume(&client, &url, token.as_deref(), &push, &shutdown).await;

        push.mark_unhealthy();

        match stream_result {
            Ok(ConsumeResult::Cancelled) => {
                info!("SSE event stream shutting down");
                return;
            }
            Ok(ConsumeResult::ServerClosed) => {
                debug!("SSE stream closed by server, reconnecting");
                attempt = 0;
            }
            Err(e) => {
                warn!(error = %e, attempt, "SSE stream failed, will reconnect");
                attempt = attempt.saturating_add(1);
            }
        }

        let delay = backoff_for(attempt);
        tokio::select! {
            () = shutdown.cancelled() => return,
            () = tokio::time::sleep(delay) => {}
        }
    }
}

enum ConsumeResult {
    /// Shutdown signalled.
    Cancelled,
    /// Server closed the stream cleanly (EOF on the body).
    ServerClosed,
}

async fn connect_and_consume(
    client: &reqwest::Client,
    url: &str,
    bearer: Option<&str>,
    push: &PushState,
    shutdown: &CancellationToken,
) -> Result<ConsumeResult, String> {
    let mut req = client.get(url).header("accept", "text/event-stream");
    if let Some(token) = bearer {
        req = req.bearer_auth(token);
    }

    let resp = req.send().await.map_err(|e| format!("connect: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("server returned HTTP {}", resp.status().as_u16()));
    }

    info!("SSE event stream connected");
    push.mark_healthy();

    // Stream the body, parsing newline-delimited SSE events. We only
    // care about the *boundary* between events ("\n\n") — for v1 the
    // payload itself is informational; receiving any data event means
    // "poll now". Comments (`:keepalive`) are skipped so the timer
    // backoff isn't reset by unrelated heartbeats.
    let mut body = resp.bytes_stream();
    let mut buffer: Vec<u8> = Vec::with_capacity(4096);

    loop {
        tokio::select! {
            () = shutdown.cancelled() => return Ok(ConsumeResult::Cancelled),
            chunk = body.next() => match chunk {
                Some(Ok(bytes)) => {
                    buffer.extend_from_slice(&bytes);
                    drain_events(&mut buffer, push);
                }
                Some(Err(e)) => return Err(format!("body read: {e}")),
                None => return Ok(ConsumeResult::ServerClosed),
            }
        }
    }
}

/// Pull complete events (terminated by `\n\n`) out of `buffer` and
/// fire `push.notify_poll()` for each one that contains a `data:`
/// line. Comments-only events (keepalives) are ignored.
fn drain_events(buffer: &mut Vec<u8>, push: &PushState) {
    while let Some(pos) = find_event_boundary(buffer) {
        let event_bytes: Vec<u8> = buffer.drain(..pos + 2).collect();
        // Strip the trailing "\n\n" terminator before scanning.
        let event = &event_bytes[..event_bytes.len().saturating_sub(2)];
        let mut has_data = false;
        for line in event.split(|&b| b == b'\n') {
            // Skip empty lines and comments (which start with `:`).
            if line.first() == Some(&b':') {
                continue;
            }
            if line.starts_with(b"data:") {
                has_data = true;
                break;
            }
        }
        if has_data {
            debug!("received SSE data event, notifying poll loop");
            push.notify_poll();
        } else {
            debug!("received SSE keepalive comment");
        }
    }
}

fn find_event_boundary(buffer: &[u8]) -> Option<usize> {
    // SSE separates events with a blank line. We support both
    // "\n\n" (what axum::Sse emits) and "\r\n\r\n" (older proxies).
    if let Some(pos) = buffer.windows(4).position(|w| w == b"\r\n\r\n") {
        return Some(pos + 2); // index of the second "\n"
    }
    buffer.windows(2).position(|w| w == b"\n\n")
}

fn read_token(path: &Path) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_doubles_then_caps_at_30s() {
        assert_eq!(backoff_for(0), Duration::from_secs(1));
        assert_eq!(backoff_for(1), Duration::from_secs(2));
        assert_eq!(backoff_for(2), Duration::from_secs(4));
        assert_eq!(backoff_for(3), Duration::from_secs(8));
        assert_eq!(backoff_for(4), Duration::from_secs(16));
        assert_eq!(backoff_for(5), Duration::from_secs(30));
        assert_eq!(backoff_for(6), Duration::from_secs(30));
        assert_eq!(backoff_for(64), Duration::from_secs(30));
        // Very large attempt counts must not panic on shift overflow.
        assert_eq!(backoff_for(u32::MAX), Duration::from_secs(30));
    }

    #[test]
    fn drain_events_fires_on_data_line_only() {
        let push = PushState::new();
        let mut buf: Vec<u8> = b"data: {\"type\":\"state_changed\"}\n\n".to_vec();
        drain_events(&mut buf, &push);
        assert!(buf.is_empty(), "buffer should be fully consumed");
        // Notify state is checked indirectly via wait_poll completing.
        let woken = tokio::runtime::Runtime::new().unwrap().block_on(async {
            tokio::time::timeout(Duration::from_millis(50), push.wait_poll())
                .await
                .is_ok()
        });
        assert!(woken, "data event should have notified the poll loop");
    }

    #[test]
    fn drain_events_skips_keepalive_comments() {
        let push = PushState::new();
        let mut buf: Vec<u8> = b":keepalive\n\n".to_vec();
        drain_events(&mut buf, &push);
        assert!(buf.is_empty(), "buffer should be fully consumed");
        let woken = tokio::runtime::Runtime::new().unwrap().block_on(async {
            tokio::time::timeout(Duration::from_millis(50), push.wait_poll())
                .await
                .is_ok()
        });
        assert!(!woken, "keepalive must not wake the poll loop");
    }

    #[test]
    fn drain_events_handles_partial_then_complete() {
        let push = PushState::new();
        let mut buf: Vec<u8> = b"data: first".to_vec();
        drain_events(&mut buf, &push);
        // No boundary yet — buffer retained, no notify.
        assert_eq!(buf.len(), 11);

        buf.extend_from_slice(b"\n\ndata: second\n\n:keepalive\n\n");
        drain_events(&mut buf, &push);
        assert!(buf.is_empty());
        // Two data events were notified; Notify coalesces to a single
        // pending permit, which is the intended semantics (at most one
        // immediate poll per event burst).
        let permits = tokio::runtime::Runtime::new().unwrap().block_on(async {
            let mut count = 0;
            while tokio::time::timeout(Duration::from_millis(10), push.wait_poll())
                .await
                .is_ok()
            {
                count += 1;
            }
            count
        });
        assert!(
            (1..=2).contains(&permits),
            "expected 1-2 coalesced wakes from two data events, got {permits}"
        );
    }

    #[test]
    fn push_state_healthy_round_trips() {
        let push = PushState::new();
        assert!(!push.is_healthy());
        push.mark_healthy();
        assert!(push.is_healthy());
        push.mark_unhealthy();
        assert!(!push.is_healthy());
    }
}
