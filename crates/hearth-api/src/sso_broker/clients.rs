//! Broker's OAuth2 client registry. MVP: declarative via env var (JSON).
//! Future: DB-backed with admin API.

use serde::Deserialize;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

#[derive(Debug, Clone, Deserialize)]
pub struct BrokerClient {
    pub client_id: String,
    /// Hex-encoded SHA-256 of the client secret.
    pub client_secret_hash: String,
    pub redirect_uris: Vec<String>,
    pub scopes: Vec<String>,
    pub audience: String,
}

pub fn lookup<'a>(clients: &'a [BrokerClient], client_id: &str) -> Option<&'a BrokerClient> {
    clients.iter().find(|c| c.client_id == client_id)
}

pub fn verify_secret(client: &BrokerClient, presented: &str) -> bool {
    let hash = format!("{:x}", Sha256::digest(presented.as_bytes()));
    hash.as_bytes()
        .ct_eq(client.client_secret_hash.as_bytes())
        .into()
}

pub fn from_env() -> Vec<BrokerClient> {
    let raw = std::env::var("HEARTH_SSO_BROKER_CLIENTS_JSON").unwrap_or_default();
    if raw.is_empty() {
        return Vec::new();
    }
    serde_json::from_str(&raw).unwrap_or_else(|e| {
        tracing::warn!(error = %e, "failed to parse HEARTH_SSO_BROKER_CLIENTS_JSON");
        Vec::new()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn client(id: &str, secret: &str) -> BrokerClient {
        BrokerClient {
            client_id: id.into(),
            client_secret_hash: format!("{:x}", Sha256::digest(secret.as_bytes())),
            redirect_uris: vec!["https://example/cb".into()],
            scopes: vec!["openid".into()],
            audience: id.into(),
        }
    }

    #[test]
    fn lookup_finds_existing() {
        let c = client("synapse", "s3cret");
        let all = vec![c.clone()];
        assert_eq!(lookup(&all, "synapse").unwrap().client_id, "synapse");
        assert!(lookup(&all, "missing").is_none());
    }

    #[test]
    fn verify_correct_secret() {
        let c = client("synapse", "s3cret");
        assert!(verify_secret(&c, "s3cret"));
        assert!(!verify_secret(&c, "wrong"));
        assert!(!verify_secret(&c, ""));
    }

    #[test]
    fn from_env_returns_empty_when_unset() {
        // Don't rely on this being unset across the test suite — set it explicitly.
        unsafe { std::env::remove_var("HEARTH_SSO_BROKER_CLIENTS_JSON") };
        assert!(from_env().is_empty());
    }
}
