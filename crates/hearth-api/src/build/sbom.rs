//! SBOM generation: produces CycloneDX JSON SBOMs for built NixOS closures
//! using `sbomnix`.

use std::path::{Path, PathBuf};

use tracing::{error, info, warn};

/// Default directory for SBOM storage.
const DEFAULT_SBOM_DIR: &str = "/var/lib/hearth/sboms";

/// Get the SBOM base directory from env or default.
pub fn sbom_base_dir() -> String {
    std::env::var("HEARTH_SBOM_DIR").unwrap_or_else(|_| DEFAULT_SBOM_DIR.to_string())
}

/// Errors from SBOM generation.
#[derive(Debug, thiserror::Error)]
pub enum SbomError {
    #[error("failed to spawn sbomnix: {0}")]
    SpawnFailed(std::io::Error),
    #[error("sbomnix exited with error: {0}")]
    ExitFailed(String),
    #[error("sbomnix not found in PATH")]
    NotInstalled,
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Generate a CycloneDX JSON SBOM for a Nix store path.
///
/// Runs `sbomnix <store_path> --cdx <output_path>` and returns the path
/// to the generated SBOM file.
///
/// Returns `Err` if sbomnix is not installed or fails; the caller should
/// treat SBOM failures as non-blocking.
pub async fn generate_sbom(
    store_path: &str,
    output_dir: &Path,
    hostname: &str,
) -> Result<PathBuf, SbomError> {
    // Ensure output directory exists.
    tokio::fs::create_dir_all(output_dir)
        .await
        .map_err(SbomError::Io)?;

    let output_path = output_dir.join(format!("{hostname}.cdx.json"));

    let output = tokio::process::Command::new("sbomnix")
        .args([store_path, "--cdx", &output_path.to_string_lossy()])
        .output()
        .await
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                SbomError::NotInstalled
            } else {
                SbomError::SpawnFailed(e)
            }
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(SbomError::ExitFailed(stderr.to_string()));
    }

    Ok(output_path)
}

/// Generate SBOMs for all built closures in a deployment.
///
/// Returns a list of (hostname, relative_sbom_path) for successfully generated SBOMs.
/// Failures are logged but do not stop the deployment.
pub async fn generate_deployment_sboms(
    closure_map: &std::collections::HashMap<String, String>,
    deployment_id: uuid::Uuid,
    sbom_base_dir: &Path,
) -> Vec<(String, String)> {
    let deployment_dir = sbom_base_dir.join(deployment_id.to_string());

    let mut results = Vec::new();

    for (hostname, store_path) in closure_map {
        match generate_sbom(store_path, &deployment_dir, hostname).await {
            Ok(_path) => {
                let relative = format!("{deployment_id}/{hostname}.cdx.json");
                info!(hostname, store_path, "SBOM generated");
                results.push((hostname.clone(), relative));
            }
            Err(SbomError::NotInstalled) => {
                warn!("sbomnix not installed, skipping SBOM generation");
                return results;
            }
            Err(e) => {
                error!(hostname, error = %e, "failed to generate SBOM");
            }
        }
    }

    info!(
        generated = results.len(),
        total = closure_map.len(),
        "SBOM generation complete"
    );

    results
}
