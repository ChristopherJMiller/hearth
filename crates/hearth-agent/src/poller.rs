//! Polling loop: periodically fetches target state from the control plane,
//! sends heartbeats, and triggers system updates when the closure changes.

use std::sync::Arc;
use std::time::Duration;

use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use hearth_common::api_client::HearthApiClient;
use hearth_common::api_types::{HeartbeatRequest, MachineUpdateStatus};

use crate::queue::OfflineQueue;
use crate::updater;

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
        };
        match client.send_heartbeat(&heartbeat).await {
            Ok(resp) => {
                debug!(?resp, "heartbeat acknowledged");

                // Capture active deployment ID from response.
                active_deployment_id = resp.active_deployment_id;

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
        other => {
            debug!(event_type = %other, "unknown queued event type, discarding");
            Ok(())
        }
    }
}
