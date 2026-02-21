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
use crate::auth::OperatorIdentity;
use crate::auth::OptionalIdentity;
use crate::cache_token;
use crate::error::AppError;
use crate::repo;

pub async fn enroll(
    State(state): State<AppState>,
    identity: OptionalIdentity,
    Json(req): Json<EnrollmentRequest>,
) -> Result<(StatusCode, Json<EnrollmentResponse>), AppError> {
    // Extract the enrolling user's identity from their auth token (if present).
    let (enrolled_by, is_admin) = match &identity.0 {
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
            let admin = claims.groups.iter().any(|g| g == "hearth-admins");
            (Some(username.to_string()), admin)
        }
        _ => (None, false),
    };

    let row = repo::enroll_machine(
        &state.pool,
        &req.hostname,
        req.hardware_fingerprint.as_deref(),
        enrolled_by.as_deref(),
        req.hardware_report.as_ref(),
        req.serial_number.as_deref(),
        req.hardware_config.as_deref(),
    )
    .await?;

    let machine: Machine = row.into();
    info!(
        machine_id = %machine.id,
        enrolled_by = ?enrolled_by,
        "enrollment request submitted"
    );

    // Auto-approve the first fleet enrollment when an admin is bootstrapping.
    // This solves the chicken-and-egg problem: you need an approved machine to
    // have an admin workstation, but you need an admin to approve machines.
    if is_admin {
        let approved_count = repo::count_approved_machines(&state.pool).await?;
        if approved_count == 0 {
            return auto_approve_enrollment(&state, machine, &req, enrolled_by).await;
        }
    }

    let resp = EnrollmentResponse {
        machine_id: machine.id,
        status: machine.enrollment_status,
        message: "enrollment request submitted, awaiting approval".into(),
        enrolled_by,
        machine_token: None,
        target_closure: None,
        cache_url: None,
        cache_token: None,
        disko_config: None,
    };

    Ok((StatusCode::CREATED, Json(resp)))
}

/// Auto-approve an enrollment for admin bootstrap (first machine in the fleet).
async fn auto_approve_enrollment(
    state: &AppState,
    machine: Machine,
    req: &EnrollmentRequest,
    enrolled_by: Option<String>,
) -> Result<(StatusCode, Json<EnrollmentResponse>), AppError> {
    let id = machine.id;
    let role = req.role_hint.as_deref().unwrap_or("admin");
    let disko_config = "standard".to_string();

    // Mint a short-lived pull-only cache token for this device.
    let extra_config = match cache_token::mint_pull_token(
        &format!("enrollment-{id}"),
        Duration::from_secs(4 * 3600),
    ) {
        Ok(Some(creds)) => {
            info!(machine_id = %id, "minted cache pull token for auto-approved enrollment");
            Some(serde_json::json!({
                "cache_url": creds.cache_url,
                "cache_token": creds.cache_token,
                "disko_config": disko_config,
            }))
        }
        Ok(None) => Some(serde_json::json!({
            "disko_config": disko_config,
        })),
        Err(e) => {
            tracing::warn!(error = %e, "failed to mint cache token, proceeding without");
            Some(serde_json::json!({
                "disko_config": disko_config,
            }))
        }
    };

    // Mint a long-lived machine auth token.
    let (machine_token, token_hash) = crate::auth::mint_machine_token(id, &state.auth_config)?;

    let row = repo::approve_enrollment(
        &state.pool,
        id,
        role,
        None, // no target_closure at bootstrap time
        extra_config.as_ref(),
        Some(&token_hash),
    )
    .await?;

    match row {
        Some(r) => {
            let approved: Machine = r.into();
            let (resp_cache_url, resp_cache_token, resp_disko) = approved
                .extra_config
                .as_ref()
                .map(|ec| {
                    let cu = ec.get("cache_url").and_then(|v| v.as_str()).map(String::from);
                    let ct = ec.get("cache_token").and_then(|v| v.as_str()).map(String::from);
                    let dc = ec.get("disko_config").and_then(|v| v.as_str()).map(String::from);
                    (cu, ct, dc)
                })
                .unwrap_or((None, None, None));

            info!(
                machine_id = %id,
                role = role,
                "auto-approved first fleet enrollment (admin bootstrap)"
            );

            // Queue a build job for the machine.
            let flake_ref = std::env::var("HEARTH_FLAKE_REF")
                .unwrap_or_else(|_| "github:myorg/fleet-config".to_string());
            let machine_filter = serde_json::json!({ "machine_ids": [id.to_string()] });
            match repo::enqueue_build_job(
                &state.pool,
                &flake_ref,
                Some(&machine_filter),
                1,
                1,
                1.0,
            )
            .await
            {
                Ok(job) => {
                    info!(
                        machine_id = %id,
                        build_job_id = %job.id,
                        "queued build job after auto-approved enrollment"
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        machine_id = %id,
                        error = %e,
                        "failed to queue build job after auto-approved enrollment"
                    );
                }
            }

            Ok((
                StatusCode::CREATED,
                Json(EnrollmentResponse {
                    machine_id: approved.id,
                    status: approved.enrollment_status,
                    message: "auto-approved: first fleet enrollment by admin".into(),
                    enrolled_by,
                    machine_token: Some(machine_token),
                    target_closure: approved.target_closure,
                    cache_url: resp_cache_url,
                    cache_token: resp_cache_token,
                    disko_config: resp_disko,
                }),
            ))
        }
        None => {
            // Shouldn't happen — we just created this machine in pending status.
            tracing::error!(machine_id = %id, "auto-approval failed: machine not in pending status");
            Ok((
                StatusCode::CREATED,
                Json(EnrollmentResponse {
                    machine_id: id,
                    status: machine.enrollment_status,
                    message: "enrollment request submitted, awaiting approval".into(),
                    enrolled_by,
                    machine_token: None,
                    target_closure: None,
                    cache_url: None,
                    cache_token: None,
                    disko_config: None,
                }),
            ))
        }
    }
}

pub async fn approve(
    _op: OperatorIdentity,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<ApproveEnrollmentRequest>,
) -> Result<Json<EnrollmentResponse>, AppError> {
    let target_closure = req.target_closure.clone();

    let disko_config = req.disko_config.unwrap_or_else(|| "standard".to_string());

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
                "disko_config": disko_config,
            }))
        }
        Ok(None) => Some(serde_json::json!({
            "disko_config": disko_config,
        })),
        Err(e) => {
            tracing::warn!(error = %e, "failed to mint cache token, proceeding without");
            Some(serde_json::json!({
                "disko_config": disko_config,
            }))
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
            // Extract cache credentials from the stored extra_config.
            let (resp_cache_url, resp_cache_token, resp_disko) = machine
                .extra_config
                .as_ref()
                .map(|ec| {
                    let cu = ec
                        .get("cache_url")
                        .and_then(|v| v.as_str())
                        .map(String::from);
                    let ct = ec
                        .get("cache_token")
                        .and_then(|v| v.as_str())
                        .map(String::from);
                    let dc = ec
                        .get("disko_config")
                        .and_then(|v| v.as_str())
                        .map(String::from);
                    (cu, ct, dc)
                })
                .unwrap_or((None, None, None));

            info!(machine_id = %id, "enrollment approved, machine token minted");

            // Queue an async build job for the machine-specific closure.
            // The build worker will produce a closure incorporating the
            // device's hardware_config and instance data.
            let flake_ref = std::env::var("HEARTH_FLAKE_REF")
                .unwrap_or_else(|_| "github:myorg/fleet-config".to_string());
            let machine_filter = serde_json::json!({
                "machine_ids": [id.to_string()]
            });
            match repo::enqueue_build_job(
                &state.pool,
                &flake_ref,
                Some(&machine_filter),
                1,   // canary_size: 1 for single machine
                1,   // batch_size: 1 for single machine
                1.0, // failure_threshold: no tolerance
            )
            .await
            {
                Ok(job) => {
                    info!(
                        machine_id = %id,
                        build_job_id = %job.id,
                        "queued machine-specific build job after enrollment approval"
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        machine_id = %id,
                        error = %e,
                        "failed to queue build job after enrollment approval"
                    );
                }
            }

            Ok(Json(EnrollmentResponse {
                machine_id: machine.id,
                status: machine.enrollment_status,
                message: "enrollment approved".into(),
                enrolled_by: machine.enrolled_by,
                machine_token: Some(machine_token),
                target_closure: machine.target_closure,
                cache_url: resp_cache_url,
                cache_token: resp_cache_token,
                disko_config: resp_disko,
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

            // Extract provisioning data when the device is approved.
            let (target_closure, resp_cache_url, resp_cache_token, resp_disko) = if is_approved {
                let tc = machine.target_closure.clone();
                let (cu, ct, dc) = machine
                    .extra_config
                    .as_ref()
                    .map(|ec| {
                        let cu = ec
                            .get("cache_url")
                            .and_then(|v| v.as_str())
                            .map(String::from);
                        let ct = ec
                            .get("cache_token")
                            .and_then(|v| v.as_str())
                            .map(String::from);
                        let dc = ec
                            .get("disko_config")
                            .and_then(|v| v.as_str())
                            .map(String::from);
                        (cu, ct, dc)
                    })
                    .unwrap_or((None, None, None));
                (tc, cu, ct, dc)
            } else {
                (None, None, None, None)
            };

            Ok(Json(EnrollmentResponse {
                machine_id: machine.id,
                status: machine.enrollment_status,
                message: "enrollment status".into(),
                enrolled_by: machine.enrolled_by,
                machine_token,
                target_closure,
                cache_url: resp_cache_url,
                cache_token: resp_cache_token,
                disko_config: resp_disko,
            }))
        }
        None => Err(AppError::NotFound(format!("machine {id} not found"))),
    }
}
