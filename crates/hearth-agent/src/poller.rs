//! Polling loop: periodically fetches target state from the control plane,
//! sends heartbeats, and triggers system updates when the closure changes.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use hearth_common::api_client::HearthApiClient;
use hearth_common::api_types::{ActionResultReport, HeartbeatRequest, MachineUpdateStatus};

use crate::queue::OfflineQueue;
use crate::updater;

/// Default path for Prometheus textfile metrics (node_exporter textfile collector).
const METRICS_PATH: &str = "/var/lib/prometheus-node-exporter/hearth.prom";

/// Run the main poll loop until the shutdown token is cancelled.
///
/// On each tick the loop:
/// 1. Drains the offline event queue.
/// 2. Fetches the target state for this machine.
/// 3. Compares the target closure to the locally-tracked current closure.
/// 4. If they differ, delegates to the updater, reporting deployment status.
/// 5. Sends a heartbeat to the control plane.
pub async fn run_poll_loop<C: HearthApiClient>(
    client: Arc<C>,
    machine_id: Uuid,
    interval: Duration,
    queue: Arc<OfflineQueue>,
    machine_token_path: PathBuf,
    shutdown: CancellationToken,
) {
    info!(
        %machine_id,
        interval_secs = interval.as_secs(),
        "starting poll loop"
    );

    // Track the closure we believe is currently active on this machine.
    // In a future phase this will be read from the system profile symlink.
    let mut current_closure: Option<String> = None;

    // Track the active deployment from the control plane.
    let mut active_deployment_id: Option<Uuid> = None;

    // Track the last update error so it can be reported in the next heartbeat.
    let mut update_error: Option<String> = None;

    // Cache credentials from heartbeat — refreshed every cycle.
    let mut cache_url: Option<String> = None;
    let mut last_cache_token: Option<String> = None;

    // Track last successful heartbeat time for metrics.
    let mut last_heartbeat_time: Option<std::time::Instant> = None;
    // Track user environment count from heartbeat response.
    let mut user_env_count: u64 = 0;

    loop {
        // --- Drain offline queue ---
        match queue.drain() {
            Ok(events) if !events.is_empty() => {
                for event in events {
                    debug!(event_type = %event.event_type, "replaying queued event");
                    if let Err(e) = replay_event(&*client, &event).await {
                        warn!(event_type = %event.event_type, error = %e, "failed to replay queued event");
                        if let Err(qe) = queue.enqueue(&event.event_type, &event.payload) {
                            error!(error = %qe, "failed to re-queue event");
                        }
                    }
                }
            }
            Ok(_) => {}
            Err(e) => {
                warn!(error = %e, "failed to drain offline queue");
            }
        }

        // --- Fetch target state ---
        match client.get_target_state(machine_id).await {
            Ok(target_state) => {
                debug!(?target_state, "received target state");

                if let Some(target_closure) = &target_state.target_closure {
                    // Check if an update is needed (current != target).
                    let needs_update = current_closure.as_deref() != Some(target_closure.as_str());

                    if needs_update {
                        // Report downloading status to the control plane.
                        if let Some(deploy_id) = active_deployment_id {
                            let _ = client
                                .report_update_status(
                                    deploy_id,
                                    machine_id,
                                    MachineUpdateStatus::Downloading,
                                    None,
                                )
                                .await;
                        }

                        update_error = None;

                        match updater::check_and_apply_update(
                            current_closure.as_deref(),
                            target_closure,
                            cache_url.as_deref(),
                        )
                        .await
                        {
                            Ok(true) => {
                                info!(closure = %target_closure, "update applied successfully");
                                current_closure = Some(target_closure.clone());

                                // Report completed status.
                                if let Some(deploy_id) = active_deployment_id {
                                    let _ = client
                                        .report_update_status(
                                            deploy_id,
                                            machine_id,
                                            MachineUpdateStatus::Completed,
                                            None,
                                        )
                                        .await;
                                }
                            }
                            Ok(false) => {
                                debug!("no update needed");
                            }
                            Err(e) => {
                                let err_msg = e.to_string();
                                error!(error = %err_msg, "update failed");
                                update_error = Some(err_msg.clone());

                                // Report failed status.
                                if let Some(deploy_id) = active_deployment_id {
                                    let _ = client
                                        .report_update_status(
                                            deploy_id,
                                            machine_id,
                                            MachineUpdateStatus::Failed,
                                            Some(&err_msg),
                                        )
                                        .await;
                                }
                            }
                        }
                    } else {
                        debug!("no update needed");
                    }
                } else {
                    debug!("no target closure set for this machine");
                }
            }
            Err(e) => {
                warn!(error = %e, "failed to fetch target state, will retry next cycle");
            }
        }

        // --- Send heartbeat ---
        let heartbeat = HeartbeatRequest {
            machine_id,
            current_closure: current_closure.clone(),
            os_version: None,
            uptime_seconds: None,
            update_in_progress: None,
            update_error: update_error.clone(),
            headscale_ip: crate::headscale::detect_headscale_ip(),
        };
        match client.send_heartbeat(&heartbeat).await {
            Ok(resp) => {
                debug!(?resp, "heartbeat acknowledged");

                // Capture active deployment ID from response.
                active_deployment_id = resp.active_deployment_id;

                // Write cache credentials to netrc if the token has changed.
                if let (Some(url), Some(token)) = (&resp.cache_url, &resp.cache_token) {
                    let token_changed = last_cache_token.as_deref() != Some(token.as_str());
                    if token_changed {
                        match write_netrc(url, token) {
                            Ok(()) => {
                                info!("wrote cache credentials to /run/hearth/netrc");
                                cache_url = Some(url.clone());
                                last_cache_token = Some(token.clone());
                            }
                            Err(e) => {
                                warn!(error = %e, "failed to write cache netrc");
                            }
                        }
                    }
                }

                // Refresh machine token if the server sent a new one.
                if let Some(new_token) = &resp.machine_token {
                    client.update_token(new_token);
                    if let Err(e) = std::fs::write(&machine_token_path, new_token) {
                        warn!(error = %e, "failed to persist refreshed machine token");
                    } else {
                        info!("machine token refreshed and persisted");
                    }
                }

                last_heartbeat_time = Some(std::time::Instant::now());

                // Process pending software installs
                for install in &resp.pending_installs {
                    let req_id = install.request_id;
                    // Claim the install first (approved -> installing)
                    if let Err(e) = client.claim_install(req_id).await {
                        warn!(request_id = %req_id, error = %e, "failed to claim install, skipping");
                        continue;
                    }
                    // Execute the install
                    let (success, error_message) =
                        match crate::installer::execute_install(install).await {
                            Ok(()) => (true, None),
                            Err(e) => {
                                error!(request_id = %req_id, error = %e, "install failed");
                                (false, Some(e.to_string()))
                            }
                        };
                    // Report result back to control plane
                    let report = hearth_common::api_types::InstallResultReport {
                        request_id: req_id,
                        success,
                        error_message,
                    };
                    if let Err(e) = client.report_install_result(&report).await {
                        warn!(request_id = %req_id, error = %e, "failed to report install result, queueing");
                        let payload = serde_json::to_string(&report).unwrap_or_default();
                        if let Err(qe) = queue.enqueue("install_result", &payload) {
                            error!(error = %qe, "failed to queue install result");
                        }
                    }
                }

                // Process pending remote actions
                for action in &resp.pending_actions {
                    let (success, result) = crate::actions::execute_action(action).await;
                    let report = ActionResultReport {
                        action_id: action.id,
                        success,
                        result,
                    };
                    if let Err(e) = client.report_action_result(&report).await {
                        warn!(action_id = %action.id, error = %e, "failed to report action result, queueing");
                        let payload = serde_json::to_string(&report).unwrap_or_default();
                        if let Err(qe) = queue.enqueue("action_result", &payload) {
                            error!(error = %qe, "failed to queue action result");
                        }
                    }
                }

                // Pre-stage pending user environment closures from the cache.
                // This pulls closures into the local nix store so they're ready
                // for instant activation on next login.
                for user_env in &resp.pending_user_envs {
                    if !hearth_common::nix_store::is_valid_store_path(&user_env.target_closure) {
                        warn!(
                            username = %user_env.username,
                            closure = %user_env.target_closure,
                            "invalid store path in pending user env, skipping"
                        );
                        continue;
                    }
                    info!(
                        username = %user_env.username,
                        closure = %user_env.target_closure,
                        "pre-staging user environment closure"
                    );
                    if let Some(cache_url) = &user_env.cache_url {
                        let result = tokio::process::Command::new("nix")
                            .args(["copy", "--from", cache_url, &user_env.target_closure])
                            .output()
                            .await;
                        match result {
                            Ok(out) if out.status.success() => {
                                info!(
                                    username = %user_env.username,
                                    "pre-staged user env closure"
                                );
                            }
                            Ok(out) => {
                                let stderr = String::from_utf8_lossy(&out.stderr);
                                warn!(
                                    username = %user_env.username,
                                    %stderr,
                                    "nix copy for user env pre-staging failed"
                                );
                            }
                            Err(e) => {
                                warn!(
                                    username = %user_env.username,
                                    error = %e,
                                    "failed to run nix copy for user env pre-staging"
                                );
                            }
                        }
                    }
                }

                // Track user env count from response for metrics
                user_env_count = resp.pending_user_envs.len() as u64;
            }
            Err(e) => {
                warn!(error = %e, "failed to send heartbeat, queueing for later");
                let payload = serde_json::to_string(&heartbeat).unwrap_or_default();
                if let Err(qe) = queue.enqueue("heartbeat", &payload) {
                    error!(error = %qe, "failed to queue heartbeat");
                }
            }
        }

        // Clear the update error after it has been reported via heartbeat.
        if update_error.is_some() {
            update_error = None;
        }

        // --- Write textfile metrics for node_exporter ---
        let heartbeat_age = last_heartbeat_time
            .map(|t| t.elapsed().as_secs_f64())
            .unwrap_or(f64::NAN);
        crate::metrics::write_textfile_metrics(
            std::path::Path::new(METRICS_PATH),
            &machine_id.to_string(),
            current_closure.as_deref(),
            None, // target_closure not available here; resolved from heartbeat response
            heartbeat_age,
            user_env_count,
        );

        // --- Wait for the next tick or shutdown ---
        tokio::select! {
            () = tokio::time::sleep(interval) => {}
            () = shutdown.cancelled() => {
                info!("poll loop shutting down");
                return;
            }
        }
    }
}

/// Write a netrc file with bearer credentials for the given cache URL.
fn write_netrc(cache_url: &str, token: &str) -> Result<(), Box<dyn std::error::Error>> {
    // Extract hostname from URL (e.g. "http://cache.example.com:8080/foo" -> "cache.example.com")
    let host = cache_url
        .split("://")
        .nth(1)
        .unwrap_or(cache_url)
        .split('/')
        .next()
        .unwrap_or(cache_url)
        .split(':')
        .next()
        .unwrap_or(cache_url);
    let content = format!("machine {host}\nlogin bearer\npassword {token}\n");
    let path = std::path::Path::new("/run/hearth/netrc");
    std::fs::write(path, content)?;
    Ok(())
}

async fn replay_event<C: HearthApiClient>(
    client: &C,
    event: &crate::queue::QueuedEvent,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match event.event_type.as_str() {
        "heartbeat" => {
            let req: HeartbeatRequest = serde_json::from_str(&event.payload)?;
            client.send_heartbeat(&req).await?;
            Ok(())
        }
        "install_result" => {
            let report: hearth_common::api_types::InstallResultReport =
                serde_json::from_str(&event.payload)?;
            client.report_install_result(&report).await?;
            Ok(())
        }
        "action_result" => {
            let report: ActionResultReport = serde_json::from_str(&event.payload)?;
            client.report_action_result(&report).await?;
            Ok(())
        }
        other => {
            debug!(event_type = %other, "unknown queued event type, discarding");
            Ok(())
        }
    }
}
