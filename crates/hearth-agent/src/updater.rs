//! System update logic.
//!
//! Applies NixOS system updates by copying the target closure from a binary
//! cache (if configured), setting the system profile, and running
//! `switch-to-configuration switch`.

use std::env;
use std::path::Path;

use tracing::{debug, error, info, warn};

/// Errors that can occur during an update attempt.
#[derive(Debug, thiserror::Error)]
pub enum UpdateError {
    #[error("update command failed: {0}")]
    CommandFailed(String),
    #[error("invalid store path: {0}")]
    InvalidStorePath(String),
}

/// Compare the current system closure with the target and, if they differ,
/// apply the update.
///
/// Returns `Ok(true)` if an update was applied, `Ok(false)` if no update was
/// needed.
///
/// Steps:
/// 1. If `current_closure` equals `target_closure`, return early.
/// 2. Optionally copy the closure from a binary cache (`HEARTH_CACHE_URL`).
/// 3. Set the system profile to the target closure.
/// 4. Run `switch-to-configuration switch` from the target closure.
pub async fn check_and_apply_update(
    current_closure: Option<&str>,
    target_closure: &str,
    cache_url: Option<&str>,
) -> Result<bool, UpdateError> {
    // If we already have the target closure, nothing to do.
    if let Some(current) = current_closure {
        if current == target_closure {
            debug!(
                closure = target_closure,
                "system is already at the target closure"
            );
            return Ok(false);
        }

        info!(
            from = current,
            to = target_closure,
            "update available: switching closure"
        );
    } else {
        info!(
            to = target_closure,
            "update available: no current closure recorded, switching"
        );
    }

    // Validate the store path format.
    if !hearth_common::nix_store::is_valid_store_path(target_closure) {
        return Err(UpdateError::InvalidStorePath(target_closure.to_string()));
    }

    // Step 1: Optionally copy the closure from a binary cache.
    let effective_cache_url = cache_url
        .map(String::from)
        .or_else(|| env::var("HEARTH_CACHE_URL").ok().filter(|s| !s.is_empty()));
    if let Some(cache) = &effective_cache_url {
        info!(cache = %cache, closure = %target_closure, "copying closure from cache");
        let netrc_path = Path::new("/run/hearth/netrc");
        let mut args = vec!["copy", "--from", cache, target_closure];
        if netrc_path.exists() {
            args.extend_from_slice(&["--option", "netrc-file", "/run/hearth/netrc"]);
        }
        run_command("nix", &args).await?;
    } else {
        debug!("no cache URL configured, assuming closure is already in the local store");
    }

    // Step 2: Set the system profile to the target closure.
    info!(closure = %target_closure, "setting system profile");
    run_command(
        "nix-env",
        &[
            "--profile",
            "/nix/var/nix/profiles/system",
            "--set",
            target_closure,
        ],
    )
    .await?;

    // Step 3: Switch to the new configuration.
    let switch_bin = format!("{target_closure}/bin/switch-to-configuration");
    info!(switch_bin = %switch_bin, "switching to new configuration");
    run_command(&switch_bin, &["switch"]).await?;

    info!(closure = %target_closure, "update applied successfully");
    Ok(true)
}

/// Run a command with the given arguments, logging stdout/stderr.
///
/// Returns `Ok(())` on success or `Err(UpdateError::CommandFailed)` if the
/// command fails or cannot be started.
async fn run_command(cmd: &str, args: &[&str]) -> Result<(), UpdateError> {
    debug!(cmd = %cmd, ?args, "running command");

    let output = tokio::process::Command::new(cmd)
        .args(args)
        .output()
        .await
        .map_err(|e| UpdateError::CommandFailed(format!("failed to start {cmd}: {e}")))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if !stdout.is_empty() {
        debug!(cmd = %cmd, %stdout, "command stdout");
    }
    if !stderr.is_empty() {
        if output.status.success() {
            debug!(cmd = %cmd, %stderr, "command stderr");
        } else {
            error!(cmd = %cmd, %stderr, "command stderr");
        }
    }

    if output.status.success() {
        debug!(cmd = %cmd, "command succeeded");
        Ok(())
    } else {
        let code = output
            .status
            .code()
            .map(|c| c.to_string())
            .unwrap_or_else(|| "signal".to_string());
        let msg = if stderr.is_empty() {
            format!("{cmd} exited with code {code}")
        } else {
            format!("{cmd} exited with code {code}: {stderr}")
        };
        warn!(cmd = %cmd, %code, "command failed");
        Err(UpdateError::CommandFailed(msg))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // These tests exercise the comparison / early-return logic.
    // The actual nix commands will not run in test environments;
    // integration tests for the full update flow are done via NixOS VM tests.

    #[tokio::test]
    async fn no_update_when_same() {
        let result = check_and_apply_update(
            Some("/nix/store/aaaa-system"),
            "/nix/store/aaaa-system",
            None,
        )
        .await
        .unwrap();
        assert!(!result);
    }

    #[tokio::test]
    async fn rejects_invalid_store_path() {
        let result = check_and_apply_update(
            Some("/nix/store/aaaa-system"),
            "/tmp/not-a-store-path",
            None,
        )
        .await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(err, UpdateError::InvalidStorePath(_)),
            "expected InvalidStorePath, got: {err}"
        );
    }
}
