//! Configuration generator: queries machine inventory from the DB and produces
//! per-machine JSON config that feeds `mkFleetHost` evaluation.

use serde::Serialize;
use sqlx::PgPool;
use tracing::info;

use crate::repo;

/// Per-machine instance data for `buildMachineConfig`.
///
/// This struct is serialized to JSON and written to a per-machine file that
/// the Nix evaluator reads via `builtins.readFile` + `builtins.fromJSON`.
#[derive(Debug, Clone, Serialize)]
pub struct MachineConfig {
    pub hostname: String,
    pub machine_id: String,
    pub role: String,
    pub tags: Vec<String>,
    pub extra_config: Option<serde_json::Value>,
    /// Server URL for the agent to connect to after provisioning.
    pub server_url: Option<String>,
    /// Raw NixOS hardware-configuration.nix content from the device.
    pub hardware_config: Option<String>,
    /// Device serial number for asset tracking.
    pub serial_number: Option<String>,
    /// Kanidm URL for identity integration.
    pub kanidm_url: Option<String>,
    /// Binary cache URL for pulling closures.
    pub binary_cache_url: Option<String>,
}

/// Fleet-wide configuration for a build evaluation.
#[derive(Debug, Clone, Serialize)]
pub struct FleetConfig {
    pub machines: Vec<MachineConfig>,
}

/// Generate fleet configuration from the database.
///
/// Queries all active/enrolled machines and produces a JSON-serializable
/// config that can be fed to `nix eval` for `mkFleetHost`.
pub async fn generate_fleet_config(
    pool: &PgPool,
    target_filter: Option<&serde_json::Value>,
) -> Result<FleetConfig, sqlx::Error> {
    let machines = repo::list_machines(pool).await?;

    let mut configs: Vec<MachineConfig> = Vec::new();

    for m in machines {
        let m: hearth_common::api_types::Machine = m.into();

        // Only include active/enrolled machines
        let dominated = matches!(
            m.enrollment_status,
            hearth_common::api_types::EnrollmentStatus::Active
                | hearth_common::api_types::EnrollmentStatus::Enrolled
                | hearth_common::api_types::EnrollmentStatus::Provisioning
        );
        if !dominated {
            continue;
        }

        // Apply target filter if provided
        if let Some(filter) = target_filter
            && !matches_filter(&m, filter)
        {
            continue;
        }

        configs.push(MachineConfig {
            hostname: m.hostname,
            machine_id: m.id.to_string(),
            role: m.role.unwrap_or_else(|| "default".to_string()),
            tags: m.tags,
            extra_config: m.extra_config,
            server_url: None, // Set by orchestrator from env/config
            hardware_config: m.hardware_config,
            serial_number: m.serial_number,
            kanidm_url: None,       // Set by orchestrator from env/config
            binary_cache_url: None, // Set by orchestrator from env/config
        });
    }

    info!(count = configs.len(), "generated fleet config");

    Ok(FleetConfig { machines: configs })
}

/// Compute a SHA-256 hash of serialized instance data for reproducibility tracking.
pub fn instance_data_hash(config: &MachineConfig) -> String {
    use std::hash::{Hash, Hasher};
    // Use the JSON representation for a stable hash
    let json = serde_json::to_string(config).unwrap_or_default();
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    json.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// Write per-machine instance data JSON files and a top-level eval.nix to a build
/// directory. The eval.nix is the entry point for `nix-eval-jobs --expr`.
///
/// Returns the path to eval.nix.
pub fn write_build_dir(
    build_dir: &std::path::Path,
    fleet_config: &FleetConfig,
    flake_ref: &str,
) -> Result<std::path::PathBuf, std::io::Error> {
    std::fs::create_dir_all(build_dir)?;

    // Write per-machine JSON files
    for mc in &fleet_config.machines {
        let json_path = build_dir.join(format!("{}.json", mc.hostname));
        let json = serde_json::to_string_pretty(mc).map_err(std::io::Error::other)?;
        std::fs::write(&json_path, json)?;
    }

    // Generate eval.nix that produces nixosConfigurations.<hostname> for each machine.
    // nix-eval-jobs will evaluate this attribute set in a single process, sharing
    // thunk evaluation across machines for maximum efficiency.
    let mut eval_nix = String::from("let\n");
    eval_nix.push_str(&format!("  flake = builtins.getFlake \"{flake_ref}\";\n"));
    eval_nix.push_str("in {\n");

    for mc in &fleet_config.machines {
        let json_path = build_dir.join(format!("{}.json", mc.hostname));
        let json_path_str = json_path.to_string_lossy();
        eval_nix.push_str(&format!(
            "  \"{}\" = (flake.lib.buildMachineConfig {{ instanceDataPath = \"{json_path_str}\"; }}).config.system.build.toplevel;\n",
            mc.hostname
        ));
    }

    eval_nix.push_str("}\n");

    let eval_path = build_dir.join("eval.nix");
    std::fs::write(&eval_path, eval_nix)?;

    info!(
        build_dir = %build_dir.display(),
        machines = fleet_config.machines.len(),
        "wrote build directory with eval.nix"
    );

    Ok(eval_path)
}

/// Check if a machine matches a target filter.
///
/// The filter is a JSON object with optional keys:
/// - `role`: string — machine must have this role
/// - `tags`: array of strings — machine must have all these tags
/// - `machine_ids`: array of UUIDs — machine must be in this list
fn matches_filter(machine: &hearth_common::api_types::Machine, filter: &serde_json::Value) -> bool {
    let obj = match filter.as_object() {
        Some(o) => o,
        None => return true,
    };

    if obj.is_empty() {
        return true;
    }

    if let Some(role) = obj.get("role").and_then(|v| v.as_str())
        && machine.role.as_deref() != Some(role)
    {
        return false;
    }

    if let Some(tags) = obj.get("tags").and_then(|v| v.as_array()) {
        for tag in tags {
            if let Some(t) = tag.as_str()
                && !machine.tags.contains(&t.to_string())
            {
                return false;
            }
        }
    }

    if let Some(ids) = obj.get("machine_ids").and_then(|v| v.as_array()) {
        let id_str = machine.id.to_string();
        let found = ids.iter().any(|v| v.as_str() == Some(&id_str));
        if !found {
            return false;
        }
    }

    true
}
