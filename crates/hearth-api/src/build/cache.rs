//! Pushes store paths to an Attic binary cache.

use std::future::Future;
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

/// Sign a store path with the cache signing key (if configured).
async fn sign_path(store_path: &str) {
    if let Ok(key_file) = std::env::var("HEARTH_CACHE_SIGNING_KEY") {
        let _ = Command::new("nix")
            .args(["store", "sign", "--key-file", &key_file, store_path])
            .output()
            .await;
    }
}

/// Push a single store path to the Attic cache.
pub async fn push_to_cache(cache_name: &str, store_path: &str) -> Result<(), CacheError> {
    debug!(cache = cache_name, path = store_path, "pushing to Attic");

    // Sign the path before pushing (if signing key is configured).
    sign_path(store_path).await;

    // Attic pushes the full closure (all transitive deps) by default.
    // Deduplication ensures paths already in the cache are skipped. Shared
    // deps (e.g., LibreOffice, Firefox) are cached after the first user
    // build and pulled instantly for subsequent users.
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
    let cache_name = cache_name.to_string();
    push_all_with(store_paths, |path| {
        let cn = cache_name.clone();
        async move { push_to_cache(&cn, &path).await }
    })
    .await
}

/// Push multiple store paths using a custom push function.
///
/// Core implementation that accepts an injectable pusher for testability.
pub(crate) async fn push_all_with<F, Fut>(store_paths: &[String], push_fn: F) -> usize
where
    F: Fn(String) -> Fut,
    Fut: Future<Output = Result<(), CacheError>>,
{
    info!(count = store_paths.len(), "pushing store paths to cache");

    let mut pushed = 0;
    for path in store_paths {
        match push_fn(path.clone()).await {
            Ok(()) => pushed += 1,
            Err(e) => {
                tracing::warn!(path = %path, error = %e, "failed to push to cache, continuing");
            }
        }
    }

    info!(pushed, total = store_paths.len(), "cache push complete");
    pushed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_push_all_empty() {
        let count = push_all_with(&[], |_| async { Ok(()) }).await;
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_push_all_all_succeed() {
        let paths: Vec<String> = vec![
            "/nix/store/a".into(),
            "/nix/store/b".into(),
            "/nix/store/c".into(),
        ];
        let count = push_all_with(&paths, |_| async { Ok(()) }).await;
        assert_eq!(count, 3);
    }

    #[tokio::test]
    async fn test_push_all_partial_failure() {
        let paths: Vec<String> = vec![
            "/nix/store/a".into(),
            "/nix/store/bad".into(),
            "/nix/store/c".into(),
        ];
        let count = push_all_with(&paths, |path| async move {
            if path.contains("bad") {
                Err(CacheError::PushFailed {
                    code: 1,
                    stderr: "push failed".into(),
                })
            } else {
                Ok(())
            }
        })
        .await;
        assert_eq!(count, 2);
    }

    #[tokio::test]
    async fn test_push_all_all_fail() {
        let paths: Vec<String> = vec!["/nix/store/a".into(), "/nix/store/b".into()];
        let count = push_all_with(&paths, |_| async {
            Err(CacheError::PushFailed {
                code: 1,
                stderr: "error".into(),
            })
        })
        .await;
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_push_all_tracks_correct_paths() {
        let pushed_paths = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let paths: Vec<String> = vec!["/nix/store/x".into(), "/nix/store/y".into()];

        let pp = pushed_paths.clone();
        push_all_with(&paths, move |path| {
            let pp = pp.clone();
            async move {
                pp.lock().unwrap().push(path);
                Ok(())
            }
        })
        .await;

        let recorded = pushed_paths.lock().unwrap();
        assert_eq!(*recorded, vec!["/nix/store/x", "/nix/store/y"]);
    }
}
