use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use hearth_common::api_types::{
    UpsertUserConfigRequest, UserConfig, UserEnvBuildJob, UserEnvClosureResponse,
};

use crate::AppState;
use crate::auth::{AdminIdentity, MachineIdentity};
use crate::error::AppError;
use crate::repo;

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
    let config = config.ok_or_else(|| {
        AppError::NotFound(format!("user config for {username} not found"))
    })?;
    let config_hash = config.config_hash.unwrap_or_default();
    let job = repo::enqueue_user_env_build(&state.pool, &username, &config_hash).await?;
    Ok((StatusCode::CREATED, Json(job.into())))
}

/// GET /api/v1/users/{username}/env-closure — agent looks up pre-built closure.
/// Returns the user's latest closure (if built) and fallback role for template activation.
pub async fn get_env_closure(
    _machine: MachineIdentity,
    State(state): State<AppState>,
    Path(username): Path<String>,
) -> Result<Json<UserEnvClosureResponse>, AppError> {
    let config = repo::get_user_config(&state.pool, &username).await?;
    match config {
        Some(row) => Ok(Json(UserEnvClosureResponse {
            closure: row.latest_closure,
            cache_url: state.cache_url.clone(),
            fallback_role: row.base_role,
        })),
        None => Ok(Json(UserEnvClosureResponse {
            closure: None,
            cache_url: state.cache_url.clone(),
            fallback_role: "default".to_string(),
        })),
    }
}
