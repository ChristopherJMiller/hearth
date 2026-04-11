//! Runs `nix build` on derivations, supports parallel builds via `JoinSet`.

use std::future::Future;
use tokio::process::Command;
use tokio::task::JoinSet;
use tracing::{debug, error, info, warn};

/// Errors from the build process.
#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    #[allow(dead_code)]
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
    #[allow(dead_code)]
    pub error: Option<String>,
}

#[cfg(test)]
impl BuildResult {
    pub(crate) fn fake_ok(drv: &str, out: &str) -> Self {
        Self {
            drv_path: drv.to_string(),
            out_path: out.to_string(),
            success: true,
            error: None,
        }
    }

    pub(crate) fn fake_fail(drv: &str) -> Self {
        Self {
            drv_path: drv.to_string(),
            out_path: String::new(),
            success: false,
            error: Some("build failed".into()),
        }
    }
}

/// Build a single derivation using `nix build`.
pub async fn build_derivation(drv_path: &str) -> Result<BuildResult, BuildError> {
    debug!(drv = drv_path, "building derivation");

    let mut args = vec![
        "build".to_string(),
        "--no-link".into(),
        "--print-out-paths".into(),
    ];
    args.extend(super::nix_extra_args());
    args.push(drv_path.into());

    let output = Command::new("nix")
        .args(&args)
        .output()
        .await
        .map_err(BuildError::SpawnFailed)?;

    if output.status.success() {
        let raw_out = String::from_utf8_lossy(&output.stdout).trim().to_string();
        // nix build --print-out-paths may return the .drv path or be empty.
        // Use nix-store --realise to ensure the output path exists in the local
        // store and get the actual output path.
        let out_path = if raw_out.ends_with(".drv") || raw_out.is_empty() {
            let realise = Command::new("nix-store")
                .args(["--realise", drv_path])
                .output()
                .await
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
            realise.unwrap_or(raw_out)
        } else {
            raw_out
        };
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
    build_all_with(drv_paths, max_parallel, |drv| async move {
        build_derivation(&drv).await
    })
    .await
}

/// Build multiple derivations in parallel using a custom builder function.
///
/// This is the core implementation that accepts an injectable builder,
/// enabling testing of the concurrency logic without the `nix` binary.
pub(crate) async fn build_all_with<F, Fut>(
    drv_paths: &[String],
    max_parallel: usize,
    builder_fn: F,
) -> Result<Vec<BuildResult>, BuildError>
where
    F: Fn(String) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<BuildResult, BuildError>> + Send + 'static,
{
    info!(
        count = drv_paths.len(),
        max_parallel, "starting parallel builds"
    );

    let builder_fn = std::sync::Arc::new(builder_fn);
    let mut results = Vec::with_capacity(drv_paths.len());
    let mut set = JoinSet::new();
    let mut pending = drv_paths.iter().cloned().peekable();

    // Seed the initial batch.
    for _ in 0..max_parallel {
        if let Some(drv) = pending.next() {
            let f = builder_fn.clone();
            set.spawn(async move { f(drv).await });
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
            let f = builder_fn.clone();
            set.spawn(async move { f(drv).await });
        }
    }

    let succeeded = results.iter().filter(|r| r.success).count();
    let failed = results.len() - succeeded;
    info!(total = results.len(), succeeded, failed, "builds complete");

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn ok_result(drv: &str) -> BuildResult {
        BuildResult::fake_ok(drv, &format!("/nix/store/{drv}-out"))
    }

    fn fail_result(drv: &str) -> BuildResult {
        BuildResult::fake_fail(drv)
    }

    #[tokio::test]
    async fn test_build_all_empty() {
        let results = build_all_with(&[], 4, |drv| async move { Ok(ok_result(&drv)) })
            .await
            .unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_build_all_single() {
        let paths = vec!["a.drv".into()];
        let results = build_all_with(&paths, 4, |drv| async move { Ok(ok_result(&drv)) })
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].success);
        assert_eq!(results[0].drv_path, "a.drv");
    }

    #[tokio::test]
    async fn test_build_all_partial_failure() {
        let paths: Vec<String> = vec!["good.drv".into(), "bad.drv".into(), "good2.drv".into()];
        let results = build_all_with(&paths, 4, |drv| async move {
            if drv.contains("bad") {
                Ok(fail_result(&drv))
            } else {
                Ok(ok_result(&drv))
            }
        })
        .await
        .unwrap();

        assert_eq!(results.len(), 3);
        let succeeded = results.iter().filter(|r| r.success).count();
        let failed = results.iter().filter(|r| !r.success).count();
        assert_eq!(succeeded, 2);
        assert_eq!(failed, 1);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_build_all_respects_max_parallel() {
        let concurrent = std::sync::Arc::new(AtomicUsize::new(0));
        let max_seen = std::sync::Arc::new(AtomicUsize::new(0));

        let paths: Vec<String> = (0..8).map(|i| format!("{i}.drv")).collect();

        let c = concurrent.clone();
        let m = max_seen.clone();
        let results = build_all_with(&paths, 2, move |drv| {
            let c = c.clone();
            let m = m.clone();
            async move {
                let current = c.fetch_add(1, Ordering::SeqCst) + 1;
                m.fetch_max(current, Ordering::SeqCst);
                // Yield to let other tasks run
                tokio::task::yield_now().await;
                c.fetch_sub(1, Ordering::SeqCst);
                Ok(ok_result(&drv))
            }
        })
        .await
        .unwrap();

        assert_eq!(results.len(), 8);
        // Max concurrency should not exceed 2
        assert!(
            max_seen.load(Ordering::SeqCst) <= 2,
            "max concurrent was {}, expected <= 2",
            max_seen.load(Ordering::SeqCst)
        );
    }

    #[tokio::test]
    async fn test_build_all_all_fail() {
        let paths: Vec<String> = vec!["a.drv".into(), "b.drv".into()];
        let results = build_all_with(&paths, 4, |drv| async move { Ok(fail_result(&drv)) })
            .await
            .unwrap();

        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|r| !r.success));
    }

    #[tokio::test]
    async fn test_build_all_spawn_error_propagates() {
        let paths: Vec<String> = vec!["a.drv".into()];
        let result = build_all_with(&paths, 4, |_drv| async move {
            Err(BuildError::SpawnFailed(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "nix not found",
            )))
        })
        .await;

        assert!(result.is_err());
    }
}
