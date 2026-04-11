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
    /// Compliance profile to activate on this machine (cis-level1, cis-level2, stig).
    pub compliance_profile: Option<String>,
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

        // Only include machines that have been approved or are further along.
        let dominated = matches!(
            m.enrollment_status,
            hearth_common::api_types::EnrollmentStatus::Approved
                | hearth_common::api_types::EnrollmentStatus::Active
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

        // Extract compliance_profile from extra_config if present.
        let compliance_profile = m
            .extra_config
            .as_ref()
            .and_then(|c| c.get("compliance_profile"))
            .and_then(|v| v.as_str())
            .map(String::from);

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
            compliance_profile,
        });
    }

    info!(count = configs.len(), "generated fleet config");

    Ok(FleetConfig { machines: configs })
}

/// Compute a SHA-256 hash of serialized instance data for reproducibility tracking.
///
/// The hash is stable across Rust compiler versions since it uses SHA-256 over
/// the JSON representation rather than the Rust `Hash` trait.
pub fn instance_data_hash(config: &MachineConfig) -> String {
    use sha2::{Digest, Sha256};
    let json = serde_json::to_string(config).unwrap_or_default();
    let hash = Sha256::digest(json.as_bytes());
    format!("{hash:x}")
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
pub(crate) fn matches_filter(
    machine: &hearth_common::api_types::Machine,
    filter: &serde_json::Value,
) -> bool {
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use hearth_common::api_types::{EnrollmentStatus, Machine};
    use serde_json::json;
    use uuid::Uuid;

    fn sample_machine_config() -> MachineConfig {
        MachineConfig {
            hostname: "desk-001".into(),
            machine_id: Uuid::nil().to_string(),
            role: "developer".into(),
            tags: vec!["floor-3".into(), "gpu".into()],
            extra_config: None,
            server_url: Some("http://localhost:3000".into()),
            hardware_config: None,
            serial_number: Some("SN123".into()),
            kanidm_url: None,
            binary_cache_url: None,
            compliance_profile: None,
        }
    }

    fn sample_machine(id: Uuid, role: &str, tags: Vec<String>) -> Machine {
        Machine {
            id,
            hostname: "desk-001".into(),
            hardware_fingerprint: None,
            enrollment_status: EnrollmentStatus::Active,
            current_closure: None,
            target_closure: None,
            rollback_closure: None,
            role: Some(role.into()),
            tags,
            extra_config: None,
            last_heartbeat: None,
            enrolled_by: None,
            machine_token_hash: None,
            hardware_report: None,
            serial_number: None,
            hardware_config: None,
            hardware_profile: None,
            instance_data_hash: None,
            module_library_ref: None,
            headscale_ip: None,
            headscale_node_id: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    // ── instance_data_hash ──────────────────────────────────────

    #[test]
    fn test_instance_data_hash_deterministic() {
        let cfg = sample_machine_config();
        let h1 = instance_data_hash(&cfg);
        let h2 = instance_data_hash(&cfg);
        assert_eq!(h1, h2, "same config must produce the same hash");
    }

    #[test]
    fn test_instance_data_hash_changes_on_different_input() {
        let mut cfg1 = sample_machine_config();
        cfg1.role = "developer".into();
        let mut cfg2 = sample_machine_config();
        cfg2.role = "designer".into();

        assert_ne!(
            instance_data_hash(&cfg1),
            instance_data_hash(&cfg2),
            "different role must change the hash"
        );
    }

    #[test]
    fn test_instance_data_hash_changes_on_tags() {
        let mut cfg1 = sample_machine_config();
        cfg1.tags = vec!["a".into()];
        let mut cfg2 = sample_machine_config();
        cfg2.tags = vec!["b".into()];

        assert_ne!(instance_data_hash(&cfg1), instance_data_hash(&cfg2));
    }

    // ── write_build_dir ─────────────────────────────────────────

    #[test]
    fn test_write_build_dir_creates_files() {
        let dir = tempfile::tempdir().unwrap();
        let fleet = FleetConfig {
            machines: vec![sample_machine_config()],
        };

        let eval_path = write_build_dir(dir.path(), &fleet, "github:org/repo").unwrap();

        // eval.nix exists
        assert!(eval_path.exists());
        let eval_content = std::fs::read_to_string(&eval_path).unwrap();
        assert!(eval_content.contains("builtins.getFlake \"github:org/repo\""));
        assert!(eval_content.contains("\"desk-001\""));

        // Per-machine JSON exists
        let json_path = dir.path().join("desk-001.json");
        assert!(json_path.exists());
        let json: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&json_path).unwrap()).unwrap();
        assert_eq!(json["hostname"], "desk-001");
        assert_eq!(json["role"], "developer");
    }

    #[test]
    fn test_write_build_dir_empty_fleet() {
        let dir = tempfile::tempdir().unwrap();
        let fleet = FleetConfig { machines: vec![] };

        let eval_path = write_build_dir(dir.path(), &fleet, "github:org/repo").unwrap();

        let eval_content = std::fs::read_to_string(&eval_path).unwrap();
        assert!(eval_content.contains("builtins.getFlake"));
        // No machine entries between { and }
        assert!(!eval_content.contains("buildMachineConfig"));
    }

    #[test]
    fn test_write_build_dir_multiple_machines() {
        let dir = tempfile::tempdir().unwrap();
        let mut m1 = sample_machine_config();
        m1.hostname = "desk-001".into();
        let mut m2 = sample_machine_config();
        m2.hostname = "desk-002".into();
        m2.role = "designer".into();

        let fleet = FleetConfig {
            machines: vec![m1, m2],
        };

        write_build_dir(dir.path(), &fleet, "github:org/repo").unwrap();

        assert!(dir.path().join("desk-001.json").exists());
        assert!(dir.path().join("desk-002.json").exists());
    }

    // ── matches_filter ──────────────────────────────────────────

    #[test]
    fn test_matches_filter_no_filter() {
        let m = sample_machine(Uuid::new_v4(), "developer", vec![]);
        assert!(matches_filter(&m, &json!({})));
    }

    #[test]
    fn test_matches_filter_non_object() {
        let m = sample_machine(Uuid::new_v4(), "developer", vec![]);
        assert!(matches_filter(&m, &json!("not an object")));
    }

    #[test]
    fn test_matches_filter_role_match() {
        let m = sample_machine(Uuid::new_v4(), "developer", vec![]);
        assert!(matches_filter(&m, &json!({"role": "developer"})));
    }

    #[test]
    fn test_matches_filter_role_mismatch() {
        let m = sample_machine(Uuid::new_v4(), "developer", vec![]);
        assert!(!matches_filter(&m, &json!({"role": "designer"})));
    }

    #[test]
    fn test_matches_filter_tags_all_present() {
        let m = sample_machine(
            Uuid::new_v4(),
            "developer",
            vec!["gpu".into(), "floor-3".into()],
        );
        assert!(matches_filter(&m, &json!({"tags": ["gpu"]})));
        assert!(matches_filter(&m, &json!({"tags": ["gpu", "floor-3"]})));
    }

    #[test]
    fn test_matches_filter_tags_missing() {
        let m = sample_machine(Uuid::new_v4(), "developer", vec!["gpu".into()]);
        assert!(!matches_filter(&m, &json!({"tags": ["gpu", "floor-3"]})));
    }

    #[test]
    fn test_matches_filter_machine_ids() {
        let id = Uuid::new_v4();
        let other_id = Uuid::new_v4();
        let m = sample_machine(id, "developer", vec![]);

        assert!(matches_filter(
            &m,
            &json!({"machine_ids": [id.to_string()]})
        ));
        assert!(!matches_filter(
            &m,
            &json!({"machine_ids": [other_id.to_string()]})
        ));
    }

    #[test]
    fn test_matches_filter_combined() {
        let id = Uuid::new_v4();
        let m = sample_machine(id, "developer", vec!["gpu".into()]);

        // All criteria match
        assert!(matches_filter(
            &m,
            &json!({"role": "developer", "tags": ["gpu"], "machine_ids": [id.to_string()]})
        ));

        // Role mismatch fails the whole filter
        assert!(!matches_filter(
            &m,
            &json!({"role": "designer", "tags": ["gpu"]})
        ));
    }
}
