//! Credential-based authentication against Kanidm, producing a proper OIDC token.
//!
//! 1. Authenticate via Kanidm's REST API (`/v1/auth`) to establish a session
//! 2. Use that session to programmatically complete an OAuth2 Authorization Code
//!    + PKCE flow (no browser needed)
//! 3. Return the OIDC id_token for use with the Hearth API

use std::time::Duration;

use openidconnect::core::CoreProviderMetadata;
use openidconnect::{
    AuthenticationFlow, AuthorizationCode, ClientId, CsrfToken, Nonce, OAuth2TokenResponse,
    PkceCodeChallenge, RedirectUrl, Scope,
};
use tracing::{debug, info};

/// Access token obtained after successful authentication.
#[derive(Debug, Clone)]
pub struct AuthToken {
    pub access_token: String,
}

fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .danger_accept_invalid_certs(true) // dev: self-signed Kanidm certs
        .cookie_store(true)
        .timeout(Duration::from_secs(15))
        .build()
        .expect("failed to build HTTP client")
}

/// Authenticate with username + password, then obtain a proper OIDC token
/// via the OAuth2 authorization code flow (driven programmatically, no browser).
pub async fn authenticate_with_credentials(
    kanidm_url: &str,
    client_id: &str,
    username: &str,
    password: &str,
) -> Result<AuthToken, String> {
    let base = kanidm_url.trim_end_matches('/');
    let client = http_client();

    // --- Phase 1: Authenticate with Kanidm REST API to get a session ---
    let auth_endpoint = format!("{base}/v1/auth");

    // Step 1: init
    let resp = client
        .post(&auth_endpoint)
        .json(&serde_json::json!({"step": {"init": username}}))
        .send()
        .await
        .map_err(|e| format!("auth request failed: {e}"))?;
    // Kanidm returns 404 for unknown users (body: "nomatchingentries")
    if !resp.status().is_success() {
        return Err("Unknown username".to_string());
    }
    let _ = resp.text().await;

    // Step 2: begin password method
    let resp = client
        .post(&auth_endpoint)
        .json(&serde_json::json!({"step": {"begin": "password"}}))
        .send()
        .await
        .map_err(|e| format!("auth request failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("auth begin returned status {}", resp.status()));
    }
    let _ = resp.text().await;

    // Step 3: submit password
    // Kanidm returns 200 for both success and failure here — check the body.
    let resp = client
        .post(&auth_endpoint)
        .json(&serde_json::json!({"step": {"cred": {"password": password}}}))
        .send()
        .await
        .map_err(|e| format!("auth request failed: {e}"))?;

    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("failed to parse auth response: {e}"))?;

    // Check for denied state (wrong password)
    if let Some(denied) = body.get("state").and_then(|s| s.get("denied")) {
        let reason = denied.as_str().unwrap_or("access denied");
        return Err(format!("Incorrect password ({reason})"));
    }

    let bearer_token = body
        .get("state")
        .and_then(|s| s.get("success"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Authentication failed".to_string())?;

    info!(username = %username, "Kanidm session established, starting OAuth2 flow");

    // --- Phase 2: OAuth2 Authorization Code + PKCE flow using the session ---

    let discovery_url =
        format!("{base}/oauth2/openid/{client_id}/.well-known/openid-configuration");

    debug!(%discovery_url, "fetching OIDC discovery document");

    let discovery_json = client
        .get(&discovery_url)
        .send()
        .await
        .map_err(|e| format!("OIDC discovery request failed: {e}"))?
        .text()
        .await
        .map_err(|e| format!("failed to read discovery response: {e}"))?;

    // Rewrite origin if the discovery doc uses a different hostname than we used,
    // and fix the authorization endpoint path: Kanidm's discovery doc advertises
    // `/ui/oauth2` (the browser consent page) but we need `/oauth2/authorise`
    // (the API endpoint that returns JSON).
    let mut rewritten = if let Some(origin) = extract_origin(&discovery_json) {
        if origin != base {
            debug!(kanidm_origin = %origin, reachable_url = %base, "rewriting discovery origin");
            discovery_json.replace(&origin, base)
        } else {
            discovery_json
        }
    } else {
        discovery_json
    };
    rewritten = rewritten.replace("/ui/oauth2", "/oauth2/authorise");

    let provider_metadata: CoreProviderMetadata = serde_json::from_str(&rewritten)
        .map_err(|e| format!("failed to parse discovery document: {e}"))?;

    // Dummy redirect URI — we intercept the redirect directly, never listen.
    let redirect_uri = RedirectUrl::new("http://localhost:0/callback".to_string())
        .map_err(|e| format!("invalid redirect URI: {e}"))?;

    let oidc_client = openidconnect::core::CoreClient::from_provider_metadata(
        provider_metadata,
        ClientId::new(client_id.to_string()),
        None,
    )
    .set_redirect_uri(redirect_uri);

    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

    let (auth_url, csrf_token, _nonce) = oidc_client
        .authorize_url(
            AuthenticationFlow::<openidconnect::core::CoreResponseType>::AuthorizationCode,
            CsrfToken::new_random,
            Nonce::new_random,
        )
        .add_scope(Scope::new("openid".to_string()))
        .add_scope(Scope::new("profile".to_string()))
        .add_scope(Scope::new("groups".to_string()))
        .set_pkce_challenge(pkce_challenge)
        .url();

    debug!(auth_url = %auth_url, "hitting authorize endpoint with session");

    // Hit the authorize endpoint with our bearer token. Kanidm returns either
    // a 302 redirect (consent pre-granted) or a 200 with a consent form.
    let resp = client
        .get(auth_url.to_string())
        .bearer_auth(bearer_token)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| format!("authorize request failed: {e}"))?;

    let status = resp.status();

    let (code, state) = if status.is_redirection() {
        let location = resp
            .headers()
            .get("location")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| "redirect without Location header".to_string())?;
        extract_code_from_redirect(location)?
    } else if status.as_u16() == 200 {
        // Consent page — Kanidm wraps it as {"ConsentRequested": {"consent_token": "..."}}.
        let body_text = resp
            .text()
            .await
            .map_err(|e| format!("failed to read consent response body: {e}"))?;
        let body: serde_json::Value = serde_json::from_str(&body_text).map_err(|e| {
            let preview = &body_text[..body_text.len().min(300)];
            format!("failed to parse consent response as JSON: {e}\n  body: {preview}")
        })?;

        let consent_token = body
            .get("ConsentRequested")
            .and_then(|cr| cr.get("consent_token"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                format!("unexpected authorize response: {body}")
            })?;

        debug!("approving OAuth2 consent");

        // Kanidm's permit endpoint expects the consent_token as a bare JSON
        // string, not wrapped in an object.
        let consent_resp = client
            .post(format!("{base}/oauth2/authorise/permit"))
            .bearer_auth(bearer_token)
            .header("Content-Type", "application/json")
            .body(format!("\"{consent_token}\""))
            .send()
            .await
            .map_err(|e| format!("consent approval request failed: {e}"))?;

        // Kanidm returns 200 with a Location header (not a 302).
        let location = consent_resp
            .headers()
            .get("location")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| {
                format!(
                    "consent permit returned status {} without Location header",
                    consent_resp.status()
                )
            })?;
        extract_code_from_redirect(location)?
    } else {
        let body_text = resp.text().await.unwrap_or_default();
        return Err(format!(
            "unexpected authorize response status: {status} (body: {})",
            &body_text[..body_text.len().min(500)]
        ));
    };

    // Verify CSRF state
    match state.as_deref() {
        Some(s) if s == csrf_token.secret() => {}
        Some(_) => return Err("state mismatch — possible CSRF".to_string()),
        None => return Err("missing state parameter in redirect".to_string()),
    }

    // --- Phase 3: Exchange authorization code for OIDC token ---

    let exchange_http = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| format!("failed to build HTTP client: {e}"))?;

    let token_response = oidc_client
        .exchange_code(AuthorizationCode::new(code))
        .map_err(|e| format!("failed to build token request: {e}"))?
        .set_pkce_verifier(pkce_verifier)
        .request_async(&exchange_http)
        .await
        .map_err(|e| format!("token exchange failed: {e}"))?;

    // Prefer id_token (carries preferred_username, groups, email)
    let token = token_response
        .extra_fields()
        .id_token()
        .map(|id| id.to_string())
        .unwrap_or_else(|| token_response.access_token().secret().to_string());

    info!("OIDC token obtained successfully");

    Ok(AuthToken {
        access_token: token,
    })
}

fn extract_code_from_redirect(location: &str) -> Result<(String, Option<String>), String> {
    let url =
        url::Url::parse(location).map_err(|e| format!("failed to parse redirect URL: {e}"))?;

    let params: std::collections::HashMap<String, String> =
        url.query_pairs().into_owned().collect();

    if let Some(err) = params.get("error") {
        let desc = params
            .get("error_description")
            .map(|d| format!(": {d}"))
            .unwrap_or_default();
        return Err(format!("authorization denied ({err}{desc})"));
    }

    let code = params
        .get("code")
        .ok_or_else(|| "missing authorization code in redirect".to_string())?
        .clone();

    Ok((code, params.get("state").cloned()))
}

/// Extract the origin (scheme + host + port) from the `issuer` field of a
/// discovery JSON document.
fn extract_origin(discovery_json: &str) -> Option<String> {
    let doc: serde_json::Value = serde_json::from_str(discovery_json).ok()?;
    let issuer = doc.get("issuer")?.as_str()?;
    let parsed = url::Url::parse(issuer).ok()?;
    Some(
        format!("{}://{}", parsed.scheme(), parsed.host_str()?)
            + &parsed.port().map(|p| format!(":{p}")).unwrap_or_default(),
    )
}
