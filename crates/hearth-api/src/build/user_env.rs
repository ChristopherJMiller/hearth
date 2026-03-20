//! Per-user environment closure builder.
//!
//! Builds a home-manager closure for a specific user by composing their
//! base role template with per-user overrides. The closure is pushed to the
//! Attic binary cache and the store path is returned.

use std::path::Path;

use serde::Serialize;
use tracing::{error, info};

use crate::db::UserConfigRow;

/// Per-user config written as JSON for the Nix expression to consume.
#[derive(Serialize)]
struct UserConfigJson {
    username: String,
    base_role: String,
    overrides: serde_json::Value,
}

/// Build a per-user home-manager closure from a `UserConfigRow`.
///
/// 1. Writes `user-config.json` to a temp directory.
/// 2. Writes `user-eval.nix` that calls `flake.lib.buildUserEnv`.
/// 3. Runs `nix build` to produce the closure.
/// 4. Optionally pushes to the Attic cache.
/// 5. Returns the store path.
pub async fn build_user_env(
    config: &UserConfigRow,
    flake_ref: &str,
    cache_url: Option<&str>,
    attic_token: Option<&str>,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    // Validate flake_ref to prevent Nix expression injection. Only allow
    // characters valid in flake references (alphanumeric, colons, slashes,
    // dots, hyphens, underscores, plus signs).
    if !flake_ref
        .chars()
        .all(|c| c.is_alphanumeric() || ":/.@_-+#".contains(c))
    {
        return Err(format!("invalid flake_ref: contains disallowed characters: {flake_ref}").into());
    }

    let build_dir = tempfile::tempdir()?;
    let build_path = build_dir.path();

    // Write user config JSON.
    let user_config = UserConfigJson {
        username: config.username.clone(),
        base_role: config.base_role.clone(),
        overrides: config.overrides.clone(),
    };
    let config_json_path = build_path.join("user-config.json");
    let config_json = serde_json::to_string_pretty(&user_config)?;
    tokio::fs::write(&config_json_path, &config_json).await?;

    // Write the eval.nix that calls the flake's buildUserEnv.
    let eval_nix = format!(
        r#"let
  flake = builtins.getFlake "{flake_ref}";
in
  flake.lib.buildUserEnv {{ userConfigPath = "{config_path}"; }}"#,
        flake_ref = flake_ref,
        config_path = config_json_path.display(),
    );
    let eval_nix_path = build_path.join("user-eval.nix");
    tokio::fs::write(&eval_nix_path, &eval_nix).await?;

    info!(
        username = %config.username,
        base_role = %config.base_role,
        build_dir = %build_path.display(),
        "building per-user home-manager closure"
    );

    // Build the closure.
    let output = tokio::process::Command::new("nix")
        .args([
            "build",
            "--no-link",
            "--print-out-paths",
            "--impure",
            "-f",
            eval_nix_path.to_str().unwrap(),
        ])
        .output()
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        error!(
            username = %config.username,
            %stderr,
            "nix build failed for per-user closure"
        );
        return Err(format!("nix build failed: {stderr}").into());
    }

    let closure = String::from_utf8(output.stdout)?.trim().to_string();
    if closure.is_empty() {
        return Err("nix build produced no output path".into());
    }

    info!(
        username = %config.username,
        %closure,
        "per-user closure built successfully"
    );

    // Push to Attic cache if configured.
    if let (Some(cache), Some(token)) = (cache_url, attic_token) {
        push_to_cache(&closure, cache, token).await;
    }

    Ok(closure)
}

/// Push a closure to the Attic binary cache. Best-effort; logs warnings on failure.
async fn push_to_cache(closure: &str, cache_url: &str, token: &str) {
    info!(%closure, %cache_url, "pushing per-user closure to cache");

    let result = tokio::process::Command::new("attic")
        .args(["push", cache_url, closure])
        .env("ATTIC_TOKEN", token)
        .output()
        .await;

    match result {
        Ok(out) if out.status.success() => {
            info!(%closure, "pushed per-user closure to cache");
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            tracing::warn!(%closure, %stderr, "attic push returned non-zero");
        }
        Err(e) => {
            tracing::warn!(%closure, error = %e, "failed to run attic push");
        }
    }
}

/// Validate that a Nix store path looks reasonable.
pub fn is_valid_store_path(path: &str) -> bool {
    Path::new(path).starts_with("/nix/store/") && path.len() > 44
}
