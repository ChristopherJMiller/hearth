//! Authentication and authorization middleware for the Hearth API.
//!
//! Supports two token types:
//! - **User tokens** (ES256/RS256): Kanidm OIDC JWTs validated against the JWKS endpoint
//! - **Machine tokens** (HS256): Minted at enrollment approval, used by agents
//!
//! If `KANIDM_OIDC_ISSUER` is not set, auth is disabled (dev mode).

use axum::extract::FromRequestParts;
use axum::http::StatusCode;
use axum::http::request::Parts;
use axum::response::{IntoResponse, Response};
use base64::Engine;
use jsonwebtoken::jwk::JwkSet;
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use uuid::Uuid;

use hearth_common::api_types::{AuthClaims, AuthIdentity};

use crate::AppState;
use crate::error::AppError;

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct AuthConfig {
    /// Kanidm OIDC issuer URLs (comma-separated in env var).
    /// Each OAuth2 client in Kanidm has its own signing key, so we need
    /// JWKS from every issuer that may produce tokens we accept.
    /// If empty, auth is disabled (dev mode).
    pub oidc_issuers: Vec<String>,
    /// Expected audiences for user tokens (comma-separated in env var).
    pub oidc_audiences: Vec<String>,
    /// HS256 secret for minting/validating machine tokens.
    pub machine_token_secret: Option<Vec<u8>>,
    /// Cached JWKS keyset for token validation.
    pub jwks_cache: Arc<RwLock<JwksCache>>,
}

#[derive(Debug, Clone)]
pub struct JwksCache {
    pub keyset: JwkSet,
    pub fetched_at: Option<std::time::Instant>,
}

impl Default for JwksCache {
    fn default() -> Self {
        Self {
            keyset: JwkSet { keys: Vec::new() },
            fetched_at: None,
        }
    }
}

impl AuthConfig {
    pub fn from_env() -> Self {
        let oidc_issuers: Vec<String> = std::env::var("KANIDM_OIDC_ISSUER")
            .ok()
            .filter(|s| !s.is_empty())
            .map(|s| {
                s.split(',')
                    .map(|a| a.trim().to_string())
                    .filter(|a| !a.is_empty())
                    .collect()
            })
            .unwrap_or_default();
        let oidc_audiences: Vec<String> = std::env::var("KANIDM_OIDC_AUDIENCE")
            .ok()
            .filter(|s| !s.is_empty())
            .map(|s| s.split(',').map(|a| a.trim().to_string()).collect())
            .unwrap_or_default();
        let machine_token_secret = std::env::var("HEARTH_MACHINE_TOKEN_SECRET")
            .ok()
            .filter(|s| !s.is_empty())
            .and_then(|s| {
                base64::engine::general_purpose::STANDARD
                    .decode(&s)
                    .or_else(|_| base64::engine::general_purpose::STANDARD_NO_PAD.decode(&s))
                    .ok()
            });

        if oidc_issuers.is_empty() {
            warn!("KANIDM_OIDC_ISSUER not set — auth is DISABLED (dev mode)");
        } else {
            info!(issuers = ?oidc_issuers, "OIDC issuers configured");
        }
        if machine_token_secret.is_none() {
            warn!("HEARTH_MACHINE_TOKEN_SECRET not set — machine token auth disabled");
        }

        Self {
            oidc_issuers,
            oidc_audiences,
            machine_token_secret,
            jwks_cache: Arc::new(RwLock::new(JwksCache::default())),
        }
    }

    pub fn is_enabled(&self) -> bool {
        !self.oidc_issuers.is_empty()
    }
}

// ---------------------------------------------------------------------------
// JWKS fetching
// ---------------------------------------------------------------------------

const JWKS_REFRESH_INTERVAL: Duration = Duration::from_secs(15 * 60);

/// OIDC discovery document (we only need `jwks_uri`).
#[derive(Deserialize)]
struct OidcDiscovery {
    jwks_uri: String,
}

/// Fetch JWKS keys from all configured OIDC issuers via their discovery documents.
async fn refresh_jwks(config: &AuthConfig) -> Result<JwkSet, String> {
    if config.oidc_issuers.is_empty() {
        return Err("no OIDC issuers configured".into());
    }

    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true) // dev: self-signed Kanidm certs
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("HTTP client error: {e}"))?;

    let mut all_keys = Vec::new();

    for issuer in &config.oidc_issuers {
        // Fetch the OIDC discovery document to get the correct jwks_uri
        let discovery_url = format!("{}/.well-known/openid-configuration", issuer);
        debug!(url = %discovery_url, "fetching OIDC discovery");

        let discovery: OidcDiscovery = match client.get(&discovery_url).send().await {
            Ok(r) => match r.json().await {
                Ok(d) => d,
                Err(e) => {
                    warn!(issuer = %issuer, error = %e, "OIDC discovery parse failed, skipping");
                    continue;
                }
            },
            Err(e) => {
                warn!(issuer = %issuer, error = %e, "OIDC discovery fetch failed, skipping");
                continue;
            }
        };

        // Kanidm's jwks_uri uses the configured origin which may differ from
        // the URL we used. Rewrite it to use the same base we can actually reach.
        let jwks_url = if let Some(pos) = issuer.find("/oauth2/") {
            let reachable_base = &issuer[..pos];
            if let Some(path_pos) = discovery.jwks_uri.find("/oauth2/") {
                format!("{reachable_base}{}", &discovery.jwks_uri[path_pos..])
            } else {
                discovery.jwks_uri.clone()
            }
        } else {
            discovery.jwks_uri.clone()
        };

        debug!(url = %jwks_url, "fetching JWKS");

        let keyset: JwkSet = match client.get(&jwks_url).send().await {
            Ok(r) => match r.json().await {
                Ok(j) => j,
                Err(e) => {
                    warn!(issuer = %issuer, error = %e, "JWKS parse failed, skipping");
                    continue;
                }
            },
            Err(e) => {
                warn!(issuer = %issuer, error = %e, "JWKS fetch failed, skipping");
                continue;
            }
        };

        debug!(
            issuer = %issuer,
            key_count = keyset.keys.len(),
            "fetched JWKS keys from issuer"
        );
        all_keys.extend(keyset.keys);
    }

    info!(
        total_keys = all_keys.len(),
        issuers = config.oidc_issuers.len(),
        "refreshed JWKS keys"
    );
    Ok(JwkSet { keys: all_keys })
}

async fn get_jwks(config: &AuthConfig) -> Result<JwkSet, String> {
    {
        let cache = config.jwks_cache.read().await;
        if let Some(fetched_at) = cache.fetched_at
            && fetched_at.elapsed() < JWKS_REFRESH_INTERVAL
        {
            return Ok(cache.keyset.clone());
        }
    }

    let keyset = refresh_jwks(config).await?;
    {
        let mut cache = config.jwks_cache.write().await;
        cache.keyset = keyset.clone();
        cache.fetched_at = Some(std::time::Instant::now());
    }
    Ok(keyset)
}

// ---------------------------------------------------------------------------
// Machine token minting
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
struct MachineTokenClaims {
    sub: String,
    machine_id: String,
    exp: u64,
    iat: u64,
}

/// Mint an HS256 machine auth token for the given machine UUID.
/// Returns (raw_token, sha256_hash_of_token) so the hash can be stored in the DB.
pub fn mint_machine_token(
    machine_id: Uuid,
    config: &AuthConfig,
) -> Result<(String, String), AppError> {
    let secret = config
        .machine_token_secret
        .as_ref()
        .ok_or_else(|| AppError::Internal("HEARTH_MACHINE_TOKEN_SECRET not configured".into()))?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let claims = MachineTokenClaims {
        sub: format!("machine:{machine_id}"),
        machine_id: machine_id.to_string(),
        exp: now + 90 * 24 * 3600, // 90 days
        iat: now,
    };

    let key = jsonwebtoken::EncodingKey::from_secret(secret);
    let token = encode(&jsonwebtoken::Header::default(), &claims, &key)
        .map_err(|e| AppError::Internal(format!("failed to mint machine token: {e}")))?;

    // Store a hash in the DB for revocation checks
    use sha2::{Digest, Sha256};
    let hash = format!("{:x}", Sha256::digest(token.as_bytes()));

    Ok((token, hash))
}

// ---------------------------------------------------------------------------
// Token validation
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct OidcClaims {
    sub: String,
    #[serde(default)]
    preferred_username: Option<String>,
    #[serde(default)]
    email: Option<String>,
    #[serde(default)]
    groups: Vec<String>,
    #[serde(default, rename = "scoped_groups")]
    scoped_groups: Vec<String>,
}

/// Normalize Kanidm's OIDC `groups` claim into clean short names.
///
/// Kanidm emits each group membership **twice** — once as a UUID and once as
/// an SPN (`name@domain`). We drop the UUID form (unreadable, redundant) and
/// strip the `@domain` suffix so downstream code can match on plain short
/// names like `hearth-admins`. Duplicates are removed while preserving order.
///
/// Note: this assumes a single Kanidm realm per deployment. Federating
/// multiple realms with overlapping group names would need to revisit this.
fn normalize_groups(raw: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    raw.into_iter()
        .filter(|g| Uuid::parse_str(g).is_err())
        .map(|g| {
            g.split_once('@')
                .map(|(name, _)| name.to_string())
                .unwrap_or(g)
        })
        .filter(|g| seen.insert(g.clone()))
        .collect()
}

fn validate_user_token(
    token: &str,
    keyset: &JwkSet,
    audiences: &[String],
) -> Result<AuthClaims, String> {
    let header =
        jsonwebtoken::decode_header(token).map_err(|e| format!("invalid JWT header: {e}"))?;

    let jwk = if let Some(kid) = &header.kid {
        keyset.find(kid).ok_or_else(|| {
            let available: Vec<_> = keyset
                .keys
                .iter()
                .filter_map(|k| k.common.key_id.as_deref())
                .collect();
            format!("no JWK with kid={kid} (available: {available:?})")
        })?
    } else {
        keyset.keys.first().ok_or("no JWK keys available")?
    };

    let decoding_key = DecodingKey::from_jwk(jwk)
        .map_err(|e| format!("failed to build decoding key from JWK: {e}"))?;

    // Use the algorithm declared in the JWK, falling back to the JWT header's alg
    let algorithm = jwk
        .common
        .key_algorithm
        .and_then(|a| a.to_string().parse::<Algorithm>().ok())
        .or(header.alg.into())
        .unwrap_or(Algorithm::ES256);

    let mut validation = Validation::new(algorithm);
    if audiences.is_empty() {
        validation.validate_aud = false;
    } else {
        validation.set_audience(audiences);
    }
    validation.validate_exp = true;
    validation.leeway = 60;

    let data = decode::<OidcClaims>(token, &decoding_key, &validation)
        .map_err(|e| format!("JWT validation failed: {e}"))?;

    let mut groups = data.claims.groups;
    groups.extend(data.claims.scoped_groups);
    let groups = normalize_groups(groups);

    info!(
        sub = %data.claims.sub,
        preferred_username = ?data.claims.preferred_username,
        groups = ?groups,
        "validated user JWT claims"
    );

    Ok(AuthClaims {
        sub: data.claims.sub,
        preferred_username: data.claims.preferred_username,
        email: data.claims.email,
        groups,
    })
}

fn validate_machine_token(token: &str, secret: &[u8]) -> Result<Uuid, String> {
    let key = DecodingKey::from_secret(secret);
    let mut validation = Validation::new(Algorithm::HS256);
    validation.validate_aud = false;
    validation.validate_exp = true;
    validation.leeway = 60;

    let data = decode::<MachineTokenClaims>(token, &key, &validation)
        .map_err(|e| format!("machine token validation failed: {e}"))?;

    data.claims
        .machine_id
        .parse::<Uuid>()
        .map_err(|e| format!("invalid machine_id in token: {e}"))
}

// ---------------------------------------------------------------------------
// Axum extractors
// ---------------------------------------------------------------------------

/// Lightweight rejection type for auth extractors (implements IntoResponse directly).
#[derive(Debug)]
pub struct AuthError(pub StatusCode, pub String);

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        (self.0, self.1).into_response()
    }
}

fn extract_bearer(parts: &Parts) -> Option<&str> {
    parts
        .headers
        .get("authorization")?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
}

/// Extractor that requires a valid user (OIDC) token.
pub struct UserIdentity(pub AuthClaims);

impl FromRequestParts<AppState> for UserIdentity {
    type Rejection = AuthError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        if !state.auth_config.is_enabled() {
            return Ok(UserIdentity(AuthClaims {
                sub: "dev-admin".into(),
                preferred_username: Some("dev-admin".into()),
                email: None,
                groups: vec!["hearth-admins".into(), "hearth-users".into()],
            }));
        }

        let token = extract_bearer(parts)
            .ok_or_else(|| AuthError(StatusCode::UNAUTHORIZED, "missing Bearer token".into()))?;

        let keyset = get_jwks(&state.auth_config).await.map_err(|e| {
            warn!(error = %e, "JWKS fetch failed during user auth");
            AuthError(StatusCode::INTERNAL_SERVER_ERROR, e)
        })?;

        let claims = validate_user_token(token, &keyset, &state.auth_config.oidc_audiences)
            .map_err(|e| {
                warn!(error = %e, "user token validation failed");
                AuthError(StatusCode::UNAUTHORIZED, e)
            })?;

        Ok(UserIdentity(claims))
    }
}

/// Extractor that requires a valid machine token.
pub struct MachineIdentity(pub Uuid);

impl FromRequestParts<AppState> for MachineIdentity {
    type Rejection = AuthError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        if !state.auth_config.is_enabled() {
            return Ok(MachineIdentity(Uuid::nil()));
        }

        let secret = state
            .auth_config
            .machine_token_secret
            .as_ref()
            .ok_or_else(|| {
                AuthError(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "machine token auth not configured".into(),
                )
            })?;

        let token = extract_bearer(parts)
            .ok_or_else(|| AuthError(StatusCode::UNAUTHORIZED, "missing Bearer token".into()))?;

        match validate_machine_token(token, secret) {
            Ok(machine_id) => Ok(MachineIdentity(machine_id)),
            Err(e) => {
                // If the token is a valid user JWT, return 403 (wrong token type)
                // rather than 401 (unauthenticated).
                if let Ok(keyset) = get_jwks(&state.auth_config).await
                    && validate_user_token(token, &keyset, &state.auth_config.oidc_audiences)
                        .is_ok()
                {
                    return Err(AuthError(
                        StatusCode::FORBIDDEN,
                        "this endpoint requires machine identity".into(),
                    ));
                }
                Err(AuthError(StatusCode::UNAUTHORIZED, e))
            }
        }
    }
}

/// Extractor that accepts either a user or machine token, or no token at all.
pub struct OptionalIdentity(pub Option<AuthIdentity>);

impl FromRequestParts<AppState> for OptionalIdentity {
    type Rejection = AuthError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        if !state.auth_config.is_enabled() {
            return Ok(OptionalIdentity(Some(AuthIdentity::User(AuthClaims {
                sub: "dev-admin".into(),
                preferred_username: Some("dev-admin".into()),
                email: None,
                groups: vec!["hearth-admins".into(), "hearth-users".into()],
            }))));
        }

        let token = match extract_bearer(parts) {
            Some(t) => t,
            None => return Ok(OptionalIdentity(None)),
        };

        // Try user token first, then machine token
        let user_err = match get_jwks(&state.auth_config).await {
            Ok(keyset) => {
                match validate_user_token(token, &keyset, &state.auth_config.oidc_audiences) {
                    Ok(claims) => {
                        return Ok(OptionalIdentity(Some(AuthIdentity::User(claims))));
                    }
                    Err(e) => Some(e),
                }
            }
            Err(e) => Some(e),
        };

        let machine_err = match &state.auth_config.machine_token_secret {
            Some(secret) => match validate_machine_token(token, secret) {
                Ok(machine_id) => {
                    return Ok(OptionalIdentity(Some(AuthIdentity::Machine { machine_id })));
                }
                Err(e) => Some(e),
            },
            None => Some("machine token auth not configured".into()),
        };

        warn!(
            user_token_error = ?user_err,
            machine_token_error = ?machine_err,
            "token validation failed for both user and machine token types"
        );

        Err(AuthError(StatusCode::UNAUTHORIZED, "invalid token".into()))
    }
}

/// Extractor that requires a user in the hearth-operators or hearth-admins group.
pub struct OperatorIdentity(pub AuthClaims);

impl FromRequestParts<AppState> for OperatorIdentity {
    type Rejection = AuthError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let UserIdentity(claims) = UserIdentity::from_request_parts(parts, state).await?;

        if claims
            .groups
            .iter()
            .any(|g| g == "hearth-operators" || g == "hearth-admins")
        {
            Ok(OperatorIdentity(claims))
        } else {
            Err(AuthError(
                StatusCode::FORBIDDEN,
                "requires hearth-operators or hearth-admins group membership".into(),
            ))
        }
    }
}

/// Extractor that requires a user in the hearth-admins group.
pub struct AdminIdentity(pub AuthClaims);

impl FromRequestParts<AppState> for AdminIdentity {
    type Rejection = AuthError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let UserIdentity(claims) = UserIdentity::from_request_parts(parts, state).await?;

        if claims.groups.iter().any(|g| g == "hearth-admins") {
            Ok(AdminIdentity(claims))
        } else {
            Err(AuthError(
                StatusCode::FORBIDDEN,
                "requires hearth-admins group membership".into(),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_groups_strips_uuid_and_domain() {
        let raw = vec![
            "00000000-0000-0000-0000-000000000035".to_string(),
            "idm_all_persons@kanidm.hearth.local".to_string(),
            "47cab1f8-fe2e-47f5-b993-f41bb31dfaab".to_string(),
            "hearth-users@kanidm.hearth.local".to_string(),
            "dd76cf22-5d62-46fe-b8d1-3a1ca9048c82".to_string(),
            "hearth-admins@kanidm.hearth.local".to_string(),
        ];
        assert_eq!(
            normalize_groups(raw),
            vec!["idm_all_persons", "hearth-users", "hearth-admins"],
        );
    }

    #[test]
    fn normalize_groups_all_uuid_yields_empty() {
        let raw = vec![
            "00000000-0000-0000-0000-000000000035".to_string(),
            "47cab1f8-fe2e-47f5-b993-f41bb31dfaab".to_string(),
        ];
        assert!(normalize_groups(raw).is_empty());
    }

    #[test]
    fn normalize_groups_short_names_pass_through() {
        // Test fixtures and dev-mode bypass use short names directly — must
        // remain unchanged so existing tests and dev flows keep working.
        let raw = vec!["hearth-admins".to_string(), "hearth-users".to_string()];
        assert_eq!(normalize_groups(raw), vec!["hearth-admins", "hearth-users"]);
    }

    #[test]
    fn normalize_groups_dedupes_across_forms() {
        let raw = vec![
            "hearth-admins".to_string(),
            "hearth-admins@kanidm.hearth.local".to_string(),
            "47cab1f8-fe2e-47f5-b993-f41bb31dfaab".to_string(),
            "hearth-users@kanidm.hearth.local".to_string(),
            "hearth-users".to_string(),
        ];
        assert_eq!(normalize_groups(raw), vec!["hearth-admins", "hearth-users"]);
    }
}
