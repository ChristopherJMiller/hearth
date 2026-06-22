//! Configuration for the SSO broker.

use crate::sso_broker::keys::SigningKey;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct SsoBrokerConfig {
    pub enabled: bool,
    pub issuer_url: Option<String>,
    pub signing_key: Option<Arc<SigningKey>>,
    pub kanidm_oauth2_client_id: Option<String>,
    pub kanidm_oauth2_client_secret: Option<String>,
    pub kanidm_authorize_url: Option<String>,
    pub kanidm_token_url: Option<String>,
    pub kanidm_userinfo_url: Option<String>,
    pub access_token_ttl: Duration,
    pub id_token_ttl: Duration,
}

impl Default for SsoBrokerConfig {
    fn default() -> Self {
        SsoBrokerConfig {
            enabled: false,
            issuer_url: None,
            signing_key: None,
            kanidm_oauth2_client_id: None,
            kanidm_oauth2_client_secret: None,
            kanidm_authorize_url: None,
            kanidm_token_url: None,
            kanidm_userinfo_url: None,
            access_token_ttl: Duration::from_secs(3600),
            id_token_ttl: Duration::from_secs(3600),
        }
    }
}

impl SsoBrokerConfig {
    pub fn from_env() -> Self {
        let issuer = std::env::var("HEARTH_SSO_BROKER_ISSUER_URL")
            .ok()
            .filter(|s| !s.is_empty());
        let key_pem = std::env::var("HEARTH_SSO_BROKER_PRIVATE_KEY_PEM").ok();
        let key_path = std::env::var("HEARTH_SSO_BROKER_PRIVATE_KEY_PATH")
            .ok()
            .map(PathBuf::from);
        let signing_key = key_pem
            .as_deref()
            .and_then(|p| SigningKey::load_from_pem(p).ok())
            .or_else(|| {
                key_path
                    .as_deref()
                    .and_then(|p| SigningKey::load_from_path(p).ok())
            })
            .map(Arc::new);
        let client_id = std::env::var("HEARTH_SSO_BROKER_KANIDM_CLIENT_ID").ok();
        let client_secret = std::env::var("HEARTH_SSO_BROKER_KANIDM_CLIENT_SECRET").ok();
        let authorize_url = std::env::var("HEARTH_SSO_BROKER_KANIDM_AUTHORIZE_URL").ok();
        let token_url = std::env::var("HEARTH_SSO_BROKER_KANIDM_TOKEN_URL").ok();
        let userinfo_url = std::env::var("HEARTH_SSO_BROKER_KANIDM_USERINFO_URL").ok();
        let enabled = issuer.is_some()
            && signing_key.is_some()
            && client_id.is_some()
            && client_secret.is_some()
            && authorize_url.is_some()
            && token_url.is_some();
        SsoBrokerConfig {
            enabled,
            issuer_url: issuer,
            signing_key,
            kanidm_oauth2_client_id: client_id,
            kanidm_oauth2_client_secret: client_secret,
            kanidm_authorize_url: authorize_url,
            kanidm_token_url: token_url,
            kanidm_userinfo_url: userinfo_url,
            access_token_ttl: Duration::from_secs(3600),
            id_token_ttl: Duration::from_secs(3600),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_when_no_env_set() {
        // Race-y if tests run in parallel and another sets these. Use serial_test
        // if your test runner needs it, or move to integration tests with a
        // dedicated env. For now this is safe since the names are unique to broker.
        let cfg = SsoBrokerConfig::from_env();
        assert!(!cfg.enabled || cfg.signing_key.is_none());
    }
}
