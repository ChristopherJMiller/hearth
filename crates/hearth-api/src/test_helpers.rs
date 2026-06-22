//! Test helpers shared across module-level tests. Compiled only under `#[cfg(test)]`.
#![cfg(test)]

use crate::{AppState, auth::AuthConfig};
use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Returns an AppState with a dangling unconnected pool and default config.
/// Use this for routing-shape tests that never hit the DB.
pub fn test_app_state() -> AppState {
    // SAFETY: PgPool::connect_lazy never panics for parseable URLs.
    let pool = PgPool::connect_lazy("postgres://test:test@127.0.0.1:1/test").expect("lazy connect");
    let auth_config = AuthConfig {
        oidc_issuers: Vec::new(),
        oidc_audiences: Vec::new(),
        machine_token_secret: None,
        jwks_cache: Arc::new(RwLock::new(crate::auth::JwksCache::default())),
    };
    AppState {
        pool,
        auth_config,
        headscale: None,
        cache_url: None,
        package_allowlist: None,
        services: Vec::new(),
        matrix_server_name: None,
        event_bus: crate::events::EventBus::new(),
        sso_broker: Arc::new(Default::default()),
        sso_authorize_stash: Arc::new(crate::sso_broker::sessions::AuthorizeStash::new(
            std::time::Duration::from_secs(600),
        )),
    }
}
