//! ES256 (P-256) signing key for broker-issued OIDC tokens.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use p256::ecdsa::{SigningKey as EcdsaSigning, signature::Signer};
use p256::pkcs8::{DecodePrivateKey, EncodePublicKey};
use sha2::{Digest, Sha256};
use std::io;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SigningKeyError {
    #[error("invalid PEM: {0}")]
    InvalidPem(String),
    #[error("io error: {0}")]
    Io(#[from] io::Error),
    #[error("key too short")]
    KeyTooShort,
}

#[derive(Clone, Debug)]
pub struct SigningKey {
    inner: EcdsaSigning,
    kid: String,
    x_b64: String,
    y_b64: String,
}

impl SigningKey {
    pub fn load_from_pem(pem: &str) -> Result<Self, SigningKeyError> {
        let inner = EcdsaSigning::from_pkcs8_pem(pem)
            .map_err(|e| SigningKeyError::InvalidPem(e.to_string()))?;
        let verifying = inner.verifying_key();
        let encoded = verifying.to_encoded_point(false);
        let x = encoded
            .x()
            .ok_or_else(|| SigningKeyError::InvalidPem("missing x coord".into()))?;
        let y = encoded
            .y()
            .ok_or_else(|| SigningKeyError::InvalidPem("missing y coord".into()))?;
        let x_b64 = URL_SAFE_NO_PAD.encode(x);
        let y_b64 = URL_SAFE_NO_PAD.encode(y);
        // Stable kid: first 32 chars of base64url(SHA-256(SubjectPublicKeyInfo DER))
        let spki = verifying
            .to_public_key_der()
            .map_err(|_e| SigningKeyError::InvalidPem("failed to encode public key DER".into()))?;
        let digest = Sha256::digest(spki.as_bytes());
        let kid = URL_SAFE_NO_PAD
            .encode(&digest[..])
            .chars()
            .take(32)
            .collect();
        Ok(SigningKey {
            inner,
            kid,
            x_b64,
            y_b64,
        })
    }

    pub fn load_from_path(path: &Path) -> Result<Self, SigningKeyError> {
        let pem = std::fs::read_to_string(path)?;
        Self::load_from_pem(&pem)
    }

    pub fn kid(&self) -> &str {
        &self.kid
    }

    pub fn jwk_public(&self) -> serde_json::Value {
        serde_json::json!({
            "kty": "EC",
            "crv": "P-256",
            "alg": "ES256",
            "use": "sig",
            "kid": self.kid,
            "x": self.x_b64,
            "y": self.y_b64,
        })
    }

    pub fn sign(&self, header_b64: &str, payload_b64: &str) -> Result<String, SigningKeyError> {
        let message = format!("{header_b64}.{payload_b64}");
        let sig: p256::ecdsa::Signature = self.inner.sign(message.as_bytes());
        Ok(URL_SAFE_NO_PAD.encode(sig.to_bytes()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_PEM: &str = "\
-----BEGIN PRIVATE KEY-----
MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgevZzL1gdAFr88hb2
OF/2NxApJCzGCEDdfSp6VQO30hyhRANCAAQRWz+jn65BtOMvdyHKcvjBeBSDZH2r
1RTwjmYSi9R/zpBnuQ4EiMnCqfMPWiZqB4QdbAd0E7oH50VpuZ1P087G
-----END PRIVATE KEY-----";

    #[test]
    fn loads_valid_pem() {
        let key = SigningKey::load_from_pem(TEST_PEM).expect("valid PEM");
        assert!(!key.kid().is_empty());
        let jwk = key.jwk_public();
        assert_eq!(jwk["kty"], "EC");
        assert_eq!(jwk["crv"], "P-256");
        assert_eq!(jwk["alg"], "ES256");
        assert_eq!(jwk["use"], "sig");
        assert_eq!(jwk["kid"], key.kid());
        assert!(jwk["x"].is_string());
        assert!(jwk["y"].is_string());
    }

    #[test]
    fn rejects_garbage_pem() {
        let err = SigningKey::load_from_pem("not a pem").unwrap_err();
        matches!(err, SigningKeyError::InvalidPem(_));
    }

    #[test]
    fn signs_and_produces_64_byte_signature_base64url() {
        let key = SigningKey::load_from_pem(TEST_PEM).unwrap();
        let header = base64_url("eyJhbGciOiJFUzI1NiJ9");
        let payload = base64_url("eyJzdWIiOiJ0ZXN0In0");
        let sig_b64 = key.sign(&header, &payload).unwrap();
        // base64url of 64 bytes (P-256 r||s concat) is 86 chars, no padding
        assert_eq!(sig_b64.len(), 86);
        assert!(!sig_b64.contains('='));
        assert!(!sig_b64.contains('+'));
        assert!(!sig_b64.contains('/'));
    }

    fn base64_url(s: &str) -> String {
        s.to_string()
    }

    #[test]
    fn kid_is_stable_across_loads() {
        let a = SigningKey::load_from_pem(TEST_PEM).unwrap();
        let b = SigningKey::load_from_pem(TEST_PEM).unwrap();
        assert_eq!(a.kid(), b.kid());
    }
}
