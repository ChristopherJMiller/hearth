use axum::Json;
use axum::extract::{Path, State};
use hearth_common::api_types::{CreateMachineRequest, Machine, TargetState, UpdateMachineRequest};
use uuid::Uuid;

use crate::AppState;
use crate::auth::{AdminIdentity, MachineIdentity, UserIdentity};
use crate::error::AppError;
use crate::repo;

pub async fn list_machines(
    _user: UserIdentity,
    State(state): State<AppState>,
) -> Result<Json<Vec<Machine>>, AppError> {
    let rows = repo::list_machines(&state.pool).await?;
    let machines: Vec<Machine> = rows.into_iter().map(Into::into).collect();
    Ok(Json(machines))
}

pub async fn get_machine(
    _user: UserIdentity,
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
    _admin: AdminIdentity,
    State(state): State<AppState>,
    Json(req): Json<CreateMachineRequest>,
) -> Result<(axum::http::StatusCode, Json<Machine>), AppError> {
    let row = repo::create_machine(&state.pool, &req).await?;
    Ok((axum::http::StatusCode::CREATED, Json(row.into())))
}

pub async fn update_machine(
    _admin: AdminIdentity,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateMachineRequest>,
) -> Result<Json<Machine>, AppError> {
    // Track whether role or extra_config changed — these trigger a rebuild.
    let triggers_rebuild = req.role.is_some() || req.extra_config.is_some();

    let row = repo::update_machine(&state.pool, id, &req).await?;
    match row {
        Some(r) => {
            let machine: Machine = r.into();

            // Auto-queue a build job when role or extra_config changes.
            if triggers_rebuild {
                let flake_ref = std::env::var("HEARTH_FLAKE_REF").map_err(|_| {
                    AppError::Internal("HEARTH_FLAKE_REF not configured on server".into())
                })?;
                let machine_filter = serde_json::json!({
                    "machine_ids": [id.to_string()]
                });
                match repo::enqueue_build_job(
                    &state.pool,
                    &flake_ref,
                    Some(&machine_filter),
                    1,
                    1,
                    1.0,
                )
                .await
                {
                    Ok(job) => {
                        tracing::info!(
                            machine_id = %id,
                            build_job_id = %job.id,
                            "queued rebuild after machine config change"
                        );
                    }
                    Err(e) => {
                        tracing::warn!(
                            machine_id = %id,
                            error = %e,
                            "failed to queue rebuild job"
                        );
                    }
                }
            }

            Ok(Json(machine))
        }
        None => Err(AppError::NotFound(format!("machine {id} not found"))),
    }
}

pub async fn delete_machine(
    _admin: AdminIdentity,
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
    _machine: MachineIdentity,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<TargetState>, AppError> {
    let target = repo::get_target_state(&state.pool, id).await?;
    match target {
        Some(t) => Ok(Json(t)),
        None => Err(AppError::NotFound(format!("machine {id} not found"))),
    }
}
