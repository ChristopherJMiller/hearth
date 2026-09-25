//! OIDC discovery document — RFC OpenID Connect Discovery 1.0.

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use serde_json::{Value, json};

use crate::AppState;

pub async fn discovery(State(state): State<AppState>) -> Response {
    if !state.sso_broker.enabled {
        return (StatusCode::SERVICE_UNAVAILABLE, "sso broker disabled").into_response();
    }
    let issuer = state.sso_broker.issuer_url.as_deref().unwrap_or_default();
    let doc = build_discovery_doc(issuer);
    Json(doc).into_response()
}

fn build_discovery_doc(issuer: &str) -> Value {
    json!({
        "issuer": issuer,
        "authorization_endpoint": format!("{issuer}/authorize"),
        "token_endpoint": format!("{issuer}/token"),
        "userinfo_endpoint": format!("{issuer}/userinfo"),
        "jwks_uri": format!("{issuer}/.well-known/jwks.json"),
        "revocation_endpoint": format!("{issuer}/revoke"),
        "response_types_supported": ["code"],
        // No refresh_token grant — not implemented.
        "grant_types_supported": ["authorization_code"],
        "subject_types_supported": ["public"],
        "id_token_signing_alg_values_supported": ["ES256"],
        "scopes_supported": ["openid", "profile", "email", "groups"],
        "claims_supported": ["sub", "preferred_username", "name", "email", "email_verified", "groups"],
        "token_endpoint_auth_methods_supported": ["client_secret_basic", "client_secret_post"],
        "code_challenge_methods_supported": ["S256"],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovery_doc_has_required_fields() {
        let doc = build_discovery_doc("https://api.hearth.example/oidc");
        assert_eq!(doc["issuer"], "https://api.hearth.example/oidc");
        assert_eq!(
            doc["authorization_endpoint"],
            "https://api.hearth.example/oidc/authorize"
        );
        assert_eq!(doc["id_token_signing_alg_values_supported"][0], "ES256");
        assert!(doc["jwks_uri"].as_str().unwrap().ends_with("/jwks.json"));
        assert_eq!(doc["grant_types_supported"], json!(["authorization_code"]));
    }

    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    #[tokio::test]
    async fn returns_503_when_disabled() {
        let state = crate::test_helpers::test_app_state();
        let router = axum::Router::new()
            .route(
                "/oidc/.well-known/openid-configuration",
                axum::routing::get(discovery),
            )
            .with_state(state);
        let req = Request::builder()
            .uri("/oidc/.well-known/openid-configuration")
            .body(Body::empty())
            .unwrap();
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    }
}
