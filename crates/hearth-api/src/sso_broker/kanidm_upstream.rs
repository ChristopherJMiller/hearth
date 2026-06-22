//! Talks to Kanidm as a downstream OIDC client: redeems Kanidm authorization
//! codes and parses Kanidm-issued id_tokens.

use serde::Deserialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum UpstreamError {
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("kanidm token endpoint returned {0}")]
    Status(u16),
    #[error("invalid id_token: {0}")]
    InvalidIdToken(String),
    #[error("missing id_token in upstream response")]
    MissingIdToken,
}

#[derive(Debug, Deserialize)]
pub struct UpstreamTokens {
    pub id_token: Option<String>,
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_in: Option<u64>,
}

pub async fn exchange_kanidm_code(
    http: &reqwest::Client,
    client_id: &str,
    client_secret: &str,
    code: &str,
    redirect_uri: &str,
    token_url: &str,
) -> Result<UpstreamTokens, UpstreamError> {
    let body = [
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", redirect_uri),
    ];
    let resp = http
        .post(token_url)
        .basic_auth(client_id, Some(client_secret))
        .form(&body)
        .send()
        .await?;
    if !resp.status().is_success() {
        return Err(UpstreamError::Status(resp.status().as_u16()));
    }
    let tokens: UpstreamTokens = resp.json().await?;
    Ok(tokens)
}

#[derive(Debug, Clone)]
pub struct UserClaims {
    pub sub: String,
    pub preferred_username: Option<String>,
    pub email: Option<String>,
    pub email_verified: Option<bool>,
    pub groups: Vec<String>,
    pub name: Option<String>,
}

/// Parses an id_token WITHOUT signature verification.
///
/// # Security rationale
///
/// The broker trusts the TLS channel + HTTP Basic auth (client_id + client_secret)
/// to Kanidm's token endpoint. This is the standard "back-channel exchange" model
/// described in RFC 6749 §4.1 and OpenID Connect Core §3.1.3: the id_token arrives
/// over a mutually-authenticated HTTPS connection, not from the user agent, so the
/// TLS transport provides equivalent assurance to signature verification for this
/// flow. The `HEARTH_KANIDM_INSECURE` escape hatch mirrors the existing
/// `identity_sync.rs` pattern and must never be enabled in production.
///
/// # TODO (Phase 2)
///
/// Add JWKS-based ES256/RS256 signature verification to harden against a
/// compromised token endpoint or MITM on the CA. Track as a follow-up
/// once the CA trust chain is stable end-to-end.
pub fn parse_id_token_unverified(id_token: &str) -> Result<UserClaims, UpstreamError> {
    use base64::Engine;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    let parts: Vec<&str> = id_token.split('.').collect();
    if parts.len() != 3 {
        return Err(UpstreamError::InvalidIdToken("expected 3 segments".into()));
    }
    let payload = URL_SAFE_NO_PAD
        .decode(parts[1])
        .map_err(|e| UpstreamError::InvalidIdToken(e.to_string()))?;
    #[derive(Deserialize)]
    struct Claims {
        sub: String,
        preferred_username: Option<String>,
        email: Option<String>,
        email_verified: Option<bool>,
        name: Option<String>,
        #[serde(default)]
        groups: Vec<String>,
    }
    let c: Claims = serde_json::from_slice(&payload)
        .map_err(|e| UpstreamError::InvalidIdToken(e.to_string()))?;
    Ok(UserClaims {
        sub: c.sub,
        preferred_username: c.preferred_username,
        email: c.email,
        email_verified: c.email_verified,
        groups: c.groups,
        name: c.name,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_id_token_payload() {
        // header.payload.signature — header and signature unused for parse_unverified.
        let header = "eyJhbGciOiJFUzI1NiJ9";
        // {"sub":"u-1","preferred_username":"alice","email":"a@b","groups":["hearth-users"]}
        let payload = "eyJzdWIiOiJ1LTEiLCJwcmVmZXJyZWRfdXNlcm5hbWUiOiJhbGljZSIsImVtYWlsIjoiYUBiIiwiZ3JvdXBzIjpbImhlYXJ0aC11c2VycyJdfQ";
        let sig = "xx";
        let tok = format!("{header}.{payload}.{sig}");
        let claims = parse_id_token_unverified(&tok).unwrap();
        assert_eq!(claims.sub, "u-1");
        assert_eq!(claims.preferred_username.unwrap(), "alice");
        assert_eq!(claims.groups, vec!["hearth-users"]);
    }

    #[test]
    fn rejects_malformed_token() {
        assert!(parse_id_token_unverified("not.jwt").is_err());
        assert!(parse_id_token_unverified("").is_err());
    }
}
