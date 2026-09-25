//! Broker token claims + minting.
//!
//! PKCE verification for the downstream app's (broker's own) code exchange
//! lives in [`crate::sso_broker::pkce::verify_s256`] — this module only
//! handles claim shaping and JWT minting.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::sso_broker::keys::{SigningKey, SigningKeyError};

#[derive(Debug, Error)]
pub enum MintError {
    #[error("signing failed: {0}")]
    Signing(#[from] SigningKeyError),
    #[error("json serialize: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Error)]
pub enum VerifyError {
    #[error("malformed signing key: {0}")]
    MalformedKey(String),
    #[error("token validation failed: {0}")]
    Jwt(#[from] jsonwebtoken::errors::Error),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BrokerClaims {
    pub iss: String,
    pub sub: String,
    pub aud: String,
    pub exp: i64,
    pub iat: i64,
    pub jti: String,
    pub scope: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preferred_username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email_verified: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nonce: Option<String>,
}

pub fn new_jti() -> String {
    Uuid::new_v4().to_string()
}

pub fn mint_jwt(claims: &BrokerClaims, key: &SigningKey) -> Result<String, MintError> {
    let header = serde_json::json!({
        "alg": "ES256",
        "typ": "JWT",
        "kid": key.kid(),
    });
    let header_b64 = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&header)?);
    let payload_b64 = URL_SAFE_NO_PAD.encode(serde_json::to_vec(claims)?);
    let sig = key.sign(&header_b64, &payload_b64)?;
    Ok(format!("{header_b64}.{payload_b64}.{sig}"))
}

/// Mints a broker id_token. For MVP this is identical in shape to
/// [`mint_access_token`] — both are ES256 JWTs signed with the broker's
/// signing key — but they are kept as distinct functions since the `aud`
/// semantics (and possibly claim sets) are expected to diverge once the
/// broker grows a real userinfo/introspection story.
pub fn mint_id_token(claims: &BrokerClaims, key: &SigningKey) -> Result<String, MintError> {
    mint_jwt(claims, key)
}

/// Mints a broker access_token. See [`mint_id_token`] for why this is kept
/// separate despite sharing an implementation for MVP.
pub fn mint_access_token(claims: &BrokerClaims, key: &SigningKey) -> Result<String, MintError> {
    mint_jwt(claims, key)
}

/// Verifies a broker-issued ES256 JWT (id_token or access_token) and returns
/// its claims.
///
/// Checks signature, `exp` (both handled by `jsonwebtoken`), and that `iss`
/// equals `expected_issuer`. Shared by the userinfo handler today and
/// intended for the token-revocation check added in a later task.
pub fn verify_jwt(
    token: &str,
    key: &SigningKey,
    expected_issuer: &str,
) -> Result<BrokerClaims, VerifyError> {
    let jwk = key.jwk_public();
    let x = jwk["x"]
        .as_str()
        .ok_or_else(|| VerifyError::MalformedKey("missing x coordinate".into()))?;
    let y = jwk["y"]
        .as_str()
        .ok_or_else(|| VerifyError::MalformedKey("missing y coordinate".into()))?;
    let decoding_key = DecodingKey::from_ec_components(x, y)
        .map_err(|e| VerifyError::MalformedKey(e.to_string()))?;

    let mut validation = Validation::new(Algorithm::ES256);
    // userinfo/introspection accept any aud the broker issued for.
    validation.validate_aud = false;
    validation.set_issuer(&[expected_issuer]);

    let data = decode::<BrokerClaims>(token, &decoding_key, &validation)?;
    Ok(data.claims)
}

/// Extracts the `jti` claim from a JWT's payload segment without verifying
/// its signature or expiry.
///
/// This is the single jti-extraction helper in the broker (it must not be
/// duplicated) — used by `/oidc/revoke`, where RFC 7009 §2.2 says a token
/// the server doesn't recognize or can't even parse is a silent no-op, not
/// an error, so revocation must be able to look at a possibly-malformed or
/// already-expired token without `verify_jwt`'s signature/`exp` checks
/// rejecting it outright. This can't be used as an oracle: the caller still
/// has to hold a valid client credential, and [`crate::sso_broker::store::revoke`]
/// scopes the update to that client's own `client_id`, so guessing a
/// foreign jti here verifies nothing and revokes nothing outside the
/// caller's own tokens.
pub fn extract_jti_unverified(token: &str) -> Option<String> {
    let mut parts = token.split('.');
    let _header = parts.next()?;
    let payload = parts.next()?;
    let _signature = parts.next()?;
    if parts.next().is_some() {
        return None; // not a 3-segment JWT
    }
    let payload = URL_SAFE_NO_PAD.decode(payload).ok()?;
    let value: serde_json::Value = serde_json::from_slice(&payload).ok()?;
    value.get("jti")?.as_str().map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PEM: &str = "\
-----BEGIN PRIVATE KEY-----
MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgevZzL1gdAFr88hb2
OF/2NxApJCzGCEDdfSp6VQO30hyhRANCAAQRWz+jn65BtOMvdyHKcvjBeBSDZH2r
1RTwjmYSi9R/zpBnuQ4EiMnCqfMPWiZqB4QdbAd0E7oH50VpuZ1P087G
-----END PRIVATE KEY-----";

    fn sample_claims() -> BrokerClaims {
        BrokerClaims {
            iss: "https://api.hearth.local/oidc".into(),
            sub: "u-1".into(),
            aud: "synapse".into(),
            exp: 1_999_999_999,
            iat: 1_700_000_000,
            jti: "j-1".into(),
            scope: "openid profile email".into(),
            preferred_username: Some("alice".into()),
            email: Some("a@b".into()),
            email_verified: Some(true),
            name: Some("Alice".into()),
            groups: vec!["hearth-users".into()],
            nonce: None,
        }
    }

    #[test]
    fn mint_jwt_produces_three_segments() {
        let key = SigningKey::load_from_pem(PEM).unwrap();
        let jwt = mint_jwt(&sample_claims(), &key).unwrap();
        let segs: Vec<&str> = jwt.split('.').collect();
        assert_eq!(segs.len(), 3);
    }

    #[test]
    fn mint_id_token_and_mint_access_token_both_produce_valid_jwts() {
        let key = SigningKey::load_from_pem(PEM).unwrap();
        let claims = sample_claims();
        let id_token = mint_id_token(&claims, &key).unwrap();
        let access_token = mint_access_token(&claims, &key).unwrap();
        assert_eq!(id_token.split('.').count(), 3);
        assert_eq!(access_token.split('.').count(), 3);
    }

    #[test]
    fn new_jti_generates_distinct_values() {
        assert_ne!(new_jti(), new_jti());
    }

    #[test]
    fn verify_jwt_round_trips_valid_token() {
        let key = SigningKey::load_from_pem(PEM).unwrap();
        let claims = sample_claims();
        let token = mint_jwt(&claims, &key).unwrap();
        let verified = verify_jwt(&token, &key, &claims.iss).unwrap();
        assert_eq!(verified.sub, claims.sub);
        assert_eq!(verified.preferred_username, claims.preferred_username);
    }

    #[test]
    fn verify_jwt_rejects_wrong_issuer() {
        let key = SigningKey::load_from_pem(PEM).unwrap();
        let claims = sample_claims();
        let token = mint_jwt(&claims, &key).unwrap();
        let err = verify_jwt(&token, &key, "https://not-the-issuer.example").unwrap_err();
        assert!(matches!(err, VerifyError::Jwt(_)));
    }

    #[test]
    fn verify_jwt_rejects_tampered_payload() {
        let key = SigningKey::load_from_pem(PEM).unwrap();
        let claims = sample_claims();
        let token = mint_jwt(&claims, &key).unwrap();
        let mut segs: Vec<&str> = token.split('.').collect();
        // Swap in a payload for a different subject, signed with the same
        // header/signature — the signature check must reject this.
        let mut tampered_claims = sample_claims();
        tampered_claims.sub = "attacker".into();
        let tampered_payload =
            URL_SAFE_NO_PAD.encode(serde_json::to_vec(&tampered_claims).unwrap());
        segs[1] = &tampered_payload;
        let tampered = segs.join(".");
        let err = verify_jwt(&tampered, &key, &claims.iss).unwrap_err();
        assert!(matches!(err, VerifyError::Jwt(_)));
    }

    #[test]
    fn extract_jti_unverified_reads_jti_from_valid_jwt() {
        let key = SigningKey::load_from_pem(PEM).unwrap();
        let claims = sample_claims();
        let token = mint_jwt(&claims, &key).unwrap();
        assert_eq!(extract_jti_unverified(&token), Some(claims.jti));
    }

    #[test]
    fn extract_jti_unverified_works_on_expired_token() {
        // Revocation must be able to read the jti out of a token whose exp
        // has already passed — verify_jwt would reject it outright.
        let key = SigningKey::load_from_pem(PEM).unwrap();
        let mut claims = sample_claims();
        claims.exp = 1_000;
        let token = mint_jwt(&claims, &key).unwrap();
        assert_eq!(extract_jti_unverified(&token), Some(claims.jti));
    }

    #[test]
    fn extract_jti_unverified_ignores_signature() {
        // No signature verification happens at all — a tampered/garbage
        // signature segment still yields the jti from the payload.
        let key = SigningKey::load_from_pem(PEM).unwrap();
        let claims = sample_claims();
        let token = mint_jwt(&claims, &key).unwrap();
        let mut segs: Vec<&str> = token.split('.').collect();
        segs[2] = "not-a-real-signature";
        let tampered = segs.join(".");
        assert_eq!(extract_jti_unverified(&tampered), Some(claims.jti));
    }

    #[test]
    fn extract_jti_unverified_rejects_wrong_segment_count() {
        assert_eq!(extract_jti_unverified("only.two"), None);
        assert_eq!(extract_jti_unverified("a.b.c.d"), None);
        assert_eq!(extract_jti_unverified("no-dots-at-all"), None);
    }

    #[test]
    fn extract_jti_unverified_rejects_non_base64_payload() {
        assert_eq!(extract_jti_unverified("header.not!base64.sig"), None);
    }

    #[test]
    fn extract_jti_unverified_rejects_payload_without_jti() {
        let payload = URL_SAFE_NO_PAD.encode(
            serde_json::to_vec(&serde_json::json!({
                "sub": "u-1",
            }))
            .unwrap(),
        );
        let token = format!("header.{payload}.sig");
        assert_eq!(extract_jti_unverified(&token), None);
    }

    #[test]
    fn verify_jwt_rejects_expired_token() {
        let key = SigningKey::load_from_pem(PEM).unwrap();
        let mut claims = sample_claims();
        claims.exp = 1_000; // long past, well outside the default leeway
        let token = mint_jwt(&claims, &key).unwrap();
        let err = verify_jwt(&token, &key, &claims.iss).unwrap_err();
        assert!(matches!(err, VerifyError::Jwt(_)));
    }
}
