//! Reporting endpoints: compliance posture, deployment timeline, enrollment timeline.

use axum::Json;
use axum::extract::{Query, State};
use serde::{Deserialize, Serialize};

use crate::AppState;
use crate::auth::UserIdentity;
use crate::error::AppError;
use crate::repo;

// --- Compliance report ---

#[derive(Debug, Serialize)]
pub struct ComplianceReport {
    pub total: i64,
    pub compliant: i64,
    pub drifted: i64,
    pub no_target: i64,
}

/// GET /api/v1/reports/compliance
pub async fn compliance_report(
    UserIdentity(_claims): UserIdentity,
    State(state): State<AppState>,
) -> Result<Json<ComplianceReport>, AppError> {
    let report = repo::get_compliance_report(&state.pool).await?;
    Ok(Json(report))
}

// --- Deployment timeline ---

#[derive(Debug, Deserialize)]
pub struct TimelineParams {
    #[serde(default = "default_days")]
    pub days: i64,
}

fn default_days() -> i64 {
    30
}

#[derive(Debug, Serialize)]
pub struct DeploymentTimelineEntry {
    pub date: String,
    pub completed: i64,
    pub failed: i64,
    pub rolled_back: i64,
}

/// GET /api/v1/reports/deployments
pub async fn deployment_timeline(
    UserIdentity(_claims): UserIdentity,
    State(state): State<AppState>,
    Query(params): Query<TimelineParams>,
) -> Result<Json<Vec<DeploymentTimelineEntry>>, AppError> {
    let entries = repo::get_deployment_timeline(&state.pool, params.days).await?;
    Ok(Json(entries))
}

// --- Enrollment timeline ---

#[derive(Debug, Serialize)]
pub struct EnrollmentTimelineEntry {
    pub date: String,
    pub enrolled: i64,
    pub pending: i64,
}

/// GET /api/v1/reports/enrollments
pub async fn enrollment_timeline(
    UserIdentity(_claims): UserIdentity,
    State(state): State<AppState>,
    Query(params): Query<TimelineParams>,
) -> Result<Json<Vec<EnrollmentTimelineEntry>>, AppError> {
    let entries = repo::get_enrollment_timeline(&state.pool, params.days).await?;
    Ok(Json(entries))
}
