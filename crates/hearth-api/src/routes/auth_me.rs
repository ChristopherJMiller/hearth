//! GET /api/v1/auth/me — Returns the authenticated user's identity from token claims.

use axum::Json;
use serde_json::{Value, json};

use crate::auth::UserIdentity;

pub async fn me(UserIdentity(claims): UserIdentity) -> Json<Value> {
    Json(json!({
        "sub": claims.sub,
        "username": claims.preferred_username,
        "email": claims.email,
        "groups": claims.groups,
    }))
}
