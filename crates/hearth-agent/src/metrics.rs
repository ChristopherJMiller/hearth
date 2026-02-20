//! Prometheus textfile exporter for node_exporter.
//!
//! Uses the `prometheus-client` crate to build a proper metric registry and
//! encode it in OpenMetrics text format. Writes to the node_exporter textfile
//! collector directory so fleet metrics appear alongside system metrics.

use std::path::Path;
use std::sync::atomic::AtomicU64;

use prometheus_client::encoding::text::encode;
use prometheus_client::metrics::family::Family;
use prometheus_client::metrics::gauge::Gauge;
use prometheus_client::registry::Registry;
use tracing::{debug, warn};

/// Write Hearth agent metrics in Prometheus text format.
///
/// Metrics exposed:
/// - `hearth_agent_info` — always 1, carries machine_id label
/// - `hearth_agent_closure_drift` — 1 if current != target, 0 otherwise
/// - `hearth_agent_last_heartbeat_age_seconds` — seconds since last successful heartbeat
/// - `hearth_agent_user_environments` — number of active user environments on this machine
pub fn write_textfile_metrics(
    metrics_path: &Path,
    machine_id: &str,
    current_closure: Option<&str>,
    target_closure: Option<&str>,
    heartbeat_age_secs: f64,
    user_env_count: u64,
) {
    let mut registry = <Registry>::default();

    // hearth_agent_info{machine_id="..."}
    let info_family = Family::<Vec<(String, String)>, Gauge>::default();
    registry.register(
        "hearth_agent_info",
        "Hearth agent information",
        info_family.clone(),
    );
    info_family
        .get_or_create(&vec![("machine_id".to_string(), machine_id.to_string())])
        .set(1);

    // hearth_agent_closure_drift
    let drift_gauge = Gauge::<i64>::default();
    registry.register(
        "hearth_agent_closure_drift",
        "Whether current closure differs from target (1=drifted)",
        drift_gauge.clone(),
    );
    let drift = match (current_closure, target_closure) {
        (Some(c), Some(t)) if c == t => 0,
        (None, None) => 0,
        (_, Some(_)) => 1, // target set but doesn't match current
        _ => 0,
    };
    drift_gauge.set(drift);

    // hearth_agent_last_heartbeat_age_seconds
    let heartbeat_age_gauge = Gauge::<f64, AtomicU64>::default();
    registry.register(
        "hearth_agent_last_heartbeat_age_seconds",
        "Seconds since last successful heartbeat",
        heartbeat_age_gauge.clone(),
    );
    heartbeat_age_gauge.set(heartbeat_age_secs);

    // hearth_agent_user_environments
    let user_env_gauge = Gauge::<i64>::default();
    registry.register(
        "hearth_agent_user_environments",
        "Number of active user environments",
        user_env_gauge.clone(),
    );
    user_env_gauge.set(user_env_count as i64);

    // Encode to OpenMetrics text format
    let mut buf = String::with_capacity(1024);
    if let Err(e) = encode(&mut buf, &registry) {
        warn!(error = %e, "failed to encode metrics");
        return;
    }

    // Write atomically: write to .tmp then rename
    let tmp_path = metrics_path.with_extension("prom.tmp");
    if let Some(parent) = metrics_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match std::fs::write(&tmp_path, &buf) {
        Ok(()) => {
            if let Err(e) = std::fs::rename(&tmp_path, metrics_path) {
                warn!(error = %e, "failed to rename metrics tempfile");
            } else {
                debug!("wrote textfile metrics to {}", metrics_path.display());
            }
        }
        Err(e) => {
            warn!(error = %e, "failed to write textfile metrics");
        }
    }
}
