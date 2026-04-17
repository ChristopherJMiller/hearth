use std::collections::HashSet;

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use hearth_common::api_types::{
    ReportClosureFailureRequest, ReportClosureFailureResponse, UpsertUserConfigRequest, UserConfig,
    UserEnvBuildJob, UserEnvClosureResponse,
};
use serde::Deserialize;

use crate::AppState;
use crate::auth::{AdminIdentity, MachineIdentity};
use crate::error::AppError;
use crate::repo;

/// Validate that `extra_packages` in the overrides only contains allowed packages.
///
/// When `allowlist` is `None`, all packages are allowed (no restrictions).
/// Returns the list of disallowed package names on failure.
fn validate_extra_packages(
    overrides: &serde_json::Value,
    allowlist: &Option<HashSet<String>>,
) -> Result<(), Vec<String>> {
    let Some(allowlist) = allowlist else {
        return Ok(());
    };

    let Some(packages) = overrides.get("extra_packages").and_then(|v| v.as_array()) else {
        return Ok(());
    };

    let disallowed: Vec<String> = packages
        .iter()
        .filter_map(|v| v.as_str())
        .filter(|name| !allowlist.contains(*name))
        .map(String::from)
        .collect();

    if disallowed.is_empty() {
        Ok(())
    } else {
        Err(disallowed)
    }
}

/// GET /api/v1/users/{username}/config — get a user's per-user config (admin).
pub async fn get_config(
    _admin: AdminIdentity,
    State(state): State<AppState>,
    Path(username): Path<String>,
) -> Result<Json<UserConfig>, AppError> {
    let row = repo::get_user_config(&state.pool, &username).await?;
    match row {
        Some(r) => Ok(Json(r.into())),
        None => Err(AppError::NotFound(format!(
            "user config for {username} not found"
        ))),
    }
}

/// PUT /api/v1/users/{username}/config — upsert a user's config (admin).
/// Role templates are initial seeds; this sets the per-user source of truth.
pub async fn upsert_config(
    _admin: AdminIdentity,
    State(state): State<AppState>,
    Path(username): Path<String>,
    Json(req): Json<UpsertUserConfigRequest>,
) -> Result<(StatusCode, Json<UserConfig>), AppError> {
    let base_role = req.base_role.as_deref().unwrap_or("default");
    let overrides = req.overrides.as_ref().cloned().unwrap_or_default();

    if let Err(disallowed) = validate_extra_packages(&overrides, &state.package_allowlist) {
        return Err(AppError::BadRequest(format!(
            "packages not in allowlist: {}",
            disallowed.join(", ")
        )));
    }

    let row = repo::upsert_user_config(&state.pool, &username, base_role, &overrides).await?;
    Ok((StatusCode::OK, Json(row.into())))
}

/// POST /api/v1/users/{username}/config/build — force-trigger a rebuild (admin).
pub async fn trigger_build(
    _admin: AdminIdentity,
    State(state): State<AppState>,
    Path(username): Path<String>,
) -> Result<(StatusCode, Json<UserEnvBuildJob>), AppError> {
    let config = repo::get_user_config(&state.pool, &username).await?;
    let config = config
        .ok_or_else(|| AppError::NotFound(format!("user config for {username} not found")))?;
    let config_hash = config.config_hash.unwrap_or_default();
    let job = repo::enqueue_user_env_build(&state.pool, &username, &config_hash).await?;
    Ok((StatusCode::CREATED, Json(job.into())))
}

/// Query parameters for the env-closure endpoint.
#[derive(Debug, Deserialize)]
pub struct EnvClosureQuery {
    /// The agent-resolved role for this user. When provided and no `user_config`
    /// exists yet, the API auto-provisions one so the build pipeline can produce
    /// a per-user closure for subsequent logins.
    pub role: Option<String>,
}

/// GET /api/v1/users/{username}/env-closure — agent looks up pre-built closure.
/// Returns the user's latest closure (if built) and fallback role for template activation.
/// Auto-provisions a `user_config` on first contact so the build pipeline kicks in.
pub async fn get_env_closure(
    _machine: MachineIdentity,
    State(state): State<AppState>,
    Path(username): Path<String>,
    Query(query): Query<EnvClosureQuery>,
) -> Result<Json<UserEnvClosureResponse>, AppError> {
    let config = repo::get_user_config(&state.pool, &username).await?;
    match config {
        Some(row) => {
            let build_status: hearth_common::api_types::UserEnvBuildStatus =
                row.build_status.into();
            Ok(Json(UserEnvClosureResponse {
                closure: row.latest_closure,
                cache_url: state.cache_url.clone(),
                fallback_role: row.base_role,
                build_status: Some(build_status),
            }))
        }
        None => {
            // Auto-provision: create a user_config with the agent-resolved role
            // so the background build task enqueues a closure build.
            let base_role = query.role.as_deref().unwrap_or("default");
            let overrides = serde_json::Value::Object(Default::default());
            let _ = repo::upsert_user_config(&state.pool, &username, base_role, &overrides).await?;
            tracing::info!(%username, %base_role, "auto-provisioned user config on first login");
            Ok(Json(UserEnvClosureResponse {
                closure: None,
                cache_url: state.cache_url.clone(),
                fallback_role: base_role.to_string(),
                build_status: Some(hearth_common::api_types::UserEnvBuildStatus::Pending),
            }))
        }
    }
}

/// POST /api/v1/users/{username}/env-closure/report-failure — agent reports a broken closure.
///
/// If the reported closure matches the current `latest_closure`, invalidates it
/// and enqueues a rebuild. Prevents duplicate rebuilds if one is already in progress.
pub async fn report_closure_failure(
    _machine: MachineIdentity,
    State(state): State<AppState>,
    Path(username): Path<String>,
    Json(req): Json<ReportClosureFailureRequest>,
) -> Result<Json<ReportClosureFailureResponse>, AppError> {
    tracing::warn!(
        %username,
        closure = %req.closure,
        error = %req.error,
        "agent reported closure failure"
    );

    let rebuild_queued =
        repo::invalidate_user_closure(&state.pool, &username, &req.closure).await?;

    if rebuild_queued {
        tracing::info!(%username, "rebuild enqueued after closure failure report");
    }

    Ok(Json(ReportClosureFailureResponse { rebuild_queued }))
}
