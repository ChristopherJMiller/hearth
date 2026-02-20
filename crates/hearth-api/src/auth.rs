//! Authentication and authorization middleware for the Hearth API.
//!
//! Supports two token types:
//! - **User tokens** (RS256): Kanidm OIDC JWTs validated against the JWKS endpoint
//! - **Machine tokens** (HS256): Minted at enrollment approval, used by agents
//!
//! If `KANIDM_OIDC_ISSUER` is not set, auth is disabled (dev mode).

use axum::extract::FromRequestParts;
use axum::http::StatusCode;
use axum::http::request::Parts;
use axum::response::{IntoResponse, Response};
use base64::Engine;
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
    /// Kanidm OIDC issuer URL (e.g. `https://localhost:8443/oauth2/openid/hearth-console`).
    /// If None, auth is disabled (dev mode).
    pub oidc_issuer: Option<String>,
    /// Expected audiences for user tokens (comma-separated in env var).
    pub oidc_audiences: Vec<String>,
    /// HS256 secret for minting/validating machine tokens.
    pub machine_token_secret: Option<Vec<u8>>,
    /// Cached JWKS keys for RS256 validation.
    pub jwks_cache: Arc<RwLock<JwksCache>>,
}

#[derive(Debug, Clone, Default)]
pub struct JwksCache {
    pub keys: Vec<JwkEntry>,
    pub fetched_at: Option<std::time::Instant>,
}

#[derive(Clone)]
pub struct JwkEntry {
    pub kid: Option<String>,
    pub decoding_key: DecodingKey,
}

impl std::fmt::Debug for JwkEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JwkEntry")
            .field("kid", &self.kid)
            .field("decoding_key", &"<redacted>")
            .finish()
    }
}

impl AuthConfig {
    pub fn from_env() -> Self {
        let oidc_issuer = std::env::var("KANIDM_OIDC_ISSUER")
            .ok()
            .filter(|s| !s.is_empty());
        let oidc_audiences: Vec<String> = std::env::var("KANIDM_OIDC_AUDIENCE")
            .ok()
            .filter(|s| !s.is_empty())
            .map(|s| s.split(',').map(|a| a.trim().to_string()).collect())
            .unwrap_or_default();
        let machine_token_secret = std::env::var("HEARTH_MACHINE_TOKEN_SECRET")
            .ok()
            .filter(|s| !s.is_empty())
            .and_then(|s| base64::engine::general_purpose::STANDARD.decode(&s).ok());

        if oidc_issuer.is_none() {
            warn!("KANIDM_OIDC_ISSUER not set — auth is DISABLED (dev mode)");
        }
        if machine_token_secret.is_none() {
            warn!("HEARTH_MACHINE_TOKEN_SECRET not set — machine token auth disabled");
        }

        Self {
            oidc_issuer,
            oidc_audiences,
            machine_token_secret,
            jwks_cache: Arc::new(RwLock::new(JwksCache::default())),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.oidc_issuer.is_some()
    }
}

// ---------------------------------------------------------------------------
// JWKS fetching
// ---------------------------------------------------------------------------

const JWKS_REFRESH_INTERVAL: Duration = Duration::from_secs(15 * 60);

#[derive(Deserialize)]
struct JwksResponse {
    keys: Vec<JwkKey>,
}

#[derive(Deserialize)]
struct JwkKey {
    kid: Option<String>,
    kty: String,
    #[serde(default)]
    n: Option<String>,
    #[serde(default)]
    e: Option<String>,
}

async fn refresh_jwks(config: &AuthConfig) -> Result<Vec<JwkEntry>, String> {
    let issuer = config
        .oidc_issuer
        .as_ref()
        .ok_or("no OIDC issuer configured")?;

    let jwks_url = format!("{issuer}/jwks");
    debug!(url = %jwks_url, "fetching JWKS");

    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true) // dev: self-signed Kanidm certs
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("HTTP client error: {e}"))?;

    let resp: JwksResponse = client
        .get(&jwks_url)
        .send()
        .await
        .map_err(|e| format!("JWKS fetch failed: {e}"))?
        .json()
        .await
        .map_err(|e| format!("JWKS parse failed: {e}"))?;

    let mut entries = Vec::new();
    for key in resp.keys {
        if key.kty == "RSA"
            && let (Some(n), Some(e)) = (&key.n, &key.e)
        {
            match DecodingKey::from_rsa_components(n, e) {
                Ok(dk) => entries.push(JwkEntry {
                    kid: key.kid.clone(),
                    decoding_key: dk,
                }),
                Err(err) => warn!(kid = ?key.kid, error = %err, "skipping invalid RSA JWK"),
            }
        }
    }

    info!(count = entries.len(), "refreshed JWKS keys");
    Ok(entries)
}

async fn get_jwks(config: &AuthConfig) -> Result<Vec<JwkEntry>, String> {
    {
        let cache = config.jwks_cache.read().await;
        if let Some(fetched_at) = cache.fetched_at
            && fetched_at.elapsed() < JWKS_REFRESH_INTERVAL
        {
            return Ok(cache.keys.clone());
        }
    }

    let keys = refresh_jwks(config).await?;
    {
        let mut cache = config.jwks_cache.write().await;
        cache.keys = keys.clone();
        cache.fetched_at = Some(std::time::Instant::now());
    }
    Ok(keys)
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
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    token.hash(&mut hasher);
    let hash = format!("{:016x}", hasher.finish());

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

fn validate_user_token(
    token: &str,
    keys: &[JwkEntry],
    audiences: &[String],
) -> Result<AuthClaims, String> {
    let header =
        jsonwebtoken::decode_header(token).map_err(|e| format!("invalid JWT header: {e}"))?;

    let key = if let Some(kid) = &header.kid {
        keys.iter()
            .find(|k| k.kid.as_deref() == Some(kid))
            .ok_or_else(|| format!("no JWK with kid={kid}"))?
    } else {
        keys.first().ok_or("no JWK keys available")?
    };

    let mut validation = Validation::new(Algorithm::RS256);
    if audiences.is_empty() {
        validation.validate_aud = false;
    } else {
        validation.set_audience(audiences);
    }
    validation.validate_exp = true;
    validation.leeway = 60;

    let data = decode::<OidcClaims>(token, &key.decoding_key, &validation)
        .map_err(|e| format!("JWT validation failed: {e}"))?;

    let mut groups = data.claims.groups;
    groups.extend(data.claims.scoped_groups);

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

        let keys = get_jwks(&state.auth_config)
            .await
            .map_err(|e| AuthError(StatusCode::INTERNAL_SERVER_ERROR, e))?;

        let claims = validate_user_token(token, &keys, &state.auth_config.oidc_audiences)
            .map_err(|e| AuthError(StatusCode::UNAUTHORIZED, e))?;

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

        let machine_id = validate_machine_token(token, secret)
            .map_err(|e| AuthError(StatusCode::UNAUTHORIZED, e))?;

        Ok(MachineIdentity(machine_id))
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
        if let Ok(keys) = get_jwks(&state.auth_config).await
            && let Ok(claims) =
                validate_user_token(token, &keys, &state.auth_config.oidc_audiences)
        {
            return Ok(OptionalIdentity(Some(AuthIdentity::User(claims))));
        }

        if let Some(secret) = &state.auth_config.machine_token_secret
            && let Ok(machine_id) = validate_machine_token(token, secret)
        {
            return Ok(OptionalIdentity(Some(AuthIdentity::Machine { machine_id })));
        }

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
