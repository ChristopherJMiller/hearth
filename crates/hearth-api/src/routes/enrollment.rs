use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use hearth_common::api_types::{
    ApproveEnrollmentRequest, EnrollmentRequest, EnrollmentResponse, Machine,
};
use uuid::Uuid;

use crate::AppState;
use crate::error::AppError;
use crate::repo;

pub async fn enroll(
    State(state): State<AppState>,
    Json(req): Json<EnrollmentRequest>,
) -> Result<(StatusCode, Json<EnrollmentResponse>), AppError> {
    let row = repo::enroll_machine(
        &state.pool,
        &req.hostname,
        req.hardware_fingerprint.as_deref(),
    )
    .await?;

    let machine: Machine = row.into();
    let resp = EnrollmentResponse {
        machine_id: machine.id,
        status: machine.enrollment_status,
        message: "enrollment request submitted, awaiting approval".into(),
    };

    Ok((StatusCode::CREATED, Json(resp)))
}

pub async fn approve(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<ApproveEnrollmentRequest>,
) -> Result<Json<Machine>, AppError> {
    let row =
        repo::approve_enrollment(&state.pool, id, &req.role, req.target_closure.as_deref()).await?;
    match row {
        Some(r) => Ok(Json(r.into())),
        None => Err(AppError::NotFound(format!(
            "machine {id} not found or not in pending status"
        ))),
    }
}

pub async fn enrollment_status(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Machine>, AppError> {
    let row = repo::get_machine(&state.pool, id).await?;
    match row {
        Some(r) => Ok(Json(r.into())),
        None => Err(AppError::NotFound(format!("machine {id} not found"))),
    }
}
