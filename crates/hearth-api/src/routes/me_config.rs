//! Self-service user config endpoints at `/api/v1/me/config`.
//!
//! Allows authenticated users to customize their own environment (git config,
//! editor, shell aliases, session variables) without admin intervention.
//! Admin-only fields (base_role, extra_packages) are preserved but not
//! modifiable through these endpoints.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use hearth_common::api_types::{UpdateMyConfigRequest, UserConfig};
use serde_json::json;

use crate::AppState;
use crate::auth::UserIdentity;
use crate::error::AppError;
use crate::repo;

/// GET /api/v1/me/config — get the authenticated user's environment config.
///
/// Creates a default config if none exists yet.
pub async fn get_my_config(
    UserIdentity(claims): UserIdentity,
    State(state): State<AppState>,
) -> Result<Json<UserConfig>, AppError> {
    let username = claims.username();

    let row = repo::get_user_config(&state.pool, username).await?;
    match row {
        Some(r) => Ok(Json(r.into())),
        None => {
            // Auto-create a default config for the user on first access.
            let overrides = serde_json::Value::Object(Default::default());
            let row =
                repo::upsert_user_config(&state.pool, username, "default", &overrides).await?;
            Ok(Json(row.into()))
        }
    }
}

/// PUT /api/v1/me/config — update the authenticated user's environment config.
///
/// Only user-safe fields are accepted. Admin-only fields (base_role,
/// extra_packages) in the existing config are preserved.
pub async fn update_my_config(
    UserIdentity(claims): UserIdentity,
    State(state): State<AppState>,
    Json(req): Json<UpdateMyConfigRequest>,
) -> Result<(StatusCode, Json<UserConfig>), AppError> {
    let username = claims.username();

    // Load existing config or start from defaults.
    let existing = repo::get_user_config(&state.pool, username).await?;
    let (base_role, mut overrides) = match existing {
        Some(row) => {
            let role = row.base_role;
            let ovr = if row.overrides.is_object() {
                row.overrides
            } else {
                json!({})
            };
            (role, ovr)
        }
        None => ("default".to_string(), json!({})),
    };

    let obj = overrides
        .as_object_mut()
        .ok_or_else(|| AppError::Internal("overrides is not a JSON object".into()))?;

    // Merge user-editable fields into the overrides.
    if let Some(ref name) = req.git_user_name {
        let git = obj.entry("git").or_insert_with(|| json!({}));
        git["user_name"] = json!(name);
    }
    if let Some(ref email) = req.git_user_email {
        let git = obj.entry("git").or_insert_with(|| json!({}));
        git["user_email"] = json!(email);
    }
    if let Some(ref editor) = req.editor {
        obj.insert("editor".into(), json!(editor));
    }
    if let Some(ref aliases) = req.shell_aliases {
        obj.insert("shell_aliases".into(), json!(aliases));
    }
    if let Some(ref vars) = req.session_variables {
        obj.insert("session_variables".into(), json!(vars));
    }

    let row = repo::upsert_user_config(&state.pool, username, &base_role, &overrides).await?;
    Ok((StatusCode::OK, Json(row.into())))
}
