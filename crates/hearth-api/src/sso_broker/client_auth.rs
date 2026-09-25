//! Shared confidential-client authentication for the broker's token-bearing
//! OAuth2 endpoints.
//!
//! Supports both `client_secret_basic` (HTTP Basic auth — per RFC 6749
//! §2.3.1 the client_id and client_secret are individually
//! `application/x-www-form-urlencoded` before being joined with `:` and
//! base64-encoded, so we percent-decode after splitting) and
//! `client_secret_post` (client_id + client_secret sent as form fields).
//! RFC 6749 §2.3.1 also requires that a client use only one authentication
//! method per request, so presenting both is rejected as `invalid_request`.
//!
//! This module is deliberately handler-independent (it knows nothing about
//! axum `Response`/`Json` shapes) so both `/oidc/token` (Task 9) and
//! `/oidc/revoke` (Task 13) can reuse it and format the resulting error
//! however suits their own response contract.

use axum::http::{HeaderMap, StatusCode, header};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;

use crate::sso_broker::clients::{self, BrokerClient};

/// An OAuth2 client-authentication failure, per RFC 6749 §5.2.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientAuthError {
    /// The request itself is malformed (missing, incomplete, or conflicting
    /// client credentials) — HTTP 400.
    InvalidRequest(&'static str),
    /// Client authentication was well-formed but failed — HTTP 401.
    InvalidClient(&'static str),
}

impl ClientAuthError {
    pub fn status(&self) -> StatusCode {
        match self {
            ClientAuthError::InvalidRequest(_) => StatusCode::BAD_REQUEST,
            ClientAuthError::InvalidClient(_) => StatusCode::UNAUTHORIZED,
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            ClientAuthError::InvalidRequest(_) => "invalid_request",
            ClientAuthError::InvalidClient(_) => "invalid_client",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            ClientAuthError::InvalidRequest(d) | ClientAuthError::InvalidClient(d) => d,
        }
    }
}

/// Decodes an RFC 6749 §2.3.1 HTTP Basic `Authorization` header into
/// `(client_id, client_secret)`.
///
/// Returns `Ok(None)` when there is no `Basic` auth scheme present at all
/// (not an error by itself — the caller may still have `client_secret_post`
/// credentials in the form body), and `Err` when a `Basic` header is present
/// but malformed.
fn parse_basic_auth(headers: &HeaderMap) -> Result<Option<(String, String)>, ClientAuthError> {
    let Some(value) = headers.get(header::AUTHORIZATION) else {
        return Ok(None);
    };
    let value = value
        .to_str()
        .map_err(|_| ClientAuthError::InvalidRequest("malformed Authorization header"))?;
    let mut parts = value.splitn(2, ' ');
    let scheme = parts.next().unwrap_or("");
    if !scheme.eq_ignore_ascii_case("basic") {
        return Ok(None);
    }
    let encoded = parts.next().ok_or(ClientAuthError::InvalidRequest(
        "malformed Basic credentials",
    ))?;
    let decoded = STANDARD
        .decode(encoded.trim())
        .map_err(|_| ClientAuthError::InvalidRequest("malformed Basic credentials"))?;
    let decoded = String::from_utf8(decoded)
        .map_err(|_| ClientAuthError::InvalidRequest("malformed Basic credentials"))?;
    let (id, secret) = decoded
        .split_once(':')
        .ok_or(ClientAuthError::InvalidRequest(
            "malformed Basic credentials",
        ))?;
    let id = urlencoding::decode(id)
        .map_err(|_| ClientAuthError::InvalidRequest("malformed Basic credentials"))?
        .into_owned();
    let secret = urlencoding::decode(secret)
        .map_err(|_| ClientAuthError::InvalidRequest("malformed Basic credentials"))?
        .into_owned();
    Ok(Some((id, secret)))
}

/// Authenticates a confidential OAuth2 client against `clients`, accepting
/// either `client_secret_basic` (via `headers`) or `client_secret_post` (via
/// `form_client_id`/`form_client_secret`) — never both at once.
pub fn authenticate<'a>(
    headers: &HeaderMap,
    form_client_id: Option<&str>,
    form_client_secret: Option<&str>,
    clients: &'a [BrokerClient],
) -> Result<&'a BrokerClient, ClientAuthError> {
    let basic = parse_basic_auth(headers)?;
    let (client_id, client_secret) = match (basic, form_client_id, form_client_secret) {
        (Some(_), Some(_), _) | (Some(_), _, Some(_)) => {
            return Err(ClientAuthError::InvalidRequest(
                "must not use more than one client authentication method",
            ));
        }
        (Some((id, secret)), None, None) => (id, secret),
        (None, Some(id), Some(secret)) => (id.to_string(), secret.to_string()),
        (None, Some(_), None) | (None, None, Some(_)) => {
            return Err(ClientAuthError::InvalidRequest(
                "incomplete client_secret_post credentials",
            ));
        }
        (None, None, None) => {
            return Err(ClientAuthError::InvalidRequest(
                "missing client authentication",
            ));
        }
    };
    let Some(client) = clients::lookup(clients, &client_id) else {
        return Err(ClientAuthError::InvalidClient("unknown client_id"));
    };
    if !clients::verify_secret(client, &client_secret) {
        return Err(ClientAuthError::InvalidClient("bad client secret"));
    }
    Ok(client)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    fn client(id: &str, secret: &str) -> BrokerClient {
        BrokerClient {
            client_id: id.into(),
            client_secret_hash: format!("{:x}", Sha256::digest(secret.as_bytes())),
            redirect_uris: vec!["https://example/cb".into()],
            scopes: vec!["openid".into()],
            audience: id.into(),
        }
    }

    fn basic_header(id: &str, secret: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        let raw = format!(
            "{}:{}",
            urlencoding::encode(id),
            urlencoding::encode(secret)
        );
        let value = format!("Basic {}", STANDARD.encode(raw));
        headers.insert(header::AUTHORIZATION, value.parse().unwrap());
        headers
    }

    #[test]
    fn client_secret_basic_success() {
        let clients = vec![client("synapse", "sekret")];
        let headers = basic_header("synapse", "sekret");
        let result = authenticate(&headers, None, None, &clients).unwrap();
        assert_eq!(result.client_id, "synapse");
    }

    #[test]
    fn client_secret_post_success() {
        let clients = vec![client("synapse", "sekret")];
        let headers = HeaderMap::new();
        let result = authenticate(&headers, Some("synapse"), Some("sekret"), &clients).unwrap();
        assert_eq!(result.client_id, "synapse");
    }

    #[test]
    fn both_methods_present_is_invalid_request() {
        let clients = vec![client("synapse", "sekret")];
        let headers = basic_header("synapse", "sekret");
        let err = authenticate(&headers, Some("synapse"), Some("sekret"), &clients).unwrap_err();
        assert_eq!(err.code(), "invalid_request");
        assert_eq!(err.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn missing_credentials_is_invalid_request() {
        let clients = vec![client("synapse", "sekret")];
        let headers = HeaderMap::new();
        let err = authenticate(&headers, None, None, &clients).unwrap_err();
        assert_eq!(err.code(), "invalid_request");
    }

    #[test]
    fn wrong_secret_is_invalid_client_401() {
        let clients = vec![client("synapse", "sekret")];
        let headers = HeaderMap::new();
        let err = authenticate(&headers, Some("synapse"), Some("wrong"), &clients).unwrap_err();
        assert_eq!(err.code(), "invalid_client");
        assert_eq!(err.status(), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn unknown_client_is_invalid_client_401() {
        let clients = vec![client("synapse", "sekret")];
        let headers = HeaderMap::new();
        let err = authenticate(&headers, Some("ghost"), Some("wrong"), &clients).unwrap_err();
        assert_eq!(err.code(), "invalid_client");
        assert_eq!(err.status(), StatusCode::UNAUTHORIZED);
    }
}
