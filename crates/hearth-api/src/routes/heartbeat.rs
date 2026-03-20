use std::time::Duration;

use axum::Json;
use axum::extract::State;
use hearth_common::api_types::{HeartbeatRequest, HeartbeatResponse};
use tracing::{debug, warn};

use crate::AppState;
use crate::auth::MachineIdentity;
use crate::cache_token;
use crate::error::AppError;
use crate::repo;

pub async fn record_heartbeat(
    _machine: MachineIdentity,
    State(state): State<AppState>,
    Json(req): Json<HeartbeatRequest>,
) -> Result<Json<HeartbeatResponse>, AppError> {
    metrics::counter!("hearth_heartbeats_total").increment(1);
    let response = repo::record_heartbeat(&state.pool, &req).await?;
    match response {
        Some(mut resp) => {
            // Mint a fresh cache pull token for this agent (4h validity).
            let subject = format!("agent-{}", req.machine_id);
            match cache_token::mint_pull_token(&subject, Duration::from_secs(4 * 3600)) {
                Ok(Some(creds)) => {
                    debug!(machine_id = %req.machine_id, "minted cache pull token for {subject}");
                    resp.cache_url = Some(creds.cache_url);
                    resp.cache_token = Some(creds.cache_token);
                }
                Ok(None) => {
                    // Secret not configured — no cache auth available.
                }
                Err(e) => {
                    warn!(error = %e, "failed to mint cache token for {subject}");
                }
            }

            // Include the service directory so the agent can populate desktop bookmarks.
            resp.services = state.services.clone();

            Ok(Json(resp))
        }
        None => Err(AppError::NotFound(format!(
            "machine {} not found",
            req.machine_id
        ))),
    }
}
