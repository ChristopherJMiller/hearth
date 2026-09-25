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

use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;

use crate::sso_broker::clients::{self, BrokerClient};

/// `WWW-Authenticate` value for a 401 triggered by a request that attempted
/// HTTP Basic auth. RFC 6749 §5.2 requires the realm be present; the value
/// is otherwise opaque to clients.
const WWW_AUTHENTICATE_BASIC: &str = "Basic realm=\"hearth-sso-broker\"";

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
    let id = decode_form_component(id)?;
    let secret = decode_form_component(secret)?;
    Ok(Some((id, secret)))
}

/// Decodes one `application/x-www-form-urlencoded` component (RFC 6749
/// §2.3.1 requires the client_id/client_secret each be encoded this way
/// before being joined with `:`). `urlencoding::decode` alone only handles
/// `%XX` escapes — form-urlencoding also uses a literal `+` for space, so
/// that has to be turned into `%20` before percent-decoding or a `+` in a
/// client_id/secret round-trips as itself instead of a space.
fn decode_form_component(s: &str) -> Result<String, ClientAuthError> {
    let percent_escaped = s.replace('+', "%20");
    urlencoding::decode(&percent_escaped)
        .map(|c| c.into_owned())
        .map_err(|_| ClientAuthError::InvalidRequest("malformed Basic credentials"))
}

/// True if the request's `Authorization` header names the `Basic` scheme,
/// regardless of whether its credentials are well-formed. Used to decide
/// whether a 401 response must carry `WWW-Authenticate` (RFC 6749 §5.2:
/// "If the client attempted to authenticate via the Authorization request
/// header field, the authorization server MUST ... include the
/// WWW-Authenticate response header field matching the authentication
/// scheme used by the client").
pub fn attempted_basic_auth(headers: &HeaderMap) -> bool {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split_whitespace().next())
        .is_some_and(|scheme| scheme.eq_ignore_ascii_case("basic"))
}

/// Builds the `WWW-Authenticate` header value a caller should attach to the
/// response for `err`, or `None` when RFC 6749 §5.2 doesn't call for one
/// (anything other than a 401, or a 401 that didn't come from an attempted
/// Basic-auth request — e.g. `client_secret_post` with a bad secret).
/// Handler-independent like the rest of this module: callers attach it to
/// whatever response shape they use.
pub fn www_authenticate(err: ClientAuthError, headers: &HeaderMap) -> Option<HeaderValue> {
    if err.status() == StatusCode::UNAUTHORIZED && attempted_basic_auth(headers) {
        Some(HeaderValue::from_static(WWW_AUTHENTICATE_BASIC))
    } else {
        None
    }
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
    fn basic_auth_plus_decodes_to_space() {
        // Simulate a client that form-encoded each half before joining with
        // ':' and base64-encoding (RFC 6749 §2.3.1) — a literal '+' in the
        // wire format must decode to a space, not survive as '+'.
        let clients = vec![client("synapse client", "sekret value")];
        let raw = "synapse+client:sekret+value";
        let value = format!("Basic {}", STANDARD.encode(raw));
        let mut headers = HeaderMap::new();
        headers.insert(header::AUTHORIZATION, value.parse().unwrap());
        let result = authenticate(&headers, None, None, &clients).unwrap();
        assert_eq!(result.client_id, "synapse client");
    }

    #[test]
    fn attempted_basic_auth_true_for_basic_scheme() {
        assert!(attempted_basic_auth(&basic_header("synapse", "sekret")));
    }

    #[test]
    fn attempted_basic_auth_false_without_header() {
        assert!(!attempted_basic_auth(&HeaderMap::new()));
    }

    #[test]
    fn attempted_basic_auth_false_for_other_scheme() {
        let mut headers = HeaderMap::new();
        headers.insert(header::AUTHORIZATION, "Bearer sometoken".parse().unwrap());
        assert!(!attempted_basic_auth(&headers));
    }

    #[test]
    fn www_authenticate_present_for_basic_401() {
        let headers = basic_header("synapse", "wrong");
        let err = ClientAuthError::InvalidClient("bad client secret");
        let value = www_authenticate(err, &headers).unwrap();
        assert_eq!(value, WWW_AUTHENTICATE_BASIC);
    }

    #[test]
    fn www_authenticate_absent_for_post_401() {
        let err = ClientAuthError::InvalidClient("bad client secret");
        assert!(www_authenticate(err, &HeaderMap::new()).is_none());
    }

    #[test]
    fn www_authenticate_absent_for_400_even_with_basic_header() {
        let headers = basic_header("synapse", "sekret");
        let err = ClientAuthError::InvalidRequest("must not use more than one method");
        assert!(www_authenticate(err, &headers).is_none());
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
