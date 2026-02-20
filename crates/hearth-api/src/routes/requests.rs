use axum::Json;
use axum::extract::{Path, Query, State};
use hearth_common::api_types::{InstallResultReport, ResolveRequestBody, SoftwareRequest};
use serde::Deserialize;
use uuid::Uuid;

use crate::AppState;
use crate::auth::{MachineIdentity, OperatorIdentity, UserIdentity};
use crate::db::SoftwareRequestStatusDb;
use crate::error::AppError;
use crate::repo;

#[derive(Debug, Deserialize)]
pub struct ListRequestsParams {
    pub status: Option<String>,
}

fn parse_status(s: &str) -> Result<SoftwareRequestStatusDb, AppError> {
    match s {
        "pending" => Ok(SoftwareRequestStatusDb::Pending),
        "approved" => Ok(SoftwareRequestStatusDb::Approved),
        "denied" => Ok(SoftwareRequestStatusDb::Denied),
        "installing" => Ok(SoftwareRequestStatusDb::Installing),
        "installed" => Ok(SoftwareRequestStatusDb::Installed),
        "failed" => Ok(SoftwareRequestStatusDb::Failed),
        other => Err(AppError::BadRequest(format!("invalid status: {other}"))),
    }
}

pub async fn list_requests(
    _user: UserIdentity,
    State(state): State<AppState>,
    Query(params): Query<ListRequestsParams>,
) -> Result<Json<Vec<SoftwareRequest>>, AppError> {
    let status_filter = params.status.as_deref().map(parse_status).transpose()?;

    let rows = repo::list_requests(&state.pool, status_filter).await?;
    let requests: Vec<SoftwareRequest> = rows.into_iter().map(Into::into).collect();
    Ok(Json(requests))
}

pub async fn approve_request(
    _op: OperatorIdentity,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<ResolveRequestBody>,
) -> Result<Json<SoftwareRequest>, AppError> {
    let row = repo::approve_request(&state.pool, id, &body.admin).await?;
    match row {
        Some(r) => Ok(Json(r.into())),
        None => Err(AppError::NotFound(format!(
            "request {id} not found or not pending"
        ))),
    }
}

pub async fn deny_request(
    _op: OperatorIdentity,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<ResolveRequestBody>,
) -> Result<Json<SoftwareRequest>, AppError> {
    let row = repo::deny_request(&state.pool, id, &body.admin).await?;
    match row {
        Some(r) => Ok(Json(r.into())),
        None => Err(AppError::NotFound(format!(
            "request {id} not found or not pending"
        ))),
    }
}

pub async fn claim_install(
    _machine: MachineIdentity,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<SoftwareRequest>, AppError> {
    let row = repo::claim_install(&state.pool, id).await?;
    match row {
        Some(r) => Ok(Json(r.into())),
        None => Err(AppError::NotFound(format!(
            "request {id} not found or not approved"
        ))),
    }
}

pub async fn report_install_result(
    _machine: MachineIdentity,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<InstallResultReport>,
) -> Result<Json<SoftwareRequest>, AppError> {
    let row = repo::report_install_result(&state.pool, id, body.success).await?;
    match row {
        Some(r) => Ok(Json(r.into())),
        None => Err(AppError::NotFound(format!(
            "request {id} not found or not in installing state"
        ))),
    }
}
