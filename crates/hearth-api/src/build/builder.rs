//! Runs `nix build` on derivations, supports parallel builds via `JoinSet`.

use tokio::process::Command;
use tokio::task::JoinSet;
use tracing::{debug, error, info, warn};

/// Errors from the build process.
#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    #[error("nix build failed for {drv}: exit code {code}")]
    BuildFailed { drv: String, code: i32 },
    #[error("nix build failed to start: {0}")]
    SpawnFailed(#[source] std::io::Error),
    #[error("join error: {0}")]
    JoinError(#[source] tokio::task::JoinError),
}

/// Result of building a single derivation.
#[derive(Debug, Clone)]
pub struct BuildResult {
    pub drv_path: String,
    pub out_path: String,
    pub success: bool,
    pub error: Option<String>,
}

/// Build a single derivation.
pub async fn build_derivation(drv_path: &str) -> Result<BuildResult, BuildError> {
    debug!(drv = drv_path, "building derivation");

    let output = Command::new("nix")
        .args(["build", "--no-link", "--print-out-paths", drv_path])
        .output()
        .await
        .map_err(BuildError::SpawnFailed)?;

    if output.status.success() {
        let out_path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        debug!(drv = drv_path, out = %out_path, "build succeeded");
        Ok(BuildResult {
            drv_path: drv_path.to_string(),
            out_path,
            success: true,
            error: None,
        })
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let code = output.status.code().unwrap_or(-1);
        error!(drv = drv_path, code, stderr = %stderr, "build failed");
        Ok(BuildResult {
            drv_path: drv_path.to_string(),
            out_path: String::new(),
            success: false,
            error: Some(stderr),
        })
    }
}

/// Build multiple derivations in parallel.
///
/// `max_parallel` controls the concurrency limit. Returns results for all
/// derivations, including failures.
pub async fn build_all(
    drv_paths: &[String],
    max_parallel: usize,
) -> Result<Vec<BuildResult>, BuildError> {
    info!(
        count = drv_paths.len(),
        max_parallel, "starting parallel builds"
    );

    let mut results = Vec::with_capacity(drv_paths.len());
    let mut set = JoinSet::new();
    let mut pending = drv_paths.iter().cloned().peekable();

    // Seed the initial batch.
    for _ in 0..max_parallel {
        if let Some(drv) = pending.next() {
            set.spawn(async move { build_derivation(&drv).await });
        }
    }

    while let Some(join_result) = set.join_next().await {
        let build_result = join_result.map_err(BuildError::JoinError)??;
        if !build_result.success {
            warn!(drv = %build_result.drv_path, "build failed, continuing");
        }
        results.push(build_result);

        // Spawn next if available.
        if let Some(drv) = pending.next() {
            set.spawn(async move { build_derivation(&drv).await });
        }
    }

    let succeeded = results.iter().filter(|r| r.success).count();
    let failed = results.len() - succeeded;
    info!(total = results.len(), succeeded, failed, "builds complete");

    Ok(results)
}
