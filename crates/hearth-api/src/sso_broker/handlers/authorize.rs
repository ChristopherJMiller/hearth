//! /oidc/authorize — entry point for OAuth2 authorization code + PKCE.
//!
//! Validates the incoming request against the configured client registry,
//! stashes the request, and 302s the user agent to Kanidm with a fresh
//! upstream state token.

// OIDC endpoints use their own auth model (browser sessions / OAuth client
// credentials) and are explicitly exempt from the hearth-api auth extractors.

use axum::extract::{Query, State};
use axum::http::{HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use std::time::Instant;

use crate::AppState;
use crate::sso_broker::clients;
use crate::sso_broker::pkce;
use crate::sso_broker::sessions::AuthorizeRequest;

#[derive(Debug, Deserialize)]
pub struct AuthorizeQuery {
    pub response_type: String,
    pub client_id: String,
    pub redirect_uri: String,
    pub scope: String,
    pub state: String,
    pub code_challenge: String,
    pub code_challenge_method: String,
    /// Synapse (and OIDC clients generally) always send this and reject
    /// id_tokens that don't echo it back.
    #[serde(default)]
    pub nonce: Option<String>,
}

pub async fn authorize(State(state): State<AppState>, Query(q): Query<AuthorizeQuery>) -> Response {
    if !state.sso_broker.enabled {
        return (StatusCode::SERVICE_UNAVAILABLE, "sso broker disabled").into_response();
    }
    if q.response_type != "code" {
        return bad_request("unsupported_response_type", "only response_type=code");
    }
    if q.code_challenge_method != "S256" {
        return bad_request("invalid_request", "code_challenge_method must be S256");
    }
    let Some(client) = clients::lookup(&state.sso_broker.clients, &q.client_id) else {
        return bad_request("invalid_client", "unknown client_id");
    };
    if !client.redirect_uris.iter().any(|u| u == &q.redirect_uri) {
        return bad_request("invalid_request", "redirect_uri not registered");
    }
    // Fresh PKCE pair for the broker's own (upstream) leg toward Kanidm —
    // independent of the downstream app's PKCE pair above. Kanidm 1.10
    // enforces PKCE on OAuth2 clients by default.
    let upstream_pkce_verifier = pkce::generate_verifier();
    let upstream_pkce_challenge = pkce::s256_challenge(&upstream_pkce_verifier);

    // Stash request keyed by an upstream_state we send to Kanidm.
    let upstream_state = state.sso_authorize_stash.insert(AuthorizeRequest {
        client_id: q.client_id.clone(),
        redirect_uri: q.redirect_uri.clone(),
        scope: q.scope.clone(),
        state: q.state.clone(),
        code_challenge: q.code_challenge.clone(),
        code_challenge_method: q.code_challenge_method.clone(),
        nonce: q.nonce.clone(),
        upstream_pkce_verifier,
        created_at: Instant::now(),
    });
    let broker_redirect = format!(
        "{}/callback",
        state.sso_broker.issuer_url.as_deref().unwrap_or_default()
    );
    let kanidm_authz = state
        .sso_broker
        .kanidm_authorize_url
        .as_deref()
        .unwrap_or_default();
    let kanidm_client_id = state
        .sso_broker
        .kanidm_oauth2_client_id
        .as_deref()
        .unwrap_or_default();
    let url = format!(
        "{kanidm_authz}?response_type=code&client_id={cid}&redirect_uri={rd}&scope=openid+profile+email+groups&state={st}&code_challenge={cc}&code_challenge_method=S256",
        cid = urlencoding::encode(kanidm_client_id),
        rd = urlencoding::encode(&broker_redirect),
        st = urlencoding::encode(&upstream_state),
        cc = urlencoding::encode(&upstream_pkce_challenge),
    );
    let mut resp = (StatusCode::FOUND, "").into_response();
    resp.headers_mut().insert(
        axum::http::header::LOCATION,
        HeaderValue::from_str(&url).unwrap_or(HeaderValue::from_static("/")),
    );
    resp
}

fn bad_request(error: &str, desc: &str) -> Response {
    (
        StatusCode::BAD_REQUEST,
        format!("{{\"error\":\"{error}\",\"error_description\":\"{desc}\"}}"),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sso_broker::clients::BrokerClient;
    use axum::body::Body;
    use axum::http::Request;
    use sha2::{Digest, Sha256};
    use tower::ServiceExt;

    fn enabled_state_with_client() -> AppState {
        let mut s = crate::test_helpers::test_app_state();
        let mut cfg = (*s.sso_broker).clone();
        cfg.enabled = true;
        cfg.issuer_url = Some("https://api.hearth.local/oidc".into());
        cfg.kanidm_authorize_url = Some("https://kanidm.hearth.local:8443/ui/oauth2".into());
        cfg.kanidm_oauth2_client_id = Some("hearth-sso-broker".into());
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

    #[tokio::test]
    async fn redirects_to_kanidm() {
        let state = enabled_state_with_client();
        let router = axum::Router::new()
            .route("/oidc/authorize", axum::routing::get(authorize))
            .with_state(state);
        let q = "response_type=code&client_id=synapse&redirect_uri=https%3A%2F%2Fchat.hearth.local%2Fcb&scope=openid&state=xyz&code_challenge=abc&code_challenge_method=S256";
        let req = Request::builder()
            .uri(format!("/oidc/authorize?{q}"))
            .body(Body::empty())
            .unwrap();
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::FOUND);
        let loc = resp
            .headers()
            .get(axum::http::header::LOCATION)
            .unwrap()
            .to_str()
            .unwrap();
        assert!(loc.starts_with("https://kanidm.hearth.local:8443/ui/oauth2?"));
        assert!(loc.contains("client_id=hearth-sso-broker"));
        assert!(loc.contains("state="));
    }

    #[tokio::test]
    async fn round_trips_nonce_and_generates_upstream_pkce() {
        let state = enabled_state_with_client();
        let router = axum::Router::new()
            .route("/oidc/authorize", axum::routing::get(authorize))
            .with_state(state.clone());
        let q = "response_type=code&client_id=synapse&redirect_uri=https%3A%2F%2Fchat.hearth.local%2Fcb&scope=openid&state=xyz&code_challenge=abc&code_challenge_method=S256&nonce=n-0S6_WzA2Mj";
        let req = Request::builder()
            .uri(format!("/oidc/authorize?{q}"))
            .body(Body::empty())
            .unwrap();
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::FOUND);
        let loc = resp
            .headers()
            .get(axum::http::header::LOCATION)
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();

        // The broker must present its own PKCE challenge to Kanidm.
        assert!(
            loc.contains("code_challenge_method=S256"),
            "missing upstream code_challenge_method in {loc}"
        );
        let upstream_challenge =
            query_param(&loc, "code_challenge").expect("missing upstream code_challenge param");
        let upstream_state = query_param(&loc, "state").expect("missing upstream state param");

        // Pull the stashed request back out via the upstream state and check
        // both the nonce round-trip and that the verifier matches the
        // challenge sent to Kanidm.
        let stashed = state
            .sso_authorize_stash
            .take(&upstream_state)
            .expect("upstream state should be stashed");
        assert_eq!(stashed.nonce.as_deref(), Some("n-0S6_WzA2Mj"));
        assert!(
            !stashed.upstream_pkce_verifier.is_empty(),
            "upstream_pkce_verifier should be generated"
        );
        assert!(
            crate::sso_broker::pkce::verify_s256(
                &stashed.upstream_pkce_verifier,
                &upstream_challenge
            ),
            "stashed verifier must match the challenge sent to Kanidm"
        );
    }

    fn query_param(url: &str, key: &str) -> Option<String> {
        let query = url.split('?').nth(1)?;
        query.split('&').find_map(|pair| {
            let (k, v) = pair.split_once('=')?;
            if k == key {
                Some(urlencoding::decode(v).ok()?.into_owned())
            } else {
                None
            }
        })
    }

    #[tokio::test]
    async fn rejects_unknown_client() {
        let state = enabled_state_with_client();
        let router = axum::Router::new()
            .route("/oidc/authorize", axum::routing::get(authorize))
            .with_state(state);
        let q = "response_type=code&client_id=ghost&redirect_uri=https%3A%2F%2Fchat.hearth.local%2Fcb&scope=openid&state=xyz&code_challenge=abc&code_challenge_method=S256";
        let req = Request::builder()
            .uri(format!("/oidc/authorize?{q}"))
            .body(Body::empty())
            .unwrap();
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }
}
