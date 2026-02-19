//! Rolling batch controller: splits machines into batches, sets target_closure
//! per batch, waits for completion, checks health, and triggers rollback if
//! failure threshold is exceeded.

use sqlx::PgPool;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::db::{DeploymentStatusDb, MachineUpdateStatusDb};
use crate::health_check;
use crate::repo;

/// Errors from the rollout controller.
#[derive(Debug, thiserror::Error)]
pub enum RolloutError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("failure threshold exceeded: {rate:.1}% failed (threshold: {threshold:.1}%)")]
    ThresholdExceeded { rate: f64, threshold: f64 },
    #[error("deployment not found: {0}")]
    NotFound(Uuid),
}

/// Select canary machines for a deployment.
///
/// Returns the machine IDs selected as canaries (up to `canary_size`).
pub async fn select_canary_machines(
    pool: &PgPool,
    deployment_id: Uuid,
    canary_size: usize,
) -> Result<Vec<Uuid>, RolloutError> {
    let machines = repo::get_deployment_machines(pool, deployment_id).await?;

    let canaries: Vec<Uuid> = machines
        .iter()
        .filter(|m| m.status == MachineUpdateStatusDb::Pending)
        .take(canary_size)
        .map(|m| m.machine_id)
        .collect();

    info!(
        deployment_id = %deployment_id,
        count = canaries.len(),
        "selected canary machines"
    );

    Ok(canaries)
}

/// Get the next batch of machines to roll out to.
///
/// Returns up to `batch_size` pending machines.
pub async fn get_next_batch(
    pool: &PgPool,
    deployment_id: Uuid,
    batch_size: usize,
) -> Result<Vec<Uuid>, RolloutError> {
    let machines = repo::get_deployment_machines(pool, deployment_id).await?;

    let batch: Vec<Uuid> = machines
        .iter()
        .filter(|m| m.status == MachineUpdateStatusDb::Pending)
        .take(batch_size)
        .map(|m| m.machine_id)
        .collect();

    info!(
        deployment_id = %deployment_id,
        batch_size = batch.len(),
        remaining = machines.iter().filter(|m| m.status == MachineUpdateStatusDb::Pending).count() - batch.len(),
        "selected next batch"
    );

    Ok(batch)
}

/// Validate canary health: check if the failure rate is below the threshold.
pub async fn validate_canary(
    pool: &PgPool,
    deployment_id: Uuid,
    failure_threshold: f64,
) -> Result<bool, RolloutError> {
    let health = health_check::check_deployment_health(pool, deployment_id).await?;

    if !health.is_complete() {
        // Canary batch hasn't finished yet
        return Ok(true);
    }

    let rate = health.failure_rate();
    if rate > failure_threshold {
        warn!(
            deployment_id = %deployment_id,
            failure_rate = rate,
            threshold = failure_threshold,
            "canary validation FAILED"
        );
        return Ok(false);
    }

    info!(
        deployment_id = %deployment_id,
        failure_rate = rate,
        "canary validation passed"
    );

    Ok(true)
}

/// Advance a deployment from canary to rolling if canary health is good.
pub async fn advance_to_rolling(pool: &PgPool, deployment_id: Uuid) -> Result<(), RolloutError> {
    let deployment = repo::get_deployment(pool, deployment_id)
        .await?
        .ok_or(RolloutError::NotFound(deployment_id))?;

    if deployment.status != DeploymentStatusDb::Canary {
        return Ok(());
    }

    let healthy = validate_canary(pool, deployment_id, deployment.failure_threshold).await?;
    if !healthy {
        // Trigger rollback
        trigger_rollback(
            pool,
            deployment_id,
            "canary validation failed: failure threshold exceeded",
        )
        .await?;
        return Err(RolloutError::ThresholdExceeded {
            rate: health_check::check_deployment_health(pool, deployment_id)
                .await?
                .failure_rate()
                * 100.0,
            threshold: deployment.failure_threshold * 100.0,
        });
    }

    // Advance to rolling
    repo::update_deployment_status(pool, deployment_id, DeploymentStatusDb::Rolling).await?;
    info!(deployment_id = %deployment_id, "advanced to rolling");

    Ok(())
}

/// Trigger a rollback for a deployment.
pub async fn trigger_rollback(
    pool: &PgPool,
    deployment_id: Uuid,
    reason: &str,
) -> Result<(), RolloutError> {
    warn!(deployment_id = %deployment_id, reason, "triggering rollback");

    repo::rollback_deployment(pool, deployment_id, reason).await?;

    // Set all pending/in-progress machines to rolled_back
    let machines = repo::get_deployment_machines(pool, deployment_id).await?;
    for m in &machines {
        if matches!(
            m.status,
            MachineUpdateStatusDb::Pending
                | MachineUpdateStatusDb::Downloading
                | MachineUpdateStatusDb::Switching
        ) && let Err(e) = repo::upsert_deployment_machine(
            pool,
            deployment_id,
            m.machine_id,
            MachineUpdateStatusDb::RolledBack,
            Some(reason),
        )
        .await
        {
            error!(
                machine_id = %m.machine_id,
                error = %e,
                "failed to rollback machine"
            );
        }
    }

    // Restore rollback closures on affected machines
    for m in &machines {
        if let Some(machine) = repo::get_machine(pool, m.machine_id).await? {
            let machine: hearth_common::api_types::Machine = machine.into();
            if let Some(rollback_closure) = &machine.rollback_closure {
                let update = hearth_common::api_types::UpdateMachineRequest {
                    hostname: None,
                    role: None,
                    tags: None,
                    target_closure: Some(rollback_closure.clone()),
                    extra_config: None,
                };
                if let Err(e) = repo::update_machine(pool, m.machine_id, &update).await {
                    error!(machine_id = %m.machine_id, error = %e, "failed to restore rollback closure");
                }
            }
        }
    }

    Ok(())
}
