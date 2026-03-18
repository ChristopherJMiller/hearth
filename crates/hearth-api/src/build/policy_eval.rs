//! Compliance policy evaluation: generates Nix expressions to evaluate
//! compliance policies against machine NixOS configurations at build time.

use std::path::Path;

use tracing::{error, info, warn};
use uuid::Uuid;

use super::config_gen::{FleetConfig, MachineConfig};
use crate::db::CompliancePolicyRow;

/// Result of evaluating compliance policies against a single machine.
#[derive(Debug)]
pub struct PolicyEvalResult {
    pub machine_id: Uuid,
    pub policy_id: Uuid,
    pub policy_name: String,
    pub passed: bool,
    pub message: Option<String>,
}

/// Evaluate all compliance policies against each machine's NixOS config.
///
/// For each machine, generates a Nix expression that:
/// 1. Imports the machine's NixOS configuration via `buildMachineConfig`
/// 2. Evaluates each policy's `nix_expression` against `config`
/// 3. Returns a JSON object mapping policy names to booleans
///
/// Policy evaluation is non-blocking: failures are recorded but don't stop the build.
pub async fn evaluate_policies(
    build_dir: &Path,
    fleet_config: &FleetConfig,
    flake_ref: &str,
    policies: &[CompliancePolicyRow],
) -> Vec<PolicyEvalResult> {
    if policies.is_empty() {
        return Vec::new();
    }

    info!(
        policies = policies.len(),
        machines = fleet_config.machines.len(),
        "evaluating compliance policies"
    );

    let mut all_results = Vec::new();

    for machine in &fleet_config.machines {
        let machine_id: Uuid = match machine.machine_id.parse() {
            Ok(id) => id,
            Err(e) => {
                error!(machine_id = %machine.machine_id, error = %e, "invalid machine UUID");
                continue;
            }
        };

        match evaluate_machine_policies(build_dir, machine, machine_id, flake_ref, policies).await {
            Ok(results) => {
                let failed = results.iter().filter(|r| !r.passed).count();
                if failed > 0 {
                    warn!(
                        hostname = %machine.hostname,
                        failed,
                        total = results.len(),
                        "policy violations detected"
                    );
                }
                all_results.extend(results);
            }
            Err(e) => {
                error!(
                    hostname = %machine.hostname,
                    error = %e,
                    "failed to evaluate policies for machine"
                );
                // Record all policies as failed for this machine.
                for policy in policies {
                    all_results.push(PolicyEvalResult {
                        machine_id,
                        policy_id: policy.id,
                        policy_name: policy.name.clone(),
                        passed: false,
                        message: Some(format!("evaluation error: {e}")),
                    });
                }
            }
        }
    }

    info!(
        total = all_results.len(),
        passed = all_results.iter().filter(|r| r.passed).count(),
        failed = all_results.iter().filter(|r| !r.passed).count(),
        "policy evaluation complete"
    );

    all_results
}

/// Evaluate all policies against a single machine's NixOS configuration.
async fn evaluate_machine_policies(
    build_dir: &Path,
    machine: &MachineConfig,
    machine_id: Uuid,
    flake_ref: &str,
    policies: &[CompliancePolicyRow],
) -> Result<Vec<PolicyEvalResult>, String> {
    let json_path = build_dir.join(format!("{}.json", machine.hostname));

    // Generate a Nix expression that evaluates all policies in one shot.
    // Each policy expression is evaluated in the context of the machine's NixOS config.
    let mut nix_expr = String::new();
    nix_expr.push_str("let\n");
    nix_expr.push_str(&format!("  flake = builtins.getFlake \"{flake_ref}\";\n"));
    nix_expr.push_str(&format!(
        "  machineModule = flake.lib.buildMachineConfig {{ instanceDataPath = \"{}\"; }};\n",
        json_path.to_string_lossy()
    ));
    nix_expr.push_str("  config = machineModule.config;\n");
    nix_expr.push_str("in {\n");

    for policy in policies {
        // Sanitize policy name for use as a Nix attribute name.
        let attr_name = sanitize_nix_attr(&policy.name);
        // Wrap the expression in a try-catch to prevent one bad policy from
        // breaking the entire evaluation.
        nix_expr.push_str(&format!(
            "  \"{attr_name}\" = builtins.tryEval ({});\n",
            policy.nix_expression
        ));
    }

    nix_expr.push_str("}\n");

    // Write the evaluation expression to a temp file.
    let eval_file = build_dir.join(format!("policy-eval-{}.nix", machine.hostname));
    std::fs::write(&eval_file, &nix_expr)
        .map_err(|e| format!("failed to write policy eval file: {e}"))?;

    // Run nix eval --json.
    let output = tokio::process::Command::new("nix")
        .args([
            "eval",
            "--json",
            "--expr",
            &format!("import {}", eval_file.to_string_lossy()),
            "--impure",
        ])
        .output()
        .await
        .map_err(|e| format!("failed to spawn nix eval: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("nix eval failed: {stderr}"));
    }

    // Parse the JSON result: { "policy-name": { "success": bool, "value": bool }, ... }
    let result_json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("failed to parse nix eval output: {e}"))?;

    let mut results = Vec::new();

    for policy in policies {
        let attr_name = sanitize_nix_attr(&policy.name);
        let (passed, message) = match result_json.get(&attr_name) {
            Some(val) => {
                let success = val
                    .get("success")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                if !success {
                    (false, Some("policy expression evaluation failed".into()))
                } else {
                    let value = val.get("value").and_then(|v| v.as_bool()).unwrap_or(false);
                    if value {
                        (true, None)
                    } else {
                        (
                            false,
                            Some(format!("policy check failed: {}", policy.nix_expression)),
                        )
                    }
                }
            }
            None => (false, Some("policy not found in evaluation output".into())),
        };

        results.push(PolicyEvalResult {
            machine_id,
            policy_id: policy.id,
            policy_name: policy.name.clone(),
            passed,
            message,
        });
    }

    Ok(results)
}

/// Sanitize a string for use as a Nix attribute name.
fn sanitize_nix_attr(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_nix_attr() {
        assert_eq!(sanitize_nix_attr("firewall-enabled"), "firewall-enabled");
        assert_eq!(sanitize_nix_attr("CIS 3.4.1"), "CIS-3-4-1");
        assert_eq!(sanitize_nix_attr("stig_v_230223"), "stig_v_230223");
    }
}
