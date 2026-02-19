use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use hearth_common::api_types::{
    ApproveEnrollmentRequest, AuthIdentity, EnrollmentRequest, EnrollmentResponse, Machine,
};
use std::time::Duration;
use tracing::info;
use uuid::Uuid;

use crate::AppState;
use crate::auth::OptionalIdentity;
use crate::cache_token;
use crate::error::AppError;
use crate::repo;

pub async fn enroll(
    State(state): State<AppState>,
    identity: OptionalIdentity,
    Json(req): Json<EnrollmentRequest>,
) -> Result<(StatusCode, Json<EnrollmentResponse>), AppError> {
    // Extract the enrolling user's username from their auth token (if present).
    let enrolled_by = match &identity.0 {
        Some(AuthIdentity::User(claims)) => {
            let username = claims.preferred_username.as_deref().unwrap_or(&claims.sub);
            // Upsert the user record so we track known identities.
            let _ = repo::upsert_user(
                &state.pool,
                username,
                None,
                claims.email.as_deref(),
                Some(&claims.sub),
                &claims.groups,
            )
            .await;
            Some(username.to_string())
        }
        _ => None,
    };

    let row = repo::enroll_machine(
        &state.pool,
        &req.hostname,
        req.hardware_fingerprint.as_deref(),
        enrolled_by.as_deref(),
    )
    .await?;

    let machine: Machine = row.into();
    info!(
        machine_id = %machine.id,
        enrolled_by = ?enrolled_by,
        "enrollment request submitted"
    );

    let resp = EnrollmentResponse {
        machine_id: machine.id,
        status: machine.enrollment_status,
        message: "enrollment request submitted, awaiting approval".into(),
        enrolled_by,
        machine_token: None,
    };

    Ok((StatusCode::CREATED, Json(resp)))
}

pub async fn approve(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<ApproveEnrollmentRequest>,
) -> Result<Json<EnrollmentResponse>, AppError> {
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

    // Mint a long-lived machine auth token.
    let (machine_token, token_hash) = crate::auth::mint_machine_token(id, &state.auth_config)?;

    let row = repo::approve_enrollment(
        &state.pool,
        id,
        &req.role,
        target_closure.as_deref(),
        extra_config.as_ref(),
        Some(&token_hash),
    )
    .await?;

    match row {
        Some(r) => {
            let machine: Machine = r.into();
            info!(machine_id = %id, "enrollment approved, machine token minted");
            Ok(Json(EnrollmentResponse {
                machine_id: machine.id,
                status: machine.enrollment_status,
                message: "enrollment approved".into(),
                enrolled_by: machine.enrolled_by,
                machine_token: Some(machine_token),
            }))
        }
        None => Err(AppError::NotFound(format!(
            "machine {id} not found or not in pending status"
        ))),
    }
}

/// Returns the enrollment status for a machine.
///
/// When the device first detects it has been approved (machine_token_hash is
/// empty), a fresh machine token is minted, its hash stored, and the raw
/// token included in the response so the device can persist it.
pub async fn enrollment_status(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<EnrollmentResponse>, AppError> {
    let row = repo::get_machine(&state.pool, id).await?;
    match row {
        Some(r) => {
            let machine: Machine = r.into();
            let is_approved = matches!(
                machine.enrollment_status,
                hearth_common::api_types::EnrollmentStatus::Approved
                    | hearth_common::api_types::EnrollmentStatus::Enrolled
                    | hearth_common::api_types::EnrollmentStatus::Provisioning
                    | hearth_common::api_types::EnrollmentStatus::Active
            );

            // Mint machine token on first poll after approval if not already issued.
            let machine_token = if is_approved && machine.machine_token_hash.is_none() {
                match crate::auth::mint_machine_token(id, &state.auth_config) {
                    Ok((token, hash)) => {
                        let _ = repo::set_machine_token_hash(&state.pool, id, &hash).await;
                        info!(machine_id = %id, "minted machine token for device");
                        Some(token)
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "failed to mint machine token");
                        None
                    }
                }
            } else {
                None
            };

            Ok(Json(EnrollmentResponse {
                machine_id: machine.id,
                status: machine.enrollment_status,
                message: "enrollment status".into(),
                enrolled_by: machine.enrolled_by,
                machine_token,
            }))
        }
        None => Err(AppError::NotFound(format!("machine {id} not found"))),
    }
}
