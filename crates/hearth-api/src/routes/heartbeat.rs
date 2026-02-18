use axum::Json;
use axum::extract::State;
use hearth_common::api_types::{HeartbeatRequest, HeartbeatResponse};

use crate::AppState;
use crate::error::AppError;
use crate::repo;

pub async fn record_heartbeat(
    State(state): State<AppState>,
    Json(req): Json<HeartbeatRequest>,
) -> Result<Json<HeartbeatResponse>, AppError> {
    let response = repo::record_heartbeat(&state.pool, &req).await?;
    match response {
        Some(resp) => Ok(Json(resp)),
        None => Err(AppError::NotFound(format!(
            "machine {} not found",
            req.machine_id
        ))),
    }
}
