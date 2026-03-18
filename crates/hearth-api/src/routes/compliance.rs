//! Compliance endpoints: drift detection, policy CRUD, deployment results, SBOMs.

use axum::Json;
use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use hearth_common::api_types::{
    self, CreateCompliancePolicyRequest, DriftStatus, UpdateCompliancePolicyRequest,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::AppState;
use crate::auth::{AdminIdentity, UserIdentity};
use crate::error::AppError;
use crate::repo;

const VALID_SEVERITIES: &[&str] = &["low", "medium", "high", "critical"];

fn validate_severity(severity: &str) -> Result<(), AppError> {
    if !VALID_SEVERITIES.contains(&severity) {
        return Err(AppError::BadRequest(format!(
            "invalid severity '{severity}', must be one of: {}",
            VALID_SEVERITIES.join(", ")
        )));
    }
    Ok(())
}

// --- Drift detection ---

#[derive(Debug, Deserialize)]
pub struct DriftParams {
    #[serde(default)]
    pub status: Option<String>,
}

/// GET /api/v1/compliance/drift
pub async fn list_drift(
    UserIdentity(_claims): UserIdentity,
    State(state): State<AppState>,
    Query(params): Query<DriftParams>,
) -> Result<Json<Vec<api_types::DriftedMachine>>, AppError> {
    let rows = repo::list_machines_by_drift_status(&state.pool, params.status.as_deref()).await?;

    let machines: Vec<api_types::DriftedMachine> = rows
        .into_iter()
        .map(|row| {
            let drift_status = match (&row.target_closure, &row.current_closure) {
                (None, _) => DriftStatus::NoTarget,
                (Some(target), Some(current)) if current == target => DriftStatus::Compliant,
                _ => DriftStatus::Drifted,
            };
            api_types::DriftedMachine {
                id: row.id,
                hostname: row.hostname,
                current_closure: row.current_closure,
                target_closure: row.target_closure,
                last_heartbeat: row.last_heartbeat,
                role: row.role,
                tags: row.tags,
                drift_status,
            }
        })
        .collect();

    Ok(Json(machines))
}

// --- Compliance policy CRUD ---

/// GET /api/v1/compliance/policies
pub async fn list_policies(
    UserIdentity(_claims): UserIdentity,
    State(state): State<AppState>,
) -> Result<Json<Vec<api_types::CompliancePolicy>>, AppError> {
    let rows = repo::list_compliance_policies(&state.pool).await?;
    Ok(Json(rows.into_iter().map(Into::into).collect()))
}

/// POST /api/v1/compliance/policies
pub async fn create_policy(
    AdminIdentity(_claims): AdminIdentity,
    State(state): State<AppState>,
    Json(req): Json<CreateCompliancePolicyRequest>,
) -> Result<Json<api_types::CompliancePolicy>, AppError> {
    validate_severity(&req.severity)?;
    let row = repo::create_compliance_policy(&state.pool, &req).await?;
    Ok(Json(row.into()))
}

/// GET /api/v1/compliance/policies/:id
pub async fn get_policy(
    UserIdentity(_claims): UserIdentity,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<api_types::CompliancePolicy>, AppError> {
    let row = repo::get_compliance_policy(&state.pool, id)
        .await?
        .ok_or_else(|| AppError::NotFound("compliance policy not found".into()))?;
    Ok(Json(row.into()))
}

/// PUT /api/v1/compliance/policies/:id
pub async fn update_policy(
    AdminIdentity(_claims): AdminIdentity,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateCompliancePolicyRequest>,
) -> Result<Json<api_types::CompliancePolicy>, AppError> {
    if let Some(ref sev) = req.severity {
        validate_severity(sev)?;
    }
    let row = repo::update_compliance_policy(&state.pool, id, &req)
        .await?
        .ok_or_else(|| AppError::NotFound("compliance policy not found".into()))?;
    Ok(Json(row.into()))
}

/// DELETE /api/v1/compliance/policies/:id
pub async fn delete_policy(
    AdminIdentity(_claims): AdminIdentity,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    let deleted = repo::delete_compliance_policy(&state.pool, id).await?;
    if !deleted {
        return Err(AppError::NotFound("compliance policy not found".into()));
    }
    Ok(Json(serde_json::json!({ "deleted": true })))
}

// --- Deployment policy results ---

/// GET /api/v1/compliance/deployments/:id/results
pub async fn deployment_results(
    UserIdentity(_claims): UserIdentity,
    State(state): State<AppState>,
    Path(deployment_id): Path<Uuid>,
) -> Result<Json<Vec<api_types::PolicyResult>>, AppError> {
    let rows = repo::get_deployment_policy_results(&state.pool, deployment_id).await?;
    Ok(Json(rows.into_iter().map(Into::into).collect()))
}

/// GET /api/v1/compliance/deployments/:id/summary
pub async fn deployment_summary(
    UserIdentity(_claims): UserIdentity,
    State(state): State<AppState>,
    Path(deployment_id): Path<Uuid>,
) -> Result<Json<api_types::DeploymentComplianceSummary>, AppError> {
    let summary = repo::get_deployment_compliance_summary(&state.pool, deployment_id).await?;
    Ok(Json(summary))
}

// --- SBOM endpoints ---

/// Serve an SBOM file from disk given a relative path.
async fn serve_sbom_file(sbom_path: &str) -> Result<Response, AppError> {
    let sbom_dir = crate::build::sbom::sbom_base_dir();
    let path = std::path::Path::new(&sbom_dir).join(sbom_path);

    let content = tokio::fs::read(&path).await.map_err(|e| {
        tracing::error!(path = %path.display(), error = %e, "failed to read SBOM file");
        AppError::NotFound("SBOM file not found on disk".into())
    })?;

    Ok((
        [(header::CONTENT_TYPE, "application/json")],
        Body::from(content),
    )
        .into_response())
}

/// GET /api/v1/compliance/sboms/:deployment_id
pub async fn list_sboms(
    UserIdentity(_claims): UserIdentity,
    State(state): State<AppState>,
    Path(deployment_id): Path<Uuid>,
) -> Result<Json<Vec<api_types::DeploymentSbom>>, AppError> {
    let rows = repo::list_deployment_sboms(&state.pool, deployment_id).await?;
    Ok(Json(rows.into_iter().map(Into::into).collect()))
}

/// GET /api/v1/compliance/sboms/:deployment_id/:machine_id
pub async fn download_sbom(
    UserIdentity(_claims): UserIdentity,
    State(state): State<AppState>,
    Path((deployment_id, machine_id)): Path<(Uuid, Uuid)>,
) -> Result<Response, AppError> {
    let row = repo::get_deployment_sbom(&state.pool, deployment_id, machine_id)
        .await?
        .ok_or_else(|| AppError::NotFound("SBOM not found".into()))?;
    serve_sbom_file(&row.sbom_path).await
}

/// GET /api/v1/compliance/machines/:machine_id/sbom
pub async fn machine_current_sbom(
    UserIdentity(_claims): UserIdentity,
    State(state): State<AppState>,
    Path(machine_id): Path<Uuid>,
) -> Result<Response, AppError> {
    let row = repo::get_machine_current_sbom(&state.pool, machine_id)
        .await?
        .ok_or_else(|| AppError::NotFound("no SBOM available for this machine".into()))?;
    serve_sbom_file(&row.sbom_path).await
}
