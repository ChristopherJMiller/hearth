//! Mint short-lived, pull-only Attic JWTs for enrollment cache access.
//!
//! Attic tokens are stateless HS256 JWTs with claims namespaced under
//! `https://jwt.attic.rs/v1`. We mint them directly using the shared
//! signing secret rather than going through an Attic admin API.
//!
//! `HEARTH_ATTIC_TOKEN_SECRET` must be the **same** base64-encoded HS256
//! secret configured in the Attic server's `[jwt.signing]` section
//! (`token-hs256-secret-base64`). In dev this comes from
//! `dev/attic/server.toml`; in production it must be injected from a
//! secrets manager and rotated in lockstep with the Attic config.

use base64::Engine;
use jsonwebtoken::{EncodingKey, Header};
use serde::Serialize;
use std::time::Duration;
use tracing::warn;

#[derive(Debug, Clone)]
pub struct CacheCredentials {
    pub cache_url: String,
    pub cache_token: String,
}

#[derive(Serialize)]
struct AtticClaims {
    sub: String,
    exp: u64,
    #[serde(rename = "https://jwt.attic.rs/v1")]
    attic: AtticV1,
}

#[derive(Serialize)]
struct AtticV1 {
    caches: std::collections::HashMap<String, CachePerms>,
}

#[derive(Serialize)]
struct CachePerms {
    r: u8,
}

/// Mint a pull-only Attic JWT for the given subject (e.g. `enrollment-<uuid>`).
///
/// The secret used here must match the Attic server's `token-hs256-secret-base64`
/// — it is a shared symmetric key, not an API credential.
///
/// Returns `Ok(None)` if `HEARTH_ATTIC_TOKEN_SECRET` is not set (graceful
/// degradation for dev environments without Attic).
pub fn mint_pull_token(
    subject: &str,
    validity: Duration,
) -> Result<Option<CacheCredentials>, jsonwebtoken::errors::Error> {
    let secret_b64 = match std::env::var("HEARTH_ATTIC_TOKEN_SECRET") {
        Ok(s) if !s.is_empty() => s,
        _ => {
            warn!("HEARTH_ATTIC_TOKEN_SECRET not set, skipping cache token");
            return Ok(None);
        }
    };

    let cache_url =
        std::env::var("HEARTH_ATTIC_SERVER").unwrap_or_else(|_| "http://localhost:8080".into());
    let cache_name = std::env::var("HEARTH_ATTIC_CACHE").unwrap_or_else(|_| "hearth".into());

    let secret_bytes = match base64::engine::general_purpose::STANDARD.decode(&secret_b64) {
        Ok(b) => b,
        Err(e) => {
            warn!("failed to decode HEARTH_ATTIC_TOKEN_SECRET: {e}");
            return Ok(None);
        }
    };

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // Attic serves each cache at /{cache_name}/ — the substituter URL must
    // include the cache name so Nix hits the right endpoint.
    let full_cache_url = format!("{}/{}", cache_url.trim_end_matches('/'), cache_name);

    let mut caches = std::collections::HashMap::new();
    caches.insert(cache_name, CachePerms { r: 1 });

    let claims = AtticClaims {
        sub: subject.to_string(),
        exp: now + validity.as_secs(),
        attic: AtticV1 { caches },
    };

    let key = EncodingKey::from_secret(&secret_bytes);
    let token = jsonwebtoken::encode(&Header::default(), &claims, &key)?;

    Ok(Some(CacheCredentials {
        cache_url: full_cache_url,
        cache_token: token,
    }))
}
