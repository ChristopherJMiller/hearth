//! /oidc/callback — Kanidm hands us back an authorization code + our upstream_state.
//! We exchange the code with Kanidm, parse the id_token, mint our own
//! authorization code, and 302 the user agent back to the original downstream
//! app with the broker's code + original state.
//!
//! This endpoint is intentionally exempt from the standard auth extractors
//! (UserIdentity, MachineIdentity, etc.) — it is part of the OIDC protocol
//! flow, which uses its own auth model (browser sessions + OAuth client
//! credentials). See the global constraints in the SSO broker plan.

use axum::extract::{Query, State};
use axum::http::{HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use std::time::Instant;

use crate::AppState;
use crate::sso_broker::kanidm_upstream;
use crate::sso_broker::sessions::BrokerCode;

#[derive(Debug, Deserialize)]
pub struct CallbackQuery {
    pub code: String,
    pub state: String,
}

pub async fn callback(
    State(state): State<AppState>,
    Query(q): Query<CallbackQuery>,
) -> Response {
    if !state.sso_broker.enabled {
        return (StatusCode::SERVICE_UNAVAILABLE, "sso broker disabled").into_response();
    }
    let Some(original) = state.sso_authorize_stash.take(&q.state) else {
        return (StatusCode::BAD_REQUEST, "unknown or expired state").into_response();
    };
    let token_url = state.sso_broker.kanidm_token_url.as_deref().unwrap_or_default();
    let our_redirect = format!(
        "{}/callback",
        state.sso_broker.issuer_url.as_deref().unwrap_or_default()
    );
    let client_id = state
        .sso_broker
        .kanidm_oauth2_client_id
        .as_deref()
        .unwrap_or_default();
    let client_secret = state
        .sso_broker
        .kanidm_oauth2_client_secret
        .as_deref()
        .unwrap_or_default();
    let tokens = match kanidm_upstream::exchange_kanidm_code(
        &state.http_client,
        client_id,
        client_secret,
        &q.code,
        &our_redirect,
        token_url,
    )
    .await
    {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!(error = %e, "kanidm code exchange failed");
            return (StatusCode::BAD_GATEWAY, "upstream auth failed").into_response();
        }
    };
    let Some(id_token) = tokens.id_token else {
        return (StatusCode::BAD_GATEWAY, "no id_token from upstream").into_response();
    };
    let claims = match kanidm_upstream::parse_id_token_unverified(&id_token) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(error = %e, "upstream id_token parse failed");
            return (StatusCode::BAD_GATEWAY, "invalid upstream id_token").into_response();
        }
    };
    let code = state.sso_broker_code_stash.insert(BrokerCode {
        client_id: original.client_id.clone(),
        redirect_uri: original.redirect_uri.clone(),
        code_challenge: original.code_challenge.clone(),
        code_challenge_method: original.code_challenge_method.clone(),
        user_sub: claims.sub,
        user_username: claims.preferred_username,
        user_email: claims.email,
        user_name: claims.name,
        user_groups: claims.groups,
        scope: original.scope.clone(),
        nonce: None,
        created_at: Instant::now(),
    });
    let url = format!(
        "{}?code={}&state={}",
        original.redirect_uri,
        urlencoding::encode(&code),
        urlencoding::encode(&original.state),
    );
    let mut resp = (StatusCode::FOUND, "").into_response();
    resp.headers_mut().insert(
        axum::http::header::LOCATION,
        HeaderValue::from_str(&url).unwrap_or(HeaderValue::from_static("/")),
    );
    resp
}
