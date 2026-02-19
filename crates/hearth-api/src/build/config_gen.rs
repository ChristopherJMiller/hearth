//! Configuration generator: queries machine inventory from the DB and produces
//! per-machine JSON config that feeds `mkFleetHost` evaluation.

use serde::Serialize;
use sqlx::PgPool;
use tracing::info;

use crate::repo;

/// Per-machine configuration for `mkFleetHost`.
#[derive(Debug, Clone, Serialize)]
pub struct MachineConfig {
    pub hostname: String,
    pub machine_id: String,
    pub role: String,
    pub tags: Vec<String>,
    pub extra_config: Option<serde_json::Value>,
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
        });
    }

    info!(count = configs.len(), "generated fleet config");

    Ok(FleetConfig { machines: configs })
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
