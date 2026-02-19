//! Build orchestrator: ties evaluation → build → cache push → deployment creation.

use sqlx::PgPool;
use tracing::{error, info};

use super::{builder, cache, config_gen, evaluator};
use crate::db::DeploymentStatusDb;
use crate::repo;
use hearth_common::api_types::CreateDeploymentRequest;

/// Errors from the orchestrator.
#[derive(Debug, thiserror::Error)]
pub enum OrchestrateError {
    #[error("evaluation failed: {0}")]
    Eval(#[from] evaluator::EvalError),
    #[error("build failed: {0}")]
    Build(#[from] builder::BuildError),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("no successful builds")]
    NoBuilds,
    #[error("config generation failed: {0}")]
    ConfigGen(String),
}

/// Result of a build orchestration run.
#[derive(Debug)]
pub struct OrchestrateResult {
    pub deployment_id: uuid::Uuid,
    pub total_machines: usize,
    pub closures_built: usize,
    pub closures_pushed: usize,
}

/// Run the full build pipeline:
///
/// 1. Generate fleet config from DB inventory.
/// 2. Evaluate the flake reference (nix-eval-jobs).
/// 3. Build all derivations in parallel.
/// 4. Push successful builds to Attic cache.
/// 5. Create a Deployment record targeting matched machines.
/// 6. Set `target_closure` on each matched machine.
pub async fn run_build_pipeline(
    pool: &PgPool,
    flake_ref: &str,
    target_filter: Option<&serde_json::Value>,
    canary_size: i32,
    batch_size: i32,
    failure_threshold: f64,
) -> Result<OrchestrateResult, OrchestrateError> {
    info!(flake_ref, "starting build pipeline");

    // Step 1: Generate fleet config to know which machines we're targeting.
    let fleet_config = config_gen::generate_fleet_config(pool, target_filter)
        .await
        .map_err(|e| OrchestrateError::ConfigGen(e.to_string()))?;

    let total_machines = fleet_config.machines.len();
    info!(total_machines, "fleet config generated");

    // Step 2: Evaluate the flake.
    let eval_results = evaluator::evaluate_flake(flake_ref).await?;
    let successful_evals: Vec<_> = eval_results.iter().filter(|r| r.error.is_none()).collect();

    if successful_evals.is_empty() {
        return Err(OrchestrateError::NoBuilds);
    }

    info!(
        total = eval_results.len(),
        successful = successful_evals.len(),
        "evaluation complete"
    );

    // Step 3: Build all derivations.
    let drv_paths: Vec<String> = successful_evals
        .iter()
        .map(|r| r.drv_path.clone())
        .collect();
    let build_results = builder::build_all(&drv_paths, 4).await?;

    let successful_builds: Vec<_> = build_results.iter().filter(|r| r.success).collect();
    if successful_builds.is_empty() {
        return Err(OrchestrateError::NoBuilds);
    }

    // The "closure" for the deployment is the first successful output path.
    // In a fleet setup, this would typically be the system closure.
    let primary_closure = successful_builds[0].out_path.clone();

    // Step 4: Push to Attic cache.
    let cache_name = std::env::var("HEARTH_ATTIC_CACHE").unwrap_or_else(|_| "hearth".to_string());
    let out_paths: Vec<String> = successful_builds
        .iter()
        .map(|r| r.out_path.clone())
        .collect();
    let pushed = cache::push_all(&cache_name, &out_paths).await;

    info!(pushed, total = out_paths.len(), "cache push complete");

    // Step 5: Create deployment record.
    let deployment_req = CreateDeploymentRequest {
        closure: primary_closure.clone(),
        module_library_ref: Some(flake_ref.to_string()),
        instance_data_hash: None,
        target_filter: target_filter.cloned(),
        canary_size,
        batch_size,
        failure_threshold,
    };

    let deployment = repo::create_deployment(pool, &deployment_req).await?;
    let deployment_id = deployment.id;

    info!(
        deployment_id = %deployment_id,
        closure = %primary_closure,
        "deployment created"
    );

    // Step 6: Create deployment_machine entries (Pending, no target_closure yet).
    for machine in &fleet_config.machines {
        let machine_id: uuid::Uuid = match machine.machine_id.parse() {
            Ok(id) => id,
            Err(e) => {
                error!(machine_id = %machine.machine_id, error = %e, "invalid machine UUID");
                continue;
            }
        };

        // Create deployment_machine entry as Pending
        if let Err(e) = repo::upsert_deployment_machine(
            pool,
            deployment_id,
            machine_id,
            crate::db::MachineUpdateStatusDb::Pending,
            None,
        )
        .await
        {
            error!(%machine_id, error = %e, "failed to create deployment_machine entry");
        }
    }

    // Step 7: Select canary machines and set target_closure only on them.
    let canaries = crate::rollout::select_canary_machines(pool, deployment_id, canary_size as usize)
        .await
        .map_err(|e| OrchestrateError::Database(match e {
            crate::rollout::RolloutError::Database(db_err) => db_err,
            other => sqlx::Error::Protocol(other.to_string()),
        }))?;

    for machine_id in &canaries {
        let update_req = hearth_common::api_types::UpdateMachineRequest {
            hostname: None,
            role: None,
            tags: None,
            target_closure: Some(primary_closure.clone()),
            extra_config: None,
        };
        if let Err(e) = repo::update_machine(pool, *machine_id, &update_req).await {
            error!(%machine_id, error = %e, "failed to set target_closure on canary");
        }
        // Mark canary as Downloading
        if let Err(e) = repo::upsert_deployment_machine(
            pool,
            deployment_id,
            *machine_id,
            crate::db::MachineUpdateStatusDb::Downloading,
            None,
        )
        .await
        {
            error!(%machine_id, error = %e, "failed to update canary machine status");
        }
    }

    info!(
        deployment_id = %deployment_id,
        canaries = canaries.len(),
        "canary machines selected, advancing to canary phase"
    );

    // Advance deployment to Canary state.
    let _ = repo::update_deployment_status(pool, deployment_id, DeploymentStatusDb::Canary).await;

    Ok(OrchestrateResult {
        deployment_id,
        total_machines,
        closures_built: successful_builds.len(),
        closures_pushed: pushed,
    })
}
