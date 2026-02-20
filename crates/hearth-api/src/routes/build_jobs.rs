use axum::Json;
use axum::extract::{Path, Query, State};
use hearth_common::api_types::BuildJob;
use serde::Deserialize;
use uuid::Uuid;

use crate::AppState;
use crate::auth::UserIdentity;
use crate::db::BuildJobStatusDb;
use crate::error::AppError;
use crate::repo;

#[derive(Debug, Deserialize)]
pub struct BuildJobFilters {
    pub status: Option<String>,
}

fn parse_build_job_status(s: &str) -> Result<BuildJobStatusDb, AppError> {
    match s {
        "pending" => Ok(BuildJobStatusDb::Pending),
        "claimed" => Ok(BuildJobStatusDb::Claimed),
        "evaluating" => Ok(BuildJobStatusDb::Evaluating),
        "building" => Ok(BuildJobStatusDb::Building),
        "pushing" => Ok(BuildJobStatusDb::Pushing),
        "deploying" => Ok(BuildJobStatusDb::Deploying),
        "completed" => Ok(BuildJobStatusDb::Completed),
        "failed" => Ok(BuildJobStatusDb::Failed),
        other => Err(AppError::BadRequest(format!(
            "invalid build job status: {other}"
        ))),
    }
}

pub async fn list_build_jobs(
    _user: UserIdentity,
    State(state): State<AppState>,
    Query(params): Query<BuildJobFilters>,
) -> Result<Json<Vec<BuildJob>>, AppError> {
    let status_filter = params
        .status
        .as_deref()
        .map(parse_build_job_status)
        .transpose()?;

    let rows = repo::list_build_jobs(&state.pool, status_filter).await?;
    let jobs: Vec<BuildJob> = rows.into_iter().map(Into::into).collect();
    Ok(Json(jobs))
}

pub async fn get_build_job(
    _user: UserIdentity,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<BuildJob>, AppError> {
    let row = repo::get_build_job(&state.pool, id).await?;
    match row {
        Some(r) => Ok(Json(r.into())),
        None => Err(AppError::NotFound(format!("build job {id} not found"))),
    }
}
