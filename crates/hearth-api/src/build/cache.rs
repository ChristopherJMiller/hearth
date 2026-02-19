//! Pushes store paths to an Attic binary cache.

use tokio::process::Command;
use tracing::{debug, error, info};

/// Errors from cache operations.
#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    #[error("attic push failed: exit code {code}: {stderr}")]
    PushFailed { code: i32, stderr: String },
    #[error("attic command failed to start: {0}")]
    SpawnFailed(#[source] std::io::Error),
}

/// Push a single store path to the Attic cache.
pub async fn push_to_cache(cache_name: &str, store_path: &str) -> Result<(), CacheError> {
    debug!(cache = cache_name, path = store_path, "pushing to Attic");

    let output = Command::new("attic")
        .args(["push", cache_name, store_path])
        .output()
        .await
        .map_err(CacheError::SpawnFailed)?;

    if output.status.success() {
        debug!(path = store_path, "pushed to cache");
        Ok(())
    } else {
        let code = output.status.code().unwrap_or(-1);
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        error!(path = store_path, code, %stderr, "cache push failed");
        Err(CacheError::PushFailed { code, stderr })
    }
}

/// Push multiple store paths to the Attic cache.
///
/// Continues on individual failures, logging warnings. Returns the count of
/// successfully pushed paths.
pub async fn push_all(cache_name: &str, store_paths: &[String]) -> usize {
    info!(
        cache = cache_name,
        count = store_paths.len(),
        "pushing store paths to cache"
    );

    let mut pushed = 0;
    for path in store_paths {
        match push_to_cache(cache_name, path).await {
            Ok(()) => pushed += 1,
            Err(e) => {
                tracing::warn!(path = %path, error = %e, "failed to push to cache, continuing");
            }
        }
    }

    info!(pushed, total = store_paths.len(), "cache push complete");
    pushed
}
