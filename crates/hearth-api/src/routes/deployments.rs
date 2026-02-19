use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use hearth_common::api_types::{
    CreateDeploymentRequest, Deployment, DeploymentMachineStatus, TriggerBuildRequest,
    UpdateDeploymentStatusRequest, UpdateMachineUpdateStatusRequest,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::AppState;
use crate::db::{DeploymentStatusDb, MachineUpdateStatusDb};
use crate::error::AppError;
use crate::repo;

#[derive(Debug, Deserialize)]
pub struct DeploymentFilters {
    pub status: Option<String>,
}

fn parse_deployment_status(s: &str) -> Result<DeploymentStatusDb, AppError> {
    match s {
        "pending" => Ok(DeploymentStatusDb::Pending),
        "canary" => Ok(DeploymentStatusDb::Canary),
        "rolling" => Ok(DeploymentStatusDb::Rolling),
        "completed" => Ok(DeploymentStatusDb::Completed),
        "failed" => Ok(DeploymentStatusDb::Failed),
        "rolled_back" => Ok(DeploymentStatusDb::RolledBack),
        other => Err(AppError::BadRequest(format!(
            "invalid deployment status: {other}"
        ))),
    }
}

pub async fn create_deployment(
    State(state): State<AppState>,
    Json(req): Json<CreateDeploymentRequest>,
) -> Result<(StatusCode, Json<Deployment>), AppError> {
    let row = repo::create_deployment(&state.pool, &req).await?;
    Ok((StatusCode::CREATED, Json(row.into())))
}

pub async fn list_deployments(
    State(state): State<AppState>,
    Query(params): Query<DeploymentFilters>,
) -> Result<Json<Vec<Deployment>>, AppError> {
    let status_filter = params
        .status
        .as_deref()
        .map(parse_deployment_status)
        .transpose()?;

    let rows = repo::list_deployments(&state.pool, status_filter).await?;
    let deployments: Vec<Deployment> = rows.into_iter().map(Into::into).collect();
    Ok(Json(deployments))
}

pub async fn get_deployment(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Deployment>, AppError> {
    let row = repo::get_deployment(&state.pool, id).await?;
    match row {
        Some(r) => Ok(Json(r.into())),
        None => Err(AppError::NotFound(format!("deployment {id} not found"))),
    }
}

pub async fn update_deployment_status(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateDeploymentStatusRequest>,
) -> Result<Json<Deployment>, AppError> {
    let status_db: DeploymentStatusDb = req.status.into();

    // Fetch current deployment to validate FSM transition
    let existing = repo::get_deployment(&state.pool, id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("deployment {id} not found")))?;

    if !crate::deployment_fsm::is_valid_transition(existing.status, status_db) {
        return Err(AppError::BadRequest(format!(
            "invalid status transition from {:?} to {:?}",
            existing.status, status_db
        )));
    }

    let row = repo::update_deployment_status(&state.pool, id, status_db).await?;
    match row {
        Some(r) => Ok(Json(r.into())),
        None => Err(AppError::NotFound(format!("deployment {id} not found"))),
    }
}

pub async fn rollback_deployment(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Deployment>, AppError> {
    // Verify the deployment exists and the FSM allows a rollback transition
    let existing = repo::get_deployment(&state.pool, id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("deployment {id} not found")))?;

    if !crate::deployment_fsm::is_valid_transition(existing.status, DeploymentStatusDb::RolledBack)
    {
        return Err(AppError::BadRequest(format!(
            "deployment {id} cannot be rolled back from {:?} status",
            existing.status
        )));
    }

    let row = repo::rollback_deployment(&state.pool, id, "manual rollback requested").await?;
    match row {
        Some(r) => Ok(Json(r.into())),
        None => Err(AppError::NotFound(format!("deployment {id} not found"))),
    }
}

pub async fn list_deployment_machines(
    State(state): State<AppState>,
    Path(deployment_id): Path<Uuid>,
) -> Result<Json<Vec<DeploymentMachineStatus>>, AppError> {
    // Verify deployment exists
    let existing = repo::get_deployment(&state.pool, deployment_id).await?;
    if existing.is_none() {
        return Err(AppError::NotFound(format!(
            "deployment {deployment_id} not found"
        )));
    }

    let rows = repo::get_deployment_machines(&state.pool, deployment_id).await?;
    let machines: Vec<DeploymentMachineStatus> = rows.into_iter().map(Into::into).collect();
    Ok(Json(machines))
}

pub async fn update_machine_status(
    State(state): State<AppState>,
    Path((deployment_id, machine_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<UpdateMachineUpdateStatusRequest>,
) -> Result<Json<DeploymentMachineStatus>, AppError> {
    let status_db: MachineUpdateStatusDb = req.status.into();
    let row = repo::upsert_deployment_machine(
        &state.pool,
        deployment_id,
        machine_id,
        status_db,
        req.error_message.as_deref(),
    )
    .await?;

    // If the machine completed or failed, update the deployment counters
    match status_db {
        MachineUpdateStatusDb::Completed => {
            repo::increment_deployment_counter(&state.pool, deployment_id, true).await?;
        }
        MachineUpdateStatusDb::Failed => {
            repo::increment_deployment_counter(&state.pool, deployment_id, false).await?;
        }
        _ => {}
    }

    Ok(Json(row.into()))
}

/// Trigger a build pipeline as a background task.
///
/// POST /api/v1/deployments/build
///
/// This evaluates the flake, builds derivations, pushes to the cache,
/// and creates a deployment. Runs as a background tokio task.
pub async fn trigger_build(
    State(state): State<AppState>,
    Json(req): Json<TriggerBuildRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), AppError> {
    let pool = state.pool.clone();
    let flake_ref = req.flake_ref.clone();
    let target_filter = req.target_filter.clone();
    let canary_size = req.canary_size.unwrap_or(1);
    let batch_size = req.batch_size.unwrap_or(5);
    let failure_threshold = req.failure_threshold.unwrap_or(0.1);

    // Spawn the build pipeline as a background task.
    tokio::spawn(async move {
        match crate::build::orchestrator::run_build_pipeline(
            &pool,
            &flake_ref,
            target_filter.as_ref(),
            canary_size,
            batch_size,
            failure_threshold,
        )
        .await
        {
            Ok(result) => {
                tracing::info!(
                    deployment_id = %result.deployment_id,
                    machines = result.total_machines,
                    built = result.closures_built,
                    pushed = result.closures_pushed,
                    "build pipeline completed"
                );
            }
            Err(e) => {
                tracing::error!(error = %e, "build pipeline failed");
            }
        }
    });

    Ok((
        StatusCode::ACCEPTED,
        Json(serde_json::json!({
            "message": "build pipeline started",
            "flake_ref": req.flake_ref,
        })),
    ))
}
