//! OAuth2 Device Authorization Grant (RFC 8628) client.
//!
//! Implements the device flow used during enrollment:
//! 1. Request device + user codes from the authorization server
//! 2. Display the verification URL + user code to the operator
//! 3. Poll the token endpoint until the user completes authentication

use serde::Deserialize;
use tracing::{debug, warn};

/// State returned from the device authorization endpoint.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct DeviceFlowState {
    pub verification_uri: String,
    pub verification_uri_complete: Option<String>,
    pub user_code: String,
    pub device_code: String,
    pub interval: u64,
    pub expires_in: u64,
}

/// Token response after successful authentication.
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct TokenResponse {
    pub access_token: String,
    #[serde(default)]
    pub token_type: Option<String>,
    #[serde(default)]
    pub expires_in: Option<u64>,
}

/// Status while polling for user authentication.
#[derive(Debug)]
pub enum PollStatus {
    Pending,
    SlowDown,
    Success(TokenResponse),
    Expired,
    AccessDenied,
    Error(String),
}

#[derive(Deserialize)]
struct DeviceAuthResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    #[serde(default)]
    verification_uri_complete: Option<String>,
    #[serde(default = "default_interval")]
    interval: u64,
    expires_in: u64,
}

fn default_interval() -> u64 {
    5
}

#[derive(Deserialize)]
struct TokenErrorResponse {
    error: String,
    #[serde(default)]
    error_description: Option<String>,
}

fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .danger_accept_invalid_certs(true) // dev: self-signed Kanidm certs
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .expect("failed to build HTTP client")
}

/// Start the device authorization flow.
///
/// Sends a request to the Kanidm device authorization endpoint and returns
/// the verification URI, user code, and device code for polling.
pub async fn start_device_flow(
    kanidm_url: &str,
    client_id: &str,
) -> Result<DeviceFlowState, String> {
    let url = format!("{}/oauth2/device", kanidm_url.trim_end_matches('/'));
    debug!(url = %url, client_id = %client_id, "starting device authorization flow");

    let client = http_client();
    let resp = client
        .post(&url)
        .form(&[("client_id", client_id), ("scope", "openid profile groups")])
        .send()
        .await
        .map_err(|e| format!("device auth request failed: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("device auth failed ({status}): {body}"));
    }

    let auth: DeviceAuthResponse = resp
        .json()
        .await
        .map_err(|e| format!("failed to parse device auth response: {e}"))?;

    debug!(
        user_code = %auth.user_code,
        verification_uri = %auth.verification_uri,
        expires_in = auth.expires_in,
        "device flow started"
    );

    Ok(DeviceFlowState {
        verification_uri: auth.verification_uri,
        verification_uri_complete: auth.verification_uri_complete,
        user_code: auth.user_code,
        device_code: auth.device_code,
        interval: auth.interval,
        expires_in: auth.expires_in,
    })
}

/// Poll the token endpoint for the device flow.
///
/// Call this at the interval specified in `DeviceFlowState`. Returns the
/// current poll status — keep polling while `PollStatus::Pending`.
pub async fn poll_for_token(kanidm_url: &str, client_id: &str, device_code: &str) -> PollStatus {
    let url = format!("{}/oauth2/token", kanidm_url.trim_end_matches('/'));

    let client = http_client();
    let resp = match client
        .post(&url)
        .form(&[
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ("client_id", client_id),
            ("device_code", device_code),
        ])
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => return PollStatus::Error(format!("token request failed: {e}")),
    };

    if resp.status().is_success() {
        match resp.json::<TokenResponse>().await {
            Ok(token) => return PollStatus::Success(token),
            Err(e) => return PollStatus::Error(format!("failed to parse token response: {e}")),
        }
    }

    // Parse the error response
    let body = match resp.text().await {
        Ok(b) => b,
        Err(e) => return PollStatus::Error(format!("failed to read error response: {e}")),
    };

    let error: TokenErrorResponse = match serde_json::from_str(&body) {
        Ok(e) => e,
        Err(_) => return PollStatus::Error(format!("unexpected error response: {body}")),
    };

    match error.error.as_str() {
        "authorization_pending" => {
            debug!("authorization pending, will retry");
            PollStatus::Pending
        }
        "slow_down" => {
            debug!("told to slow down");
            PollStatus::SlowDown
        }
        "expired_token" => {
            warn!("device code expired");
            PollStatus::Expired
        }
        "access_denied" => {
            warn!("access denied by user");
            PollStatus::AccessDenied
        }
        other => {
            let desc = error.error_description.unwrap_or_default();
            PollStatus::Error(format!("{other}: {desc}"))
        }
    }
}
