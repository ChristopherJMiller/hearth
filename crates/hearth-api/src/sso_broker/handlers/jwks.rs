use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use serde_json::json;

use crate::AppState;

pub async fn jwks(State(state): State<AppState>) -> Response {
    let Some(key) = state.sso_broker.signing_key.as_ref() else {
        return (StatusCode::SERVICE_UNAVAILABLE, "sso broker disabled").into_response();
    };
    Json(json!({ "keys": [key.jwk_public()] })).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    #[tokio::test]
    async fn jwks_empty_when_disabled() {
        let state = crate::test_helpers::test_app_state();
        let router = axum::Router::new()
            .route("/oidc/.well-known/jwks.json", axum::routing::get(jwks))
            .with_state(state);
        let req = Request::builder()
            .uri("/oidc/.well-known/jwks.json")
            .body(Body::empty())
            .unwrap();
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn jwks_returns_single_key_when_enabled() {
        const PEM: &str = "\
-----BEGIN PRIVATE KEY-----
MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgevZzL1gdAFr88hb2
OF/2NxApJCzGCEDdfSp6VQO30hyhRANCAAQRWz+jn65BtOMvdyHKcvjBeBSDZH2r
1RTwjmYSi9R/zpBnuQ4EiMnCqfMPWiZqB4QdbAd0E7oH50VpuZ1P087G
-----END PRIVATE KEY-----";
        let key = crate::sso_broker::keys::SigningKey::load_from_pem(PEM).unwrap();
        let mut state = crate::test_helpers::test_app_state();
        // Mutate the Arc; in a real test we'd construct AppState with a custom config.
        let mut cfg = (*state.sso_broker).clone();
        cfg.signing_key = Some(std::sync::Arc::new(key));
        cfg.enabled = true;
        state.sso_broker = std::sync::Arc::new(cfg);

        let router = axum::Router::new()
            .route("/oidc/.well-known/jwks.json", axum::routing::get(jwks))
            .with_state(state);
        let req = Request::builder()
            .uri("/oidc/.well-known/jwks.json")
            .body(Body::empty())
            .unwrap();
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["keys"][0]["kty"], "EC");
    }
}
