use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use hearth_common::api_types::{UpsertUserEnvRequest, UserEnvironment};
use uuid::Uuid;

use crate::AppState;
use crate::auth::{MachineIdentity, UserIdentity};
use crate::db::UserEnvStatusDb;
use crate::error::AppError;
use crate::repo;

pub async fn list_environments(
    _user: UserIdentity,
    State(state): State<AppState>,
    Path(machine_id): Path<Uuid>,
) -> Result<Json<Vec<UserEnvironment>>, AppError> {
    let rows = repo::list_user_envs(&state.pool, machine_id).await?;
    let envs: Vec<UserEnvironment> = rows.into_iter().map(Into::into).collect();
    Ok(Json(envs))
}

pub async fn get_environment(
    _user: UserIdentity,
    State(state): State<AppState>,
    Path((machine_id, username)): Path<(Uuid, String)>,
) -> Result<Json<UserEnvironment>, AppError> {
    let row = repo::get_user_env(&state.pool, machine_id, &username).await?;
    match row {
        Some(r) => Ok(Json(r.into())),
        None => Err(AppError::NotFound(format!(
            "environment for user {username} on machine {machine_id} not found"
        ))),
    }
}

pub async fn upsert_environment(
    _machine: MachineIdentity,
    State(state): State<AppState>,
    Path((machine_id, username)): Path<(Uuid, String)>,
    Json(req): Json<UpsertUserEnvRequest>,
) -> Result<(StatusCode, Json<UserEnvironment>), AppError> {
    let status = req.status.map(UserEnvStatusDb::from);
    let row = repo::upsert_user_env(&state.pool, machine_id, &username, &req.role, status).await?;
    Ok((StatusCode::OK, Json(row.into())))
}

pub async fn record_login(
    _machine: MachineIdentity,
    State(state): State<AppState>,
    Path((machine_id, username)): Path<(Uuid, String)>,
) -> Result<Json<UserEnvironment>, AppError> {
    let row = repo::record_user_login(&state.pool, machine_id, &username).await?;
    match row {
        Some(r) => Ok(Json(r.into())),
        None => Err(AppError::NotFound(format!(
            "environment for user {username} on machine {machine_id} not found"
        ))),
    }
}
