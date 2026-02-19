use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use hearth_common::api_types::{
    ApproveEnrollmentRequest, EnrollmentRequest, EnrollmentResponse, Machine,
};
use std::time::Duration;
use tracing::info;
use uuid::Uuid;

use crate::AppState;
use crate::cache_token;
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
    // If no explicit closure was provided, look up the role's pre-built closure.
    let target_closure = match req.target_closure {
        Some(ref c) => Some(c.as_str().to_owned()),
        None => repo::get_role_closure(&state.pool, &req.role)
            .await?
            .map(|rc| rc.closure),
    };

    // Mint a short-lived pull-only cache token for this device.
    let extra_config = match cache_token::mint_pull_token(
        &format!("enrollment-{id}"),
        Duration::from_secs(4 * 3600),
    ) {
        Ok(Some(creds)) => {
            info!(machine_id = %id, "minted cache pull token for enrollment");
            Some(serde_json::json!({
                "cache_url": creds.cache_url,
                "cache_token": creds.cache_token,
            }))
        }
        Ok(None) => None,
        Err(e) => {
            tracing::warn!(error = %e, "failed to mint cache token, proceeding without");
            None
        }
    };

    let row = repo::approve_enrollment(
        &state.pool,
        id,
        &req.role,
        target_closure.as_deref(),
        extra_config.as_ref(),
    )
    .await?;
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
