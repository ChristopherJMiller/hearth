//! /oidc/revoke — RFC 7009 token revocation.

// OIDC endpoints use their own auth model (browser sessions / OAuth client
// credentials) and are explicitly exempt from the hearth-api auth extractors.

use axum::extract::{Form, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Json, Response};
use serde::Deserialize;
use serde_json::json;

use crate::AppState;
use crate::repo;
use crate::sso_broker::{client_auth, store, tokens};

#[derive(Debug, Deserialize)]
pub struct RevokeForm {
    /// Not required at the type level (would otherwise make axum reject a
    /// request missing it with a raw 422 instead of an OAuth2 JSON error) —
    /// checked and reported as `invalid_request` in the handler instead.
    pub token: Option<String>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
}

/// RFC 7009 token revocation. This is a confidential-client endpoint like
/// `/oidc/token`, so it authenticates via the same
/// [`client_auth::authenticate`] helper — `client_secret_basic` or
/// `client_secret_post`, never both.
///
/// Revocation is scoped to the authenticated client:
/// [`store::revoke`] only updates rows whose `client_id` matches the caller,
/// so a jti minted for a *different* client is indistinguishable from an
/// unknown one. Per RFC 7009 §2.2 ("the authorization server responds with
/// HTTP status code 200 if the token has been revoked successfully or if
/// the client submitted an invalid token"), both cases — and a token this
/// broker can't even parse — return 200 with no side effect, so the
/// response can never be used as an oracle for whether some other client's
/// token exists.
pub async fn revoke(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(form): Form<RevokeForm>,
) -> Response {
    if !state.sso_broker.enabled {
        return (StatusCode::SERVICE_UNAVAILABLE, "sso broker disabled").into_response();
    }

    let Some(token) = form.token.as_deref() else {
        return oauth_error(StatusCode::BAD_REQUEST, "invalid_request", "missing token");
    };

    let client = match client_auth::authenticate(
        &headers,
        form.client_id.as_deref(),
        form.client_secret.as_deref(),
        &state.sso_broker.clients,
    ) {
        Ok(client) => client,
        Err(e) => {
            let mut resp = oauth_error(e.status(), e.code(), e.description());
            if let Some(v) = client_auth::www_authenticate(e, &headers) {
                resp.headers_mut().insert(header::WWW_AUTHENTICATE, v);
            }
            return resp;
        }
    };

    // RFC 7009 §2.2: a token this broker doesn't recognize the shape of
    // (can't even extract a jti from) is a silent no-op, not an error.
    let Some(jti) = tokens::extract_jti_unverified(token) else {
        return StatusCode::OK.into_response();
    };

    match store::revoke(&state.pool, &jti, &client.client_id).await {
        Ok(true) => {
            // Best-effort, not fail-closed: the revocation itself already
            // took effect, so a logging failure here shouldn't be reported
            // to the client as an error.
            if let Err(e) = repo::create_audit_event(
                &state.pool,
                "sso_broker.token_revoked",
                Some(&client.client_id),
                None,
                &json!({ "jti": jti }),
            )
            .await
            {
                tracing::error!(
                    error = %e,
                    "failed to record sso_broker.token_revoked audit event"
                );
            }
        }
        Ok(false) => {
            // Unknown jti, already revoked, or (deliberately) issued to a
            // different client_id — no state changed, nothing to audit.
        }
        Err(e) => {
            tracing::error!(error = %e, "revoke failed");
            return (StatusCode::INTERNAL_SERVER_ERROR, "db error").into_response();
        }
    }

    StatusCode::OK.into_response()
}

fn oauth_error(status: StatusCode, error: &str, description: &str) -> Response {
    let mut resp = (
        status,
        Json(json!({
            "error": error,
            "error_description": description,
        })),
    )
        .into_response();
    let headers = resp.headers_mut();
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(header::PRAGMA, HeaderValue::from_static("no-cache"));
    resp
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use base64::Engine;
    use base64::engine::general_purpose::STANDARD;
    use http_body_util::BodyExt;
    use sha2::{Digest, Sha256};
    use tower::ServiceExt;

    use crate::sso_broker::clients::BrokerClient;

    fn enabled_state() -> AppState {
        let mut s = crate::test_helpers::test_app_state();
        let mut cfg = (*s.sso_broker).clone();
        cfg.enabled = true;
        cfg.clients = vec![BrokerClient {
            client_id: "synapse".into(),
            client_secret_hash: format!("{:x}", Sha256::digest(b"sekret")),
            redirect_uris: vec!["https://chat.hearth.local/cb".into()],
            scopes: vec!["openid".into()],
            audience: "synapse".into(),
        }];
        s.sso_broker = std::sync::Arc::new(cfg);
        s
    }

    fn router(state: AppState) -> axum::Router {
        axum::Router::new()
            .route("/oidc/revoke", axum::routing::post(revoke))
            .with_state(state)
    }

    async fn post_form(
        state: AppState,
        body: String,
        extra_headers: Vec<(&str, String)>,
    ) -> Response {
        let mut builder = Request::builder()
            .method("POST")
            .uri("/oidc/revoke")
            .header("content-type", "application/x-www-form-urlencoded");
        for (k, v) in extra_headers {
            builder = builder.header(k, v);
        }
        let req = builder.body(Body::from(body)).unwrap();
        router(state).oneshot(req).await.unwrap()
    }

    #[tokio::test]
    async fn returns_503_when_disabled() {
        let state = crate::test_helpers::test_app_state();
        let resp = post_form(
            state,
            "token=x&client_id=synapse&client_secret=sekret".into(),
            vec![],
        )
        .await;
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn rejects_missing_token_as_oauth_error_not_422() {
        let state = enabled_state();
        let resp = post_form(
            state,
            "client_id=synapse&client_secret=sekret".into(),
            vec![],
        )
        .await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["error"], "invalid_request");
    }

    #[tokio::test]
    async fn rejects_missing_client_auth_as_oauth_error_not_422() {
        let state = enabled_state();
        let resp = post_form(state, "token=x".into(), vec![]).await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["error"], "invalid_request");
    }

    #[tokio::test]
    async fn rejects_unknown_client_with_401() {
        let state = enabled_state();
        let resp = post_form(
            state,
            "token=x&client_id=ghost&client_secret=whatever".into(),
            vec![],
        )
        .await;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["error"], "invalid_client");
    }

    #[tokio::test]
    async fn basic_auth_401_carries_www_authenticate() {
        let state = enabled_state();
        let basic = STANDARD.encode("synapse:wrong-secret");
        let resp = post_form(
            state,
            "token=x".into(),
            vec![("authorization", format!("Basic {basic}"))],
        )
        .await;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            resp.headers().get(header::WWW_AUTHENTICATE).unwrap(),
            "Basic realm=\"hearth-sso-broker\""
        );
    }

    #[tokio::test]
    async fn post_auth_401_has_no_www_authenticate() {
        let state = enabled_state();
        let resp = post_form(
            state,
            "token=x&client_id=synapse&client_secret=wrong".into(),
            vec![],
        )
        .await;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        assert!(resp.headers().get(header::WWW_AUTHENTICATE).is_none());
    }

    #[tokio::test]
    async fn malformed_token_is_a_200_noop_without_touching_the_db() {
        // Well-authenticated client, but `token` isn't a parseable 3-segment
        // JWT — RFC 7009 §2.2 says this is a no-op, not an error. This path
        // never reaches `store::revoke`, so it's safe to assert against
        // `test_app_state()`'s dangling pool.
        let state = enabled_state();
        let resp = post_form(
            state,
            "token=not-a-jwt&client_id=synapse&client_secret=sekret".into(),
            vec![],
        )
        .await;
        assert_eq!(resp.status(), StatusCode::OK);
    }
}
