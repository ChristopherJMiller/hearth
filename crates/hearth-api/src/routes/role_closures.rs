use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use hearth_common::api_types::{RoleClosure, UpsertRoleClosureRequest};

use crate::AppState;
use crate::auth::{AdminIdentity, UserIdentity};
use crate::error::AppError;
use crate::repo;

pub async fn list(
    _user: UserIdentity,
    State(state): State<AppState>,
) -> Result<Json<Vec<RoleClosure>>, AppError> {
    let rows = repo::list_role_closures(&state.pool).await?;
    Ok(Json(rows.into_iter().map(Into::into).collect()))
}

pub async fn get(
    _user: UserIdentity,
    State(state): State<AppState>,
    Path(role): Path<String>,
) -> Result<Json<RoleClosure>, AppError> {
    let row = repo::get_role_closure(&state.pool, &role).await?;
    match row {
        Some(r) => Ok(Json(r.into())),
        None => Err(AppError::NotFound(format!(
            "no closure registered for role '{role}'"
        ))),
    }
}

pub async fn upsert(
    _admin: AdminIdentity,
    State(state): State<AppState>,
    Json(req): Json<UpsertRoleClosureRequest>,
) -> Result<(StatusCode, Json<RoleClosure>), AppError> {
    let row = repo::upsert_role_closure(&state.pool, &req.role, &req.closure).await?;
    Ok((StatusCode::OK, Json(row.into())))
}

pub async fn delete(
    _admin: AdminIdentity,
    State(state): State<AppState>,
    Path(role): Path<String>,
) -> Result<StatusCode, AppError> {
    let deleted = repo::delete_role_closure(&state.pool, &role).await?;
    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound(format!(
            "no closure registered for role '{role}'"
        )))
    }
}
