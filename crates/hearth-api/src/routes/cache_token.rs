use axum::Json;
use axum::extract::State;
use std::time::Duration;

use crate::AppState;
use crate::auth::OptionalIdentity;
use crate::error::AppError;
use hearth_common::api_types::{AuthIdentity, CacheTokenResponse};

const TOKEN_VALIDITY_SECS: u64 = 4 * 3600; // 4 hours

/// `POST /api/v1/cache-token`
///
/// Returns a short-lived, pull-only Attic JWT for authenticated callers.
/// Accepts either a machine token (agent) or an OIDC user token.
pub async fn get_cache_token(
    identity: OptionalIdentity,
    State(_state): State<AppState>,
) -> Result<Json<CacheTokenResponse>, AppError> {
    let subject = match &identity.0 {
        Some(AuthIdentity::Machine { machine_id }) => format!("agent-{machine_id}"),
        Some(AuthIdentity::User(claims)) => format!("user-{}", claims.username()),
        None => return Err(AppError::Unauthorized("authentication required".into())),
    };

    match crate::cache_token::mint_pull_token(&subject, Duration::from_secs(TOKEN_VALIDITY_SECS)) {
        Ok(Some(creds)) => Ok(Json(CacheTokenResponse {
            cache_url: creds.cache_url,
            cache_token: creds.cache_token,
            expires_in: TOKEN_VALIDITY_SECS,
        })),
        Ok(None) => Err(AppError::Internal(
            "binary cache not configured (HEARTH_ATTIC_TOKEN_SECRET not set)".into(),
        )),
        Err(e) => Err(AppError::Internal(format!("failed to mint cache token: {e}"))),
    }
}
