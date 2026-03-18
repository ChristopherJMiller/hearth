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
    pub closure: String,
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

    // Step 1: Generate fleet config with per-machine instance data.
    let mut fleet_config = config_gen::generate_fleet_config(pool, target_filter)
        .await
        .map_err(|e| OrchestrateError::ConfigGen(e.to_string()))?;

    let total_machines = fleet_config.machines.len();
    if total_machines == 0 {
        return Err(OrchestrateError::ConfigGen(
            "no machines matched the target filter".into(),
        ));
    }
    info!(total_machines, "fleet config generated");

    // Inject global settings from environment into each machine config.
    let server_url =
        std::env::var("HEARTH_SERVER_URL").unwrap_or_else(|_| "http://localhost:3000".to_string());
    let kanidm_url = std::env::var("HEARTH_KANIDM_URL").ok();
    let binary_cache_url = std::env::var("HEARTH_BINARY_CACHE_URL").ok();

    for mc in &mut fleet_config.machines {
        mc.server_url = Some(server_url.clone());
        mc.kanidm_url = kanidm_url.clone();
        mc.binary_cache_url = binary_cache_url.clone();
    }

    // Step 2: Write per-machine instance data JSONs + eval.nix to a temp dir.
    let build_dir = std::env::temp_dir().join(format!("hearth-build-{}", uuid::Uuid::new_v4()));
    let eval_path = config_gen::write_build_dir(&build_dir, &fleet_config, flake_ref)
        .map_err(|e| OrchestrateError::ConfigGen(format!("failed to write build dir: {e}")))?;

    info!(build_dir = %build_dir.display(), "build directory prepared");

    // Step 3: Evaluate per-machine closures via nix-eval-jobs --expr.
    let eval_path_str = eval_path.to_string_lossy().to_string();
    let eval_results = evaluator::evaluate_expr(&eval_path_str).await?;
    let successful_evals: Vec<_> = eval_results.iter().filter(|r| r.error.is_none()).collect();

    if successful_evals.is_empty() {
        // Clean up temp dir
        let _ = std::fs::remove_dir_all(&build_dir);
        return Err(OrchestrateError::NoBuilds);
    }

    info!(
        total = eval_results.len(),
        successful = successful_evals.len(),
        "evaluation complete"
    );

    // Step 4: Build all derivations in parallel.
    let drv_paths: Vec<String> = successful_evals
        .iter()
        .map(|r| r.drv_path.clone())
        .collect();
    let build_results = builder::build_all(&drv_paths, 4).await?;

    let successful_builds: Vec<_> = build_results.iter().filter(|r| r.success).collect();
    if successful_builds.is_empty() {
        let _ = std::fs::remove_dir_all(&build_dir);
        return Err(OrchestrateError::NoBuilds);
    }

    // Build a hostname → out_path map so we can assign per-machine closures.
    let closure_map = build_closure_map(&eval_results, &build_results);

    // The "primary" closure for the deployment record — use the first one.
    let primary_closure = successful_builds[0].out_path.clone();

    // Step 5: Push all closures to Attic cache.
    let cache_name = std::env::var("HEARTH_ATTIC_CACHE").unwrap_or_else(|_| "hearth".to_string());
    let out_paths: Vec<String> = successful_builds
        .iter()
        .map(|r| r.out_path.clone())
        .collect();
    let pushed = cache::push_all(&cache_name, &out_paths).await;

    info!(pushed, total = out_paths.len(), "cache push complete");

    // Compute aggregate instance data hash for reproducibility.
    let aggregate_hash = aggregate_instance_hash(&fleet_config);

    // Step 6: Create deployment record.
    let deployment_req = CreateDeploymentRequest {
        closure: primary_closure.clone(),
        module_library_ref: Some(flake_ref.to_string()),
        instance_data_hash: Some(aggregate_hash),
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

    // Step 7: Create deployment_machine entries and assign per-machine closures.
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

    // Step 8: Select canary machines and set their per-machine target_closure.
    let canaries =
        crate::rollout::select_canary_machines(pool, deployment_id, canary_size as usize)
            .await
            .map_err(|e| {
                OrchestrateError::Database(match e {
                    crate::rollout::RolloutError::Database(db_err) => db_err,
                    other => sqlx::Error::Protocol(other.to_string()),
                })
            })?;

    for machine_id in &canaries {
        // Look up this machine's hostname to find its per-machine closure.
        let machine_closure = fleet_config
            .machines
            .iter()
            .find(|mc| mc.machine_id == machine_id.to_string())
            .and_then(|mc| closure_map.get(&mc.hostname))
            .cloned()
            .unwrap_or_else(|| primary_closure.clone());

        let update_req = hearth_common::api_types::UpdateMachineRequest {
            hostname: None,
            role: None,
            tags: None,
            target_closure: Some(machine_closure),
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

    // Clean up temp build directory.
    let _ = std::fs::remove_dir_all(&build_dir);

    Ok(OrchestrateResult {
        deployment_id,
        closure: primary_closure,
        total_machines,
        closures_built: successful_builds.len(),
        closures_pushed: pushed,
    })
}

/// Build a hostname → out_path map from eval results and build results.
///
/// Matches eval attrs to build derivations by drv_path, producing a mapping
/// of hostname to output store path for per-machine closure assignment.
pub(crate) fn build_closure_map(
    eval_results: &[evaluator::NixEvalResult],
    build_results: &[builder::BuildResult],
) -> std::collections::HashMap<String, String> {
    // Index successful builds by drv_path for O(1) lookup.
    let build_index: std::collections::HashMap<&str, &str> = build_results
        .iter()
        .filter(|r| r.success)
        .map(|r| (r.drv_path.as_str(), r.out_path.as_str()))
        .collect();

    eval_results
        .iter()
        .filter(|r| r.error.is_none())
        .filter_map(|eval| {
            build_index
                .get(eval.drv_path.as_str())
                .map(|out| (eval.attr.clone(), (*out).to_string()))
        })
        .collect()
}

/// Compute an aggregate SHA-256 hash over all machine instance data hashes.
pub(crate) fn aggregate_instance_hash(fleet_config: &config_gen::FleetConfig) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    for mc in &fleet_config.machines {
        hasher.update(config_gen::instance_data_hash(mc).as_bytes());
    }
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build::builder::BuildResult;
    use crate::build::config_gen::{FleetConfig, MachineConfig};
    use crate::build::evaluator::NixEvalResult;

    fn eval_ok(attr: &str, drv: &str) -> NixEvalResult {
        NixEvalResult {
            attr: attr.into(),
            drv_path: drv.into(),
            outputs: Default::default(),
            system: None,
            error: None,
        }
    }

    fn eval_err(attr: &str) -> NixEvalResult {
        NixEvalResult {
            attr: attr.into(),
            drv_path: String::new(),
            outputs: Default::default(),
            system: None,
            error: Some("eval failed".into()),
        }
    }

    fn build_ok(drv: &str, out: &str) -> BuildResult {
        BuildResult::fake_ok(drv, out)
    }

    fn build_fail(drv: &str) -> BuildResult {
        BuildResult::fake_fail(drv)
    }

    fn sample_mc(hostname: &str) -> MachineConfig {
        MachineConfig {
            hostname: hostname.into(),
            machine_id: uuid::Uuid::new_v4().to_string(),
            role: "developer".into(),
            tags: vec![],
            extra_config: None,
            server_url: None,
            hardware_config: None,
            serial_number: None,
            kanidm_url: None,
            binary_cache_url: None,
        }
    }

    // ── build_closure_map ───────────────────────────────────

    #[test]
    fn test_closure_map_matches_by_drv_path() {
        let evals = vec![
            eval_ok("host-a", "/nix/store/a.drv"),
            eval_ok("host-b", "/nix/store/b.drv"),
        ];
        let builds = vec![
            build_ok("/nix/store/a.drv", "/nix/store/a-out"),
            build_ok("/nix/store/b.drv", "/nix/store/b-out"),
        ];

        let map = build_closure_map(&evals, &builds);
        assert_eq!(map.get("host-a").unwrap(), "/nix/store/a-out");
        assert_eq!(map.get("host-b").unwrap(), "/nix/store/b-out");
    }

    #[test]
    fn test_closure_map_skips_eval_errors() {
        let evals = vec![
            eval_ok("host-a", "/nix/store/a.drv"),
            eval_err("host-b"),
        ];
        let builds = vec![build_ok("/nix/store/a.drv", "/nix/store/a-out")];

        let map = build_closure_map(&evals, &builds);
        assert_eq!(map.len(), 1);
        assert!(map.contains_key("host-a"));
        assert!(!map.contains_key("host-b"));
    }

    #[test]
    fn test_closure_map_skips_failed_builds() {
        let evals = vec![
            eval_ok("host-a", "/nix/store/a.drv"),
            eval_ok("host-b", "/nix/store/b.drv"),
        ];
        let builds = vec![
            build_ok("/nix/store/a.drv", "/nix/store/a-out"),
            build_fail("/nix/store/b.drv"),
        ];

        let map = build_closure_map(&evals, &builds);
        assert_eq!(map.len(), 1);
        assert!(map.contains_key("host-a"));
    }

    #[test]
    fn test_closure_map_empty_inputs() {
        let map = build_closure_map(&[], &[]);
        assert!(map.is_empty());
    }

    // ── aggregate_instance_hash ─────────────────────────────

    #[test]
    fn test_aggregate_hash_deterministic() {
        let fleet = FleetConfig {
            machines: vec![sample_mc("host-a"), sample_mc("host-b")],
        };
        let h1 = aggregate_instance_hash(&fleet);
        let h2 = aggregate_instance_hash(&fleet);
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_aggregate_hash_changes_with_machines() {
        let fleet1 = FleetConfig {
            machines: vec![sample_mc("host-a")],
        };
        let mut mc_b = sample_mc("host-b");
        mc_b.role = "designer".into();
        let fleet2 = FleetConfig {
            machines: vec![mc_b],
        };
        assert_ne!(
            aggregate_instance_hash(&fleet1),
            aggregate_instance_hash(&fleet2)
        );
    }

    #[test]
    fn test_aggregate_hash_empty_fleet() {
        let fleet = FleetConfig { machines: vec![] };
        // Should not panic, and should produce a valid hash
        let hash = aggregate_instance_hash(&fleet);
        assert!(!hash.is_empty());
    }
}
