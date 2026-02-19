use axum::Json;
use axum::extract::{Query, State};
use chrono::{DateTime, Utc};
use hearth_common::api_types::AuditEvent;
use serde::Deserialize;
use uuid::Uuid;

use crate::AppState;
use crate::error::AppError;
use crate::repo;

#[derive(Debug, Deserialize)]
pub struct AuditFilters {
    pub event_type: Option<String>,
    pub machine_id: Option<Uuid>,
    pub actor: Option<String>,
    pub since: Option<DateTime<Utc>>,
    pub limit: Option<i64>,
}

pub async fn list_audit_events(
    State(state): State<AppState>,
    Query(params): Query<AuditFilters>,
) -> Result<Json<Vec<AuditEvent>>, AppError> {
    let limit = params.limit.unwrap_or(100).clamp(1, 1000);

    let rows = repo::list_audit_events(
        &state.pool,
        params.event_type.as_deref(),
        params.machine_id,
        params.actor.as_deref(),
        params.since,
        limit,
    )
    .await?;

    let events: Vec<AuditEvent> = rows.into_iter().map(Into::into).collect();
    Ok(Json(events))
}
