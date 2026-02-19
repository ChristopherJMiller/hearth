//! Background deployment monitor: polls active deployments, validates canary
//! health, advances rolling batches, and triggers auto-rollback.

use sqlx::PgPool;
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

use crate::db::DeploymentStatusDb;
use crate::repo;
use crate::rollout;

/// Default poll interval for the deployment monitor.
const POLL_INTERVAL: Duration = Duration::from_secs(30);

/// Run the deployment monitor loop.
///
/// Polls every 30 seconds for active deployments (canary or rolling) and:
/// - Validates canary health, advancing to rolling if healthy
/// - Advances rolling batches, setting target_closure on the next batch
/// - Triggers auto-rollback if the failure threshold is exceeded
pub async fn run(pool: PgPool, cancel: CancellationToken) {
    info!("deployment monitor started");

    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                info!("deployment monitor shutting down");
                return;
            }
            _ = tokio::time::sleep(POLL_INTERVAL) => {
                if let Err(e) = tick(&pool).await {
                    error!(error = %e, "deployment monitor tick failed");
                }
            }
        }
    }
}

async fn tick(pool: &PgPool) -> Result<(), Box<dyn std::error::Error>> {
    // Process canary deployments
    let canary_deployments = repo::list_deployments(pool, Some(DeploymentStatusDb::Canary)).await?;

    for dep in &canary_deployments {
        match rollout::advance_to_rolling(pool, dep.id).await {
            Ok(()) => {}
            Err(rollout::RolloutError::ThresholdExceeded { rate, threshold }) => {
                warn!(
                    deployment_id = %dep.id,
                    rate,
                    threshold,
                    "canary failed, deployment rolled back"
                );
            }
            Err(e) => {
                error!(deployment_id = %dep.id, error = %e, "error advancing canary");
            }
        }
    }

    // Process rolling deployments
    let rolling_deployments =
        repo::list_deployments(pool, Some(DeploymentStatusDb::Rolling)).await?;

    for dep in &rolling_deployments {
        if let Err(e) = advance_rolling_batch(pool, dep).await {
            error!(deployment_id = %dep.id, error = %e, "error advancing rolling batch");
        }
    }

    Ok(())
}

async fn advance_rolling_batch(
    pool: &PgPool,
    dep: &crate::db::DeploymentRow,
) -> Result<(), Box<dyn std::error::Error>> {
    let health = crate::health_check::check_deployment_health(pool, dep.id).await?;

    // Check failure threshold
    let rate = health.failure_rate();
    if rate > dep.failure_threshold {
        warn!(
            deployment_id = %dep.id,
            failure_rate = rate,
            threshold = dep.failure_threshold,
            "rolling batch failure threshold exceeded, triggering rollback"
        );
        rollout::trigger_rollback(
            pool,
            dep.id,
            &format!(
                "failure threshold exceeded: {:.1}% failed (threshold: {:.1}%)",
                rate * 100.0,
                dep.failure_threshold * 100.0
            ),
        )
        .await?;
        repo::update_deployment_status(pool, dep.id, DeploymentStatusDb::RolledBack).await?;
        return Ok(());
    }

    // If there are still in-progress machines, wait for them
    if health.in_progress > 0 {
        return Ok(());
    }

    // Get the next batch of pending machines
    let batch = rollout::get_next_batch(pool, dep.id, dep.batch_size as usize).await?;

    if batch.is_empty() {
        // All machines are done — mark deployment as completed
        if health.pending == 0 {
            info!(deployment_id = %dep.id, "all batches completed, finalizing deployment");
            repo::update_deployment_status(pool, dep.id, DeploymentStatusDb::Completed).await?;
        }
        return Ok(());
    }

    // Set target_closure on the next batch of machines
    info!(
        deployment_id = %dep.id,
        batch_size = batch.len(),
        "advancing to next rolling batch"
    );

    for machine_id in &batch {
        let update = hearth_common::api_types::UpdateMachineRequest {
            hostname: None,
            role: None,
            tags: None,
            target_closure: Some(dep.closure.clone()),
            extra_config: None,
        };
        if let Err(e) = repo::update_machine(pool, *machine_id, &update).await {
            error!(machine_id = %machine_id, error = %e, "failed to set target_closure on machine");
        }
        // Mark machine as downloading in deployment tracking
        if let Err(e) = repo::upsert_deployment_machine(
            pool,
            dep.id,
            *machine_id,
            crate::db::MachineUpdateStatusDb::Downloading,
            None,
        )
        .await
        {
            error!(machine_id = %machine_id, error = %e, "failed to update machine deployment status");
        }
    }

    Ok(())
}
