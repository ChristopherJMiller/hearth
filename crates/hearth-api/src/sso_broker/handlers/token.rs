//! /oidc/token — exchange a broker authorization_code for tokens.

// OIDC endpoints use their own auth model (browser sessions / OAuth client
// credentials) and are explicitly exempt from the hearth-api auth extractors.

use axum::extract::{Form, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Json, Response};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::AppState;
use crate::repo;
use crate::sso_broker::{client_auth, pkce, store, tokens};

#[derive(Debug, Deserialize)]
pub struct TokenForm {
    /// Not required at the type level (would otherwise make axum reject a
    /// request missing it with a raw 422 instead of an OAuth2 JSON error) —
    /// checked and reported as `invalid_request` in the handler instead.
    pub grant_type: Option<String>,
    pub code: Option<String>,
    pub redirect_uri: Option<String>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub code_verifier: Option<String>,
}

pub async fn token(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(form): Form<TokenForm>,
) -> Response {
    if !state.sso_broker.enabled {
        return (StatusCode::SERVICE_UNAVAILABLE, "sso broker disabled").into_response();
    }

    let Some(grant_type) = form.grant_type.as_deref() else {
        return oauth_error(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "missing grant_type",
        );
    };
    if grant_type != "authorization_code" {
        return oauth_error(
            StatusCode::BAD_REQUEST,
            "unsupported_grant_type",
            "only authorization_code is supported",
        );
    }
    let Some(code) = form.code.as_deref() else {
        return oauth_error(StatusCode::BAD_REQUEST, "invalid_request", "missing code");
    };
    let Some(redirect_uri) = form.redirect_uri.as_deref() else {
        return oauth_error(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "missing redirect_uri",
        );
    };
    let Some(verifier) = form.code_verifier.as_deref() else {
        return oauth_error(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "missing code_verifier",
        );
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

    let Some(broker_code) = state.sso_broker_code_stash.take(code) else {
        return oauth_error(
            StatusCode::BAD_REQUEST,
            "invalid_grant",
            "code unknown or expired",
        );
    };
    if broker_code.client_id != client.client_id {
        return oauth_error(
            StatusCode::BAD_REQUEST,
            "invalid_grant",
            "code was not issued to this client",
        );
    }
    // RFC 6749 §4.1.3: redirect_uri MUST be identical to the one used in the
    // original /authorize request.
    if broker_code.redirect_uri != redirect_uri {
        return oauth_error(
            StatusCode::BAD_REQUEST,
            "invalid_grant",
            "redirect_uri mismatch",
        );
    }
    if broker_code.code_challenge_method != "S256" {
        return oauth_error(
            StatusCode::BAD_REQUEST,
            "invalid_grant",
            "unsupported pkce method",
        );
    }
    if !pkce::verify_s256(verifier, &broker_code.code_challenge) {
        return oauth_error(
            StatusCode::BAD_REQUEST,
            "invalid_grant",
            "pkce verification failed",
        );
    }
    let Some(key) = state.sso_broker.signing_key.as_ref() else {
        return oauth_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "server_error",
            "no signing key configured",
        );
    };

    let now = chrono::Utc::now().timestamp();
    let id_exp = now + state.sso_broker.id_token_ttl.as_secs() as i64;
    let access_exp = now + state.sso_broker.access_token_ttl.as_secs() as i64;

    let mut claims = tokens::BrokerClaims {
        iss: state.sso_broker.issuer_url.clone().unwrap_or_default(),
        sub: broker_code.user_sub.clone(),
        aud: client.audience.clone(),
        exp: id_exp,
        iat: now,
        jti: tokens::new_jti(),
        scope: broker_code.scope.clone(),
        preferred_username: broker_code.user_username.clone(),
        email: broker_code.user_email.clone(),
        email_verified: broker_code.user_email_verified,
        name: broker_code.user_name.clone(),
        groups: broker_code.user_groups.clone(),
        nonce: broker_code.nonce.clone(),
    };
    let id_token = match tokens::mint_id_token(&claims, key) {
        Ok(t) => t,
        Err(e) => {
            tracing::error!(error = %e, "id_token mint failed");
            return oauth_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "server_error",
                "mint failed",
            );
        }
    };
    // Capture the id_token's jti before it's reassigned below. We deliberately
    // don't recover it by decoding the minted JWT back apart — we already
    // have it in `claims.jti` right here.
    let id_jti = claims.jti.clone();
    let id_expires_at = timestamp_to_utc(id_exp);

    // Access token: fresh jti + its own (typically shorter) TTL, same key.
    claims.jti = tokens::new_jti();
    claims.exp = access_exp;
    let access_jti = claims.jti.clone();
    let access_expires_at = timestamp_to_utc(access_exp);
    let access_token = match tokens::mint_access_token(&claims, key) {
        Ok(t) => t,
        Err(e) => {
            tracing::error!(error = %e, "access_token mint failed");
            return oauth_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "server_error",
                "mint failed",
            );
        }
    };

    // Persist both jtis so `/oidc/userinfo` and `/oidc/revoke` can look them
    // up. Both rows go in a single transaction (a partial write would leave
    // one token unrevocable) and this stays fail-closed: a record failure
    // means neither token is handed to the client, since a token that
    // couldn't be recorded could never be revoked.
    if let Err(e) = store::record_token_pair(
        &state.pool,
        &broker_code.user_sub,
        &client.audience,
        &client.client_id,
        &id_jti,
        id_expires_at,
        &access_jti,
        access_expires_at,
    )
    .await
    {
        tracing::error!(error = %e, "failed to record issued tokens");
        return oauth_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "server_error",
            "failed to record issued token",
        );
    }

    // Audit the issuance. Unlike the recording above this is best-effort,
    // not fail-closed — the tokens are already durably recorded and usable,
    // so a logging failure here shouldn't turn into a 500 for the client.
    if let Err(e) = repo::create_audit_event(
        &state.pool,
        "sso_broker.token_issued",
        Some(&broker_code.user_sub),
        None,
        &json!({
            "client_id": client.client_id,
            "audience": client.audience,
            "scope": broker_code.scope,
            "id_jti": id_jti,
            "access_jti": access_jti,
        }),
    )
    .await
    {
        tracing::error!(error = %e, "failed to record sso_broker.token_issued audit event");
    }

    json_response(
        StatusCode::OK,
        json!({
            "access_token": access_token,
            "token_type": "Bearer",
            "expires_in": state.sso_broker.access_token_ttl.as_secs(),
            "id_token": id_token,
            "scope": broker_code.scope,
        }),
    )
}

/// Converts a `exp`-style Unix timestamp (seconds) into a `DateTime<Utc>`
/// for storage. Falls back to "now" for an out-of-range timestamp rather
/// than panicking — this only affects the stored `expires_at` audit value,
/// never the JWT's own `exp` claim.
fn timestamp_to_utc(ts: i64) -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::from_timestamp(ts, 0).unwrap_or_else(chrono::Utc::now)
}

fn oauth_error(status: StatusCode, error: &str, description: &str) -> Response {
    json_response(
        status,
        json!({
            "error": error,
            "error_description": description,
        }),
    )
}

/// Wraps a JSON body with the RFC 6749 §5.1 requirement that token
/// endpoint responses (success *and* error, since both may carry
/// sensitive material) not be cached.
fn json_response(status: StatusCode, body: Value) -> Response {
    let mut resp = (status, Json(body)).into_response();
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
    use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
    use http_body_util::BodyExt;
    use sha2::{Digest, Sha256};
    use tower::ServiceExt;

    use crate::sso_broker::clients::BrokerClient;
    use crate::sso_broker::sessions::BrokerCode;

    const PEM: &str = "\
-----BEGIN PRIVATE KEY-----
MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgevZzL1gdAFr88hb2
OF/2NxApJCzGCEDdfSp6VQO30hyhRANCAAQRWz+jn65BtOMvdyHKcvjBeBSDZH2r
1RTwjmYSi9R/zpBnuQ4EiMnCqfMPWiZqB4QdbAd0E7oH50VpuZ1P087G
-----END PRIVATE KEY-----";

    fn enabled_state() -> AppState {
        let mut s = crate::test_helpers::test_app_state();
        let mut cfg = (*s.sso_broker).clone();
        cfg.enabled = true;
        cfg.issuer_url = Some("https://api.hearth.local/oidc".into());
        cfg.signing_key = Some(std::sync::Arc::new(
            crate::sso_broker::keys::SigningKey::load_from_pem(PEM).unwrap(),
        ));
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
            .route("/oidc/token", axum::routing::post(token))
            .with_state(state)
    }

    fn insert_valid_code(state: &AppState, verifier: &str) -> String {
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        state.sso_broker_code_stash.insert(BrokerCode {
            client_id: "synapse".into(),
            redirect_uri: "https://chat.hearth.local/cb".into(),
            code_challenge: challenge,
            code_challenge_method: "S256".into(),
            user_sub: "u-1".into(),
            user_username: Some("alice".into()),
            user_email: Some("a@b".into()),
            user_email_verified: Some(true),
            user_name: Some("Alice".into()),
            user_groups: vec!["hearth-users".into()],
            scope: "openid profile email".into(),
            nonce: None,
        })
    }

    async fn post_form(
        state: AppState,
        body: String,
        extra_headers: Vec<(&str, String)>,
    ) -> Response {
        let mut builder = Request::builder()
            .method("POST")
            .uri("/oidc/token")
            .header("content-type", "application/x-www-form-urlencoded");
        for (k, v) in extra_headers {
            builder = builder.header(k, v);
        }
        let req = builder.body(Body::from(body)).unwrap();
        router(state).oneshot(req).await.unwrap()
    }

    // These two used to assert a 200 with minted tokens against
    // `test_helpers::test_app_state()`'s dangling lazy pool. Now that
    // `/oidc/token` persists every minted jti (fail-closed on a record
    // error), that pool can never reach a 200 here — the DB-backed success
    // path lives in `crates/hearth-api/tests/api/sso_broker.rs` against a
    // real Postgres instead. What's left worth asserting in-module is the
    // fail-closed contract itself: a record failure must produce a 500
    // `server_error` and leak no tokens, regardless of which client-auth
    // method got the request that far.

    #[tokio::test]
    async fn record_failure_returns_500_and_no_tokens_via_form_auth() {
        let state = enabled_state();
        let verifier = "abcdef123456abcdef123456abcdef123456abcdef123456";
        let code_key = insert_valid_code(&state, verifier);
        let body = format!(
            "grant_type=authorization_code&code={}&redirect_uri=https%3A%2F%2Fchat.hearth.local%2Fcb&client_id=synapse&client_secret=sekret&code_verifier={}",
            urlencoding::encode(&code_key),
            verifier,
        );
        let resp = post_form(state, body, vec![]).await;
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(
            resp.headers().get(header::CACHE_CONTROL).unwrap(),
            "no-store"
        );
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["error"], "server_error");
        assert!(v.get("id_token").is_none());
        assert!(v.get("access_token").is_none());
    }

    #[tokio::test]
    async fn record_failure_returns_500_and_no_tokens_via_client_secret_basic() {
        let state = enabled_state();
        let verifier = "abcdef123456abcdef123456abcdef123456abcdef123456";
        let code_key = insert_valid_code(&state, verifier);
        let body = format!(
            "grant_type=authorization_code&code={}&redirect_uri=https%3A%2F%2Fchat.hearth.local%2Fcb&code_verifier={}",
            urlencoding::encode(&code_key),
            verifier,
        );
        let basic = STANDARD.encode("synapse:sekret");
        let resp = post_form(
            state,
            body,
            vec![("authorization", format!("Basic {basic}"))],
        )
        .await;
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["error"], "server_error");
        assert!(v.get("id_token").is_none());
        assert!(v.get("access_token").is_none());
    }

    #[tokio::test]
    async fn rejects_both_client_auth_methods_present() {
        let state = enabled_state();
        let verifier = "abcdef123456abcdef123456abcdef123456abcdef123456";
        let code_key = insert_valid_code(&state, verifier);
        let body = format!(
            "grant_type=authorization_code&code={}&redirect_uri=https%3A%2F%2Fchat.hearth.local%2Fcb&client_id=synapse&client_secret=sekret&code_verifier={}",
            urlencoding::encode(&code_key),
            verifier,
        );
        let basic = STANDARD.encode("synapse:sekret");
        let resp = post_form(
            state,
            body,
            vec![("authorization", format!("Basic {basic}"))],
        )
        .await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["error"], "invalid_request");
    }

    #[tokio::test]
    async fn rejects_bad_pkce() {
        let state = enabled_state();
        let code_key = state.sso_broker_code_stash.insert(BrokerCode {
            client_id: "synapse".into(),
            redirect_uri: "https://chat.hearth.local/cb".into(),
            code_challenge: "wrong".into(),
            code_challenge_method: "S256".into(),
            user_sub: "u-1".into(),
            user_username: None,
            user_email: None,
            user_email_verified: None,
            user_name: None,
            user_groups: vec![],
            scope: "openid".into(),
            nonce: None,
        });
        let body = format!(
            "grant_type=authorization_code&code={}&redirect_uri=https%3A%2F%2Fchat.hearth.local%2Fcb&client_id=synapse&client_secret=sekret&code_verifier=abc",
            urlencoding::encode(&code_key),
        );
        let resp = post_form(state, body, vec![]).await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["error"], "invalid_grant");
    }

    #[tokio::test]
    async fn rejects_redirect_uri_mismatch() {
        let state = enabled_state();
        let verifier = "abcdef123456abcdef123456abcdef123456abcdef123456";
        let code_key = insert_valid_code(&state, verifier);
        let body = format!(
            "grant_type=authorization_code&code={}&redirect_uri=https%3A%2F%2Fevil.example%2Fcb&client_id=synapse&client_secret=sekret&code_verifier={}",
            urlencoding::encode(&code_key),
            verifier,
        );
        let resp = post_form(state, body, vec![]).await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["error"], "invalid_grant");
    }

    #[tokio::test]
    async fn rejects_code_redeemed_by_a_different_client() {
        // A code minted for "synapse" must not be redeemable by a second,
        // fully-legitimate confidential client ("nextcloud") even though
        // its own credentials check out fine — this is the check at
        // `broker_code.client_id != client.client_id` above, and doesn't
        // touch the DB (it returns before any `store::` call), so it's a
        // plain unit test.
        let state = enabled_state();
        let mut cfg = (*state.sso_broker).clone();
        cfg.clients.push(BrokerClient {
            client_id: "nextcloud".into(),
            client_secret_hash: format!("{:x}", Sha256::digest(b"other-sekret")),
            redirect_uris: vec!["https://cloud.hearth.local/cb".into()],
            scopes: vec!["openid".into()],
            audience: "nextcloud".into(),
        });
        let mut state = state;
        state.sso_broker = std::sync::Arc::new(cfg);

        let verifier = "abcdef123456abcdef123456abcdef123456abcdef123456";
        let code_key = insert_valid_code(&state, verifier);
        let body = format!(
            "grant_type=authorization_code&code={}&redirect_uri=https%3A%2F%2Fchat.hearth.local%2Fcb&client_id=nextcloud&client_secret=other-sekret&code_verifier={}",
            urlencoding::encode(&code_key),
            verifier,
        );
        let resp = post_form(state, body, vec![]).await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["error"], "invalid_grant");
        assert_eq!(v["error_description"], "code was not issued to this client");
    }

    #[tokio::test]
    async fn rejects_missing_grant_type_as_oauth_error_not_422() {
        let state = enabled_state();
        let body = "code=whatever".to_string();
        let resp = post_form(state, body, vec![]).await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["error"], "invalid_request");
    }

    #[tokio::test]
    async fn rejects_unsupported_grant_type() {
        let state = enabled_state();
        let body = "grant_type=client_credentials".to_string();
        let resp = post_form(state, body, vec![]).await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["error"], "unsupported_grant_type");
    }

    #[tokio::test]
    async fn rejects_unknown_client_with_401() {
        let state = enabled_state();
        let body =
            "grant_type=authorization_code&code=x&redirect_uri=https%3A%2F%2Fchat.hearth.local%2Fcb&client_id=ghost&client_secret=x&code_verifier=x"
                .to_string();
        let resp = post_form(state, body, vec![]).await;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["error"], "invalid_client");
    }

    #[tokio::test]
    async fn basic_auth_401_carries_www_authenticate() {
        let state = enabled_state();
        let body =
            "grant_type=authorization_code&code=x&redirect_uri=https%3A%2F%2Fchat.hearth.local%2Fcb&code_verifier=x"
                .to_string();
        let basic = STANDARD.encode("synapse:wrong-secret");
        let resp = post_form(
            state,
            body,
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
        let body =
            "grant_type=authorization_code&code=x&redirect_uri=https%3A%2F%2Fchat.hearth.local%2Fcb&client_id=synapse&client_secret=wrong&code_verifier=x"
                .to_string();
        let resp = post_form(state, body, vec![]).await;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        assert!(resp.headers().get(header::WWW_AUTHENTICATE).is_none());
    }

    #[tokio::test]
    async fn returns_503_when_disabled() {
        let state = crate::test_helpers::test_app_state();
        let body = "grant_type=authorization_code".to_string();
        let resp = post_form(state, body, vec![]).await;
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    }
}
