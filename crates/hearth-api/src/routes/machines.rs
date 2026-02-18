use axum::Json;
use axum::extract::{Path, State};
use hearth_common::api_types::{CreateMachineRequest, Machine, TargetState, UpdateMachineRequest};
use uuid::Uuid;

use crate::AppState;
use crate::error::AppError;
use crate::repo;

pub async fn list_machines(State(state): State<AppState>) -> Result<Json<Vec<Machine>>, AppError> {
    let rows = repo::list_machines(&state.pool).await?;
    let machines: Vec<Machine> = rows.into_iter().map(Into::into).collect();
    Ok(Json(machines))
}

pub async fn get_machine(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Machine>, AppError> {
    let row = repo::get_machine(&state.pool, id).await?;
    match row {
        Some(r) => Ok(Json(r.into())),
        None => Err(AppError::NotFound(format!("machine {id} not found"))),
    }
}

pub async fn create_machine(
    State(state): State<AppState>,
    Json(req): Json<CreateMachineRequest>,
) -> Result<(axum::http::StatusCode, Json<Machine>), AppError> {
    let row = repo::create_machine(&state.pool, &req).await?;
    Ok((axum::http::StatusCode::CREATED, Json(row.into())))
}

pub async fn update_machine(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateMachineRequest>,
) -> Result<Json<Machine>, AppError> {
    let row = repo::update_machine(&state.pool, id, &req).await?;
    match row {
        Some(r) => Ok(Json(r.into())),
        None => Err(AppError::NotFound(format!("machine {id} not found"))),
    }
}

pub async fn delete_machine(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<axum::http::StatusCode, AppError> {
    let deleted = repo::delete_machine(&state.pool, id).await?;
    if deleted {
        Ok(axum::http::StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound(format!("machine {id} not found")))
    }
}

pub async fn get_target_state(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<TargetState>, AppError> {
    let target = repo::get_target_state(&state.pool, id).await?;
    match target {
        Some(t) => Ok(Json(t)),
        None => Err(AppError::NotFound(format!("machine {id} not found"))),
    }
}
