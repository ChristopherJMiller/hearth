//! Remote action endpoints: create, list, and report results for machine actions.

use axum::Json;
use axum::extract::{Path, State};
use uuid::Uuid;

use crate::AppState;
use crate::auth::{AdminIdentity, MachineIdentity, UserIdentity};
use crate::error::AppError;
use crate::repo;
use hearth_common::api_types::{ActionResultReport, CreateActionRequest, PendingAction};

/// POST /api/v1/machines/{id}/actions — create a remote action (admin only)
pub async fn create_action(
    AdminIdentity(claims): AdminIdentity,
    State(state): State<AppState>,
    Path(machine_id): Path<Uuid>,
    Json(req): Json<CreateActionRequest>,
) -> Result<Json<PendingAction>, AppError> {
    let actor = claims.preferred_username.as_deref().unwrap_or(&claims.sub);

    let row = repo::create_action(
        &state.pool,
        machine_id,
        req.action_type,
        &req.payload,
        actor,
    )
    .await?;

    // Record audit event
    let _ = repo::create_audit_event(
        &state.pool,
        "remote_action",
        Some(actor),
        Some(machine_id),
        &serde_json::json!({
            "action_type": req.action_type,
            "action_id": row.id,
        }),
    )
    .await;

    Ok(Json(row.into()))
}

/// GET /api/v1/machines/{id}/actions — list actions for a machine
pub async fn list_actions(
    UserIdentity(_claims): UserIdentity,
    State(state): State<AppState>,
    Path(machine_id): Path<Uuid>,
) -> Result<Json<Vec<PendingAction>>, AppError> {
    let rows = repo::list_machine_actions(&state.pool, machine_id).await?;
    Ok(Json(rows.into_iter().map(Into::into).collect()))
}

/// POST /api/v1/actions/{id}/result — agent reports action result
pub async fn report_action_result(
    MachineIdentity(_machine_id): MachineIdentity,
    State(state): State<AppState>,
    Path(action_id): Path<Uuid>,
    Json(report): Json<ActionResultReport>,
) -> Result<Json<PendingAction>, AppError> {
    let row = repo::complete_action(
        &state.pool,
        action_id,
        report.success,
        report.result.as_ref(),
    )
    .await?
    .ok_or_else(|| AppError::NotFound("action not found".into()))?;

    Ok(Json(row.into()))
}
