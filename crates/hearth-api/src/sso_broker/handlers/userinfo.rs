//! /oidc/userinfo — returns claims for a broker-issued Bearer access token.

// OIDC endpoints use their own auth model (browser sessions / OAuth client
// credentials) and are explicitly exempt from the hearth-api auth extractors.

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Json, Response};
use serde_json::json;

use crate::AppState;
use crate::sso_broker::tokens;

pub async fn userinfo(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let Some(token) = bearer(&headers) else {
        return (StatusCode::UNAUTHORIZED, "missing bearer").into_response();
    };
    let Some(key) = state.sso_broker.signing_key.as_ref() else {
        return (StatusCode::SERVICE_UNAVAILABLE, "sso broker disabled").into_response();
    };
    let issuer = state.sso_broker.issuer_url.as_deref().unwrap_or_default();
    let claims = match tokens::verify_jwt(&token, key, issuer) {
        Ok(c) => c,
        Err(e) => {
            tracing::debug!(error = %e, "userinfo token validation failed");
            return (StatusCode::UNAUTHORIZED, "invalid token").into_response();
        }
    };
    Json(json!({
        "sub": claims.sub,
        "preferred_username": claims.preferred_username,
        "email": claims.email,
        "email_verified": claims.email_verified,
        "name": claims.name,
        "groups": claims.groups,
    }))
    .into_response()
}

fn bearer(headers: &HeaderMap) -> Option<String> {
    let v = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    v.strip_prefix("Bearer ").map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sso_broker::keys::SigningKey;
    use crate::sso_broker::tokens::{self, BrokerClaims};
    use axum::body::Body;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use std::sync::Arc;
    use tower::ServiceExt;

    const PEM: &str = "\
-----BEGIN PRIVATE KEY-----
MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgevZzL1gdAFr88hb2
OF/2NxApJCzGCEDdfSp6VQO30hyhRANCAAQRWz+jn65BtOMvdyHKcvjBeBSDZH2r
1RTwjmYSi9R/zpBnuQ4EiMnCqfMPWiZqB4QdbAd0E7oH50VpuZ1P087G
-----END PRIVATE KEY-----";

    fn enabled_state() -> AppState {
        let mut state = crate::test_helpers::test_app_state();
        let mut cfg = (*state.sso_broker).clone();
        cfg.enabled = true;
        cfg.issuer_url = Some("https://api.hearth.local/oidc".into());
        cfg.signing_key = Some(Arc::new(SigningKey::load_from_pem(PEM).unwrap()));
        state.sso_broker = Arc::new(cfg);
        state
    }

    fn router(state: AppState) -> axum::Router {
        axum::Router::new()
            .route("/oidc/userinfo", axum::routing::get(userinfo))
            .with_state(state)
    }

    fn sample_claims(iss: &str, exp: i64) -> BrokerClaims {
        let now = chrono::Utc::now().timestamp();
        BrokerClaims {
            iss: iss.into(),
            sub: "u-1".into(),
            aud: "synapse".into(),
            exp,
            iat: now,
            jti: "j".into(),
            scope: "openid profile email".into(),
            preferred_username: Some("alice".into()),
            email: Some("a@b".into()),
            email_verified: Some(true),
            name: Some("Alice".into()),
            groups: vec!["hearth-users".into()],
            nonce: None,
        }
    }

    #[tokio::test]
    async fn returns_claims_for_valid_token() {
        let state = enabled_state();
        let key = SigningKey::load_from_pem(PEM).unwrap();
        let now = chrono::Utc::now().timestamp();
        let claims = sample_claims("https://api.hearth.local/oidc", now + 3600);
        let token = tokens::mint_jwt(&claims, &key).unwrap();
        let req = Request::builder()
            .uri("/oidc/userinfo")
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap();
        let resp = router(state).oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["sub"], "u-1");
        assert_eq!(v["preferred_username"], "alice");
        assert_eq!(v["email"], "a@b");
        assert_eq!(v["email_verified"], true);
        assert_eq!(v["name"], "Alice");
        assert_eq!(v["groups"], json!(["hearth-users"]));
    }

    #[tokio::test]
    async fn rejects_missing_bearer() {
        let state = enabled_state();
        let req = Request::builder()
            .uri("/oidc/userinfo")
            .body(Body::empty())
            .unwrap();
        let resp = router(state).oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn rejects_expired_token() {
        let state = enabled_state();
        let key = SigningKey::load_from_pem(PEM).unwrap();
        let claims = sample_claims("https://api.hearth.local/oidc", 1_000);
        let token = tokens::mint_jwt(&claims, &key).unwrap();
        let req = Request::builder()
            .uri("/oidc/userinfo")
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap();
        let resp = router(state).oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }
}
