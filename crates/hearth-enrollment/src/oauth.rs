//! OAuth2 Authorization Code + PKCE client for enrollment.
//!
//! Uses the `openidconnect` crate with OIDC Discovery:
//! 1. Discover endpoints from Kanidm's `.well-known/openid-configuration`
//! 2. Generate PKCE challenge and build the authorization URL
//! 3. Listen on a random localhost port for the redirect callback
//! 4. Exchange the authorization code for an access token

use std::io::{Read, Write};
use std::net::TcpListener;
use std::time::Duration;

use openidconnect::core::CoreProviderMetadata;
use openidconnect::{
    AuthenticationFlow, AuthorizationCode, ClientId, CsrfToken, Nonce, OAuth2TokenResponse,
    PkceCodeChallenge, RedirectUrl, Scope,
};
use tokio::sync::oneshot;
use tracing::{debug, warn};

/// Access token obtained after successful authentication.
#[derive(Debug, Clone)]
pub struct AuthToken {
    pub access_token: String,
}

/// Handle returned from `start_auth_code_flow` — holds the authorization URL
/// and a channel to receive the token when the callback completes.
pub struct AuthFlowHandle {
    /// URL to open in the kiosk browser for user authentication.
    pub auth_url: String,
    /// Receives the token result once the user completes (or fails) auth.
    pub token_rx: oneshot::Receiver<Result<AuthToken, String>>,
}

fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .danger_accept_invalid_certs(true) // dev: self-signed Kanidm certs
        .timeout(Duration::from_secs(15))
        .build()
        .expect("failed to build HTTP client")
}

/// Start the authorization code + PKCE flow.
///
/// Discovers Kanidm's OIDC endpoints, binds a local TCP listener on a random
/// port, builds the authorization URL with PKCE, and spawns a background task
/// that waits for the callback redirect and exchanges the code for a token.
pub async fn start_auth_code_flow(
    kanidm_url: &str,
    client_id: &str,
) -> Result<AuthFlowHandle, String> {
    let http = http_client();

    // Discover OIDC endpoints from Kanidm.
    //
    // We fetch the discovery document manually instead of using `discover_async`
    // because the enrollment VM reaches Kanidm via a different URL (e.g.
    // https://10.0.2.2:8443) than Kanidm's configured origin (https://kanidm.hearth.local:8443).
    // `discover_async` would reject the issuer mismatch. We rewrite the origin
    // in the discovery JSON so all endpoints point to the reachable URL.
    let base = kanidm_url.trim_end_matches('/');
    let discovery_url = format!("{base}/oauth2/openid/{client_id}/.well-known/openid-configuration");

    debug!(%discovery_url, "fetching OIDC discovery document");

    let discovery_json = http
        .get(&discovery_url)
        .send()
        .await
        .map_err(|e| format!("OIDC discovery request failed: {e}"))?
        .text()
        .await
        .map_err(|e| format!("failed to read discovery response: {e}"))?;

    // The discovery document's URLs use Kanidm's configured origin which may
    // differ from the URL we used to reach it. Rewrite so the browser and
    // token exchange both go through the reachable address.
    let rewritten = if let Some(origin) = extract_origin(&discovery_json) {
        if origin != base {
            debug!(kanidm_origin = %origin, reachable_url = %base, "rewriting discovery origin");
            discovery_json.replace(&origin, base)
        } else {
            discovery_json
        }
    } else {
        discovery_json
    };

    let provider_metadata: CoreProviderMetadata = serde_json::from_str(&rewritten)
        .map_err(|e| format!("failed to parse discovery document: {e}"))?;

    // Bind callback listener on a random port
    let listener =
        TcpListener::bind("127.0.0.1:0").map_err(|e| format!("failed to bind listener: {e}"))?;
    let port = listener
        .local_addr()
        .map_err(|e| format!("failed to get listener address: {e}"))?
        .port();

    let redirect_uri = RedirectUrl::new(format!("http://localhost:{port}/callback"))
        .map_err(|e| format!("invalid redirect URI: {e}"))?;

    // Build the OIDC client from discovered metadata
    let oidc_client = openidconnect::core::CoreClient::from_provider_metadata(
        provider_metadata,
        ClientId::new(client_id.to_string()),
        None, // public client, no secret
    )
    .set_redirect_uri(redirect_uri);

    // Generate PKCE challenge
    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

    // Build authorization URL
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

    let auth_url_str = auth_url.to_string();
    debug!(port, auth_url = %auth_url_str, "starting auth code + PKCE flow");

    let (tx, rx) = oneshot::channel();

    // Spawn the callback handler. The client is moved into the closure so its
    // concrete endpoint typestate type is inferred — avoids spelling out the
    // 17-parameter generic signature.
    tokio::spawn(async move {
        let result = async {
            let (code, state) = accept_callback(listener).await?;

            // Verify state (CSRF protection)
            if state.as_deref() != Some(csrf_token.secret().as_str()) {
                warn!("state mismatch in OAuth2 callback");
                return Err("state mismatch — possible CSRF attack".to_string());
            }

            debug!("received authorization code, exchanging for token");

            // Exchange code for token
            let http = http_client();
            let token_response = oidc_client
                .exchange_code(AuthorizationCode::new(code))
                .map_err(|e| format!("failed to build token request: {e}"))?
                .set_pkce_verifier(pkce_verifier)
                .request_async(&http)
                .await
                .map_err(|e| format!("token exchange failed: {e}"))?;

            Ok(AuthToken {
                access_token: token_response.access_token().secret().to_string(),
            })
        }
        .await;
        let _ = tx.send(result);
    });

    Ok(AuthFlowHandle {
        auth_url: auth_url_str,
        token_rx: rx,
    })
}

/// Accept a single HTTP callback on the listener and extract the `code` and
/// `state` query parameters. Returns an error if the IdP sent an error
/// response or if the connection times out.
async fn accept_callback(
    listener: TcpListener,
) -> Result<(String, Option<String>), String> {
    // Accept one connection with a 5-minute timeout
    let stream = tokio::task::spawn_blocking({
        let timeout = Duration::from_secs(300);
        move || {
            listener
                .set_nonblocking(true)
                .map_err(|e| format!("failed to set nonblocking: {e}"))?;

            let start = std::time::Instant::now();
            loop {
                match listener.accept() {
                    Ok((stream, _)) => return Ok(stream),
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        if start.elapsed() > timeout {
                            return Err("authentication timed out (5 minutes)".to_string());
                        }
                        std::thread::sleep(Duration::from_millis(100));
                    }
                    Err(e) => return Err(format!("failed to accept connection: {e}")),
                }
            }
        }
    })
    .await
    .map_err(|e| format!("callback task panicked: {e}"))??;

    // Read the HTTP request
    let mut stream = stream;
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| format!("failed to set read timeout: {e}"))?;

    let mut buf = vec![0u8; 4096];
    let n = stream
        .read(&mut buf)
        .map_err(|e| format!("failed to read request: {e}"))?;
    let request = String::from_utf8_lossy(&buf[..n]);

    // Send a response to the browser
    let html = "<!DOCTYPE html><html><body><h2>Authentication complete</h2>\
                <p>This window will close automatically.</p>\
                <script>window.close()</script></body></html>";
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        html.len(),
        html,
    );
    let _ = stream.write_all(response.as_bytes());

    // Parse the request line to extract query parameters
    let request_line = request
        .lines()
        .next()
        .ok_or_else(|| "empty HTTP request".to_string())?;

    let path = request_line
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| "malformed HTTP request line".to_string())?;

    let url = url::Url::parse(&format!("http://localhost{path}"))
        .map_err(|e| format!("failed to parse callback URL: {e}"))?;

    let params: std::collections::HashMap<String, String> = url.query_pairs().into_owned().collect();

    // Check for error response from Kanidm
    if let Some(err) = params.get("error") {
        let desc = params
            .get("error_description")
            .map(|d| format!(": {d}"))
            .unwrap_or_default();
        warn!(error = %err, "authorization denied");
        return Err(format!("Authorization denied ({err}{desc})"));
    }

    let code = params
        .get("code")
        .ok_or_else(|| "missing authorization code in callback".to_string())?
        .clone();

    let state = params.get("state").cloned();

    Ok((code, state))
}

/// Extract the origin (scheme + host + port) from the `issuer` field of a
/// discovery JSON document. Returns e.g. `https://localhost:8443`.
fn extract_origin(discovery_json: &str) -> Option<String> {
    let doc: serde_json::Value = serde_json::from_str(discovery_json).ok()?;
    let issuer = doc.get("issuer")?.as_str()?;
    let parsed = url::Url::parse(issuer).ok()?;
    Some(format!(
        "{}://{}",
        parsed.scheme(),
        parsed.host_str()?,
    ) + &parsed
        .port()
        .map(|p| format!(":{p}"))
        .unwrap_or_default())
}
