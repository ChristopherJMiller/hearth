//! Prometheus metrics for the Hearth API server.
//!
//! Exposes a `/metrics` endpoint with fleet gauges, request counters, and histograms.

use axum::body::Body;
use axum::extract::State;
use axum::http::Request;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use metrics::{counter, gauge, histogram};
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use sqlx::PgPool;
use std::time::Instant;
use tracing::{debug, warn};

/// Initialise the Prometheus metrics recorder and return the render handle.
pub fn init() -> PrometheusHandle {
    let builder = PrometheusBuilder::new();
    builder
        .install_recorder()
        .expect("failed to install Prometheus recorder")
}

/// Axum handler that renders all collected metrics in Prometheus text format.
pub async fn metrics_handler(State(handle): State<PrometheusHandle>) -> impl IntoResponse {
    handle.render()
}

/// Axum middleware that records per-request metrics.
pub async fn track_request(req: Request<Body>, next: Next) -> Response {
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let start = Instant::now();

    let response = next.run(req).await;

    let status = response.status().as_u16().to_string();
    let duration = start.elapsed().as_secs_f64();

    let labels = [
        ("method", method.to_string()),
        ("path", normalize_path(&path)),
        ("status", status),
    ];

    counter!("http_requests_total", &labels).increment(1);
    histogram!("http_request_duration_seconds", &labels[..2]).record(duration);

    response
}

/// Collapse path parameters (UUIDs, usernames) into placeholders for low-cardinality labels.
fn normalize_path(path: &str) -> String {
    let segments: Vec<&str> = path.split('/').collect();
    let normalized: Vec<&str> = segments
        .iter()
        .map(|s| {
            if uuid::Uuid::parse_str(s).is_ok() {
                "{id}"
            } else {
                s
            }
        })
        .collect();
    normalized.join("/")
}

/// Background task that periodically queries fleet stats and updates Prometheus gauges.
pub async fn refresh_fleet_gauges(pool: PgPool, interval: std::time::Duration) {
    loop {
        match crate::repo::get_fleet_stats(&pool).await {
            Ok(stats) => {
                gauge!("hearth_active_machines").set(stats.active_machines as f64);
                gauge!("hearth_total_machines").set(stats.total_machines as f64);
                gauge!("hearth_pending_enrollments").set(stats.pending_enrollments as f64);
                gauge!("hearth_active_deployments").set(stats.active_deployments as f64);
                gauge!("hearth_pending_requests").set(stats.pending_requests as f64);
                debug!("refreshed fleet gauges");
            }
            Err(e) => {
                warn!(error = %e, "failed to refresh fleet gauges");
            }
        }

        // Also track build queue depth
        match crate::repo::list_build_jobs(&pool, Some(crate::db::BuildJobStatusDb::Pending)).await
        {
            Ok(jobs) => {
                gauge!("hearth_build_queue_depth").set(jobs.len() as f64);
            }
            Err(e) => {
                warn!(error = %e, "failed to query build queue depth");
            }
        }

        tokio::time::sleep(interval).await;
    }
}
