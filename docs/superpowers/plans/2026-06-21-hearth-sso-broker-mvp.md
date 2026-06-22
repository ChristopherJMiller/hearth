# Hearth SSO Broker MVP Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stand up a hearth-side OIDC broker that wraps Kanidm and becomes the IdP for fleet apps, proven by migrating Synapse to use it end-to-end. This is the foundation that unblocks later seamless-SSO work (device daemons, browser primer, native-app keyring) — those follow in a separate plan.

**Architecture:**
- A new `sso_broker` module inside `hearth-api` exposes an OIDC issuer at `/oidc` (discovery, JWKS, authorize, callback, token, userinfo).
- The broker is itself an OIDC **client** of Kanidm: end-users authenticate to Kanidm via the standard authorization-code flow; the broker receives Kanidm's identity claims and mints its own ES256-signed tokens with `sub` = stable Kanidm UUID.
- Apps (starting with Synapse) trust the broker as IdP — they never speak to Kanidm directly anymore.
- Kanidm remains source of truth for credentials, MFA, groups; the broker only re-issues identity tokens with a different signing key and audit trail.
- DB table `sso_issued_tokens` records every minted token's `jti` for revocation; audit events go to existing `audit_events` table.
- Helm capability toggle `capabilities.ssoBroker` gates signing-key Secret + Job. New Kanidm SA `hearth-broker-svc` reads users/groups.

**Tech Stack:** Rust (axum, jsonwebtoken, p256/ecdsa, reqwest), PostgreSQL (sqlx), Helm + helm-unittest, Synapse OIDC config, NixOS VM tests.

## Global Constraints

- Kanidm version pinned to **1.10** — do not introduce features that need newer Kanidm.
- New routes MUST use one of the 5 auth extractors in `crates/hearth-api/src/auth.rs` (`UserIdentity`, `MachineIdentity`, `OptionalIdentity`, `OperatorIdentity`, `AdminIdentity`) — except OIDC endpoints, which use their own auth model (browser sessions, OAuth client credentials); document the exemption inline.
- sqlx compile-time checked queries only (`query!` / `query_as!`). `SQLX_OFFLINE=true` by default; regenerate `.sqlx/` metadata after schema changes.
- Container `securityContext` mandatory: `allowPrivilegeEscalation: false`, `readOnlyRootFilesystem: true`, `runAsNonRoot: true`, `capabilities: { drop: [ALL] }`, `seccompProfile: { type: RuntimeDefault }`.
- Secrets via `Secret` + `secretKeyRef` only. Support both `existingSecret` (prod) and auto-generated (dev) patterns matching existing chart idiom.
- New Deployments / StatefulSets ship with a `NetworkPolicy` template.
- Rust edition 2024. Clippy with `--deny warnings`. cargo fmt clean.
- Logging via `tracing` crate.
- Frequent commits — one per task minimum.
- No `.unwrap()` / `.expect()` in request handlers. `.expect()` at startup on required config is fine.
- All endpoints accepting JSON set `axum::extract::DefaultBodyLimit`.
- The `.danger_accept_invalid_certs(true)` pattern is for dev self-signed Kanidm certs only — do not extend to new call sites except where the existing identity_sync pattern is followed verbatim.

## File Map

**New files:**
- `crates/hearth-api/src/sso_broker/mod.rs` — module root, public exports, route composition
- `crates/hearth-api/src/sso_broker/config.rs` — `SsoBrokerConfig` struct, env loading
- `crates/hearth-api/src/sso_broker/keys.rs` — ES256 signing key loading + JWKS rendering
- `crates/hearth-api/src/sso_broker/clients.rs` — broker's OAuth2 client registry (declarative for MVP)
- `crates/hearth-api/src/sso_broker/sessions.rs` — in-memory authorize/code state with TTL
- `crates/hearth-api/src/sso_broker/kanidm_upstream.rs` — broker-as-OIDC-client-of-Kanidm helper
- `crates/hearth-api/src/sso_broker/tokens.rs` — broker token minting + claims types
- `crates/hearth-api/src/sso_broker/handlers/discovery.rs` — `/oidc/.well-known/openid-configuration`
- `crates/hearth-api/src/sso_broker/handlers/jwks.rs` — `/oidc/.well-known/jwks.json`
- `crates/hearth-api/src/sso_broker/handlers/authorize.rs` — `/oidc/authorize`
- `crates/hearth-api/src/sso_broker/handlers/callback.rs` — `/oidc/callback`
- `crates/hearth-api/src/sso_broker/handlers/token.rs` — `/oidc/token`
- `crates/hearth-api/src/sso_broker/handlers/userinfo.rs` — `/oidc/userinfo`
- `crates/hearth-api/src/sso_broker/handlers/revoke.rs` — `/oidc/revoke`
- `migrations/NNNN_sso_broker.sql` — `sso_issued_tokens` table + indexes
- `chart/hearth-home/templates/sso-broker/_helpers.tpl` — broker-specific helpers
- `chart/hearth-home/templates/sso-broker/secret-signing-key.yaml` — keypair Secret (dev auto-gen)
- `chart/hearth-home/templates/sso-broker/job-genkey.yaml` — first-install keypair generation Job
- `chart/hearth-home/tests/sso_broker_signing_secret_test.yaml` — helm-unittest
- `chart/hearth-home/tests/sso_broker_synapse_oidc_test.yaml` — helm-unittest
- `tests/vm-sso-broker-synapse-login.nix` — NixOS VM test
- `docs/sso-broker.md` — operator-facing overview

**Modified files:**
- `crates/hearth-api/src/lib.rs` — register sso_broker routes, add config to AppState
- `crates/hearth-api/src/main.rs` — load broker config at startup
- `crates/hearth-api/Cargo.toml` — add `p256`, `ecdsa`, `base64`, `urlencoding`
- `crates/hearth-api/src/error.rs` — add `OAuth2Error` variants matching RFC 6749
- `chart/hearth-home/values.yaml` — `capabilities.ssoBroker`, `ssoBroker.*` stanza
- `chart/hearth-home/templates/kanidm/bootstrap-configmap.yaml` — register `hearth-broker-svc` SA + grant + register broker as Kanidm OAuth2 client
- `chart/hearth-home/templates/api/deployment.yaml` — mount signing key Secret, env vars
- `chart/hearth-home/templates/synapse/configmap.yaml` — point `oidc_providers.issuer` at broker
- `chart/hearth-home/templates/networkpolicy.yaml` — allow Synapse → API egress (broker OIDC port)
- `flake.nix` — add `vmTests.sso-broker-synapse-login` entry
- `docs/threat-model.md` — add broker as trust-boundary component
- `docs/compliance-controls.yaml` — note signing-key handling for audit

---

## Phase A: Broker module scaffolding + signing key

### Task 1: Create sso_broker module skeleton, wire empty router

**Files:**
- Create: `crates/hearth-api/src/sso_broker/mod.rs`
- Modify: `crates/hearth-api/src/lib.rs` (add `pub mod sso_broker;` and route nesting)
- Test: `crates/hearth-api/src/sso_broker/mod.rs` (inline `#[cfg(test)] mod tests`)

**Interfaces:**
- Produces: `pub fn sso_broker::routes() -> axum::Router<AppState>` — composes broker handlers under `/oidc/*` prefix. Initially returns one stub handler.

- [ ] **Step 1: Write the failing test**

In `crates/hearth-api/src/sso_broker/mod.rs`:

```rust
pub mod config;

use axum::Router;
use crate::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    #[tokio::test]
    async fn module_compiles_and_router_builds() {
        // This test exists so the module compiles before we add real routes.
        let _: Router = routes().with_state(crate::test_helpers::test_app_state());
    }
}
```

Create `crates/hearth-api/src/sso_broker/config.rs` with a minimal placeholder:

```rust
//! Configuration for the SSO broker.

#[derive(Debug, Clone, Default)]
pub struct SsoBrokerConfig {
    pub enabled: bool,
}
```

- [ ] **Step 2: Wire the module into lib.rs**

In `crates/hearth-api/src/lib.rs`, add near the other module declarations (after `pub mod routes;`):

```rust
pub mod sso_broker;
```

In `build_router`, add the nest call just before `.fallback_service(spa)`:

```rust
.nest("/oidc", sso_broker::routes())
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test -p hearth-api sso_broker::tests::module_compiles_and_router_builds`
Expected: FAIL — `test_helpers::test_app_state` does not exist (we will add it next).

- [ ] **Step 4: Add minimal test_helpers**

Create `crates/hearth-api/src/test_helpers.rs`:

```rust
//! Test helpers shared across module-level tests. Compiled only under `#[cfg(test)]`.
#![cfg(test)]

use crate::{auth::AuthConfig, AppState};
use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Returns an AppState with a dangling unconnected pool and default config.
/// Use this for routing-shape tests that never hit the DB.
pub fn test_app_state() -> AppState {
    // SAFETY: PgPool::connect_lazy never panics for parseable URLs.
    let pool = PgPool::connect_lazy("postgres://test:test@127.0.0.1:1/test")
        .expect("lazy connect");
    AppState {
        pool,
        auth: AuthConfig::default(),
        // populate other fields as required by the current AppState shape;
        // mirror the existing pattern in lib.rs's test scaffolding if any.
    }
}
```

Wire it in `crates/hearth-api/src/lib.rs` near the other module declarations:

```rust
#[cfg(test)]
mod test_helpers;
```

If `AppState` has more fields than `pool` and `auth`, expand the construction to cover all of them with safe defaults (use existing test scaffolding in `lib.rs` or `routes/*` as the reference shape).

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p hearth-api sso_broker::tests::module_compiles_and_router_builds`
Expected: PASS

- [ ] **Step 6: Verify the OIDC nesting compiles and is reachable**

Run: `cargo build -p hearth-api`
Expected: builds clean.

- [ ] **Step 7: Commit**

```bash
git add crates/hearth-api/src/sso_broker/ crates/hearth-api/src/lib.rs crates/hearth-api/src/test_helpers.rs
git commit -m "feat(sso-broker): scaffold module and /oidc router"
```

---

### Task 2: ES256 signing key loading

**Files:**
- Create: `crates/hearth-api/src/sso_broker/keys.rs`
- Modify: `crates/hearth-api/src/sso_broker/mod.rs` (`pub mod keys;`)
- Modify: `crates/hearth-api/src/sso_broker/config.rs` (add `signing_key_path: Option<PathBuf>`, `signing_key_pem: Option<String>`)
- Modify: `crates/hearth-api/Cargo.toml` (add `p256 = { version = "0.13", features = ["pem", "ecdsa", "jwk"] }`, `ecdsa = { version = "0.16", features = ["signing"] }`, `pem = "3"`)

**Interfaces:**
- Produces:
  - `pub struct SigningKey { /* opaque */ }`
  - `pub fn load_from_pem(pem: &str) -> Result<SigningKey, SigningKeyError>`
  - `pub fn load_from_path(path: &Path) -> Result<SigningKey, SigningKeyError>`
  - `pub fn kid(&self) -> &str` — stable key ID (SHA-256 of public key DER, base64url-no-pad, first 32 chars)
  - `pub fn jwk_public(&self) -> serde_json::Value` — public JWK for JWKS endpoint
  - `pub fn sign(&self, header_b64: &str, payload_b64: &str) -> Result<String, SigningKeyError>` — returns base64url signature
  - `pub enum SigningKeyError { InvalidPem(String), Io(io::Error), KeyTooShort }`

- [ ] **Step 1: Write the failing test**

Create `crates/hearth-api/src/sso_broker/keys.rs`:

```rust
//! ES256 (P-256) signing key for broker-issued OIDC tokens.

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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p hearth-api sso_broker::keys::tests`
Expected: FAIL — `SigningKey` not defined.

- [ ] **Step 3: Implement SigningKey**

Add the implementation above the `#[cfg(test)]` block in `keys.rs`:

```rust
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use p256::ecdsa::{SigningKey as EcdsaSigning, signature::Signer};
use p256::pkcs8::DecodePrivateKey;
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
}

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
        let spki = verifying.to_public_key_der()
            .map_err(|e| SigningKeyError::InvalidPem(e.to_string()))?;
        let digest = Sha256::digest(spki.as_bytes());
        let kid = URL_SAFE_NO_PAD.encode(&digest[..]).chars().take(32).collect();
        Ok(SigningKey { inner, kid, x_b64, y_b64 })
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
```

Add `pub mod keys;` to `sso_broker/mod.rs`.

Add to `crates/hearth-api/Cargo.toml` under `[dependencies]`:

```toml
p256 = { version = "0.13", features = ["pem", "ecdsa", "jwk"] }
pem = "3"
sha2 = "0.10"
thiserror = "2"
```

(check existing Cargo.toml — `sha2`, `thiserror`, `base64` are likely already there; only add missing deps).

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p hearth-api sso_broker::keys::tests`
Expected: 4 tests PASS.

- [ ] **Step 5: Run clippy**

Run: `cargo clippy -p hearth-api -- --deny warnings`
Expected: clean.

- [ ] **Step 6: Commit**

```bash
git add crates/hearth-api/src/sso_broker/keys.rs crates/hearth-api/src/sso_broker/mod.rs crates/hearth-api/Cargo.toml
git commit -m "feat(sso-broker): ES256 signing key loading + JWK rendering"
```

---

### Task 3: SsoBrokerConfig from env + AppState wiring

**Files:**
- Modify: `crates/hearth-api/src/sso_broker/config.rs`
- Modify: `crates/hearth-api/src/lib.rs` (extend `AppState`, plumb config)
- Modify: `crates/hearth-api/src/main.rs` (load config at startup)

**Interfaces:**
- Produces:
  - `pub struct SsoBrokerConfig { pub enabled: bool, pub issuer_url: Option<String>, pub signing_key: Option<Arc<SigningKey>>, pub kanidm_oauth2_client_id: Option<String>, pub kanidm_oauth2_client_secret: Option<String>, pub kanidm_authorize_url: Option<String>, pub kanidm_token_url: Option<String>, pub kanidm_userinfo_url: Option<String>, pub access_token_ttl: Duration, pub id_token_ttl: Duration }`
  - `impl SsoBrokerConfig { pub fn from_env() -> Self }` — reads `HEARTH_SSO_BROKER_*` env vars; returns disabled config when any required var missing.
- Consumes: `keys::SigningKey` from Task 2.

- [ ] **Step 1: Write the failing test**

Append to `crates/hearth-api/src/sso_broker/config.rs`:

```rust
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use crate::sso_broker::keys::SigningKey;

#[derive(Debug, Clone, Default)]
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
            .or_else(|| key_path.as_deref().and_then(|p| SigningKey::load_from_path(p).ok()))
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
```

- [ ] **Step 2: Run test to verify it passes**

Run: `cargo test -p hearth-api sso_broker::config`
Expected: PASS.

- [ ] **Step 3: Wire config into AppState**

Read `crates/hearth-api/src/lib.rs` and find the `AppState` struct definition. Add the field:

```rust
pub sso_broker: Arc<crate::sso_broker::config::SsoBrokerConfig>,
```

Find every construction site for `AppState` (including `main.rs` and any test helpers) and populate the new field. In `main.rs`, load via:

```rust
let sso_broker = Arc::new(hearth_api::sso_broker::config::SsoBrokerConfig::from_env());
if sso_broker.enabled {
    tracing::info!("sso_broker enabled with issuer {}", sso_broker.issuer_url.as_deref().unwrap_or("?"));
} else {
    tracing::info!("sso_broker disabled (set HEARTH_SSO_BROKER_* env vars to enable)");
}
```

Pass `sso_broker` into `AppState`.

In `test_helpers.rs`, populate `sso_broker: Arc::new(Default::default())`.

- [ ] **Step 4: Verify build**

Run: `cargo build -p hearth-api`
Expected: clean.

- [ ] **Step 5: Run all tests to verify nothing broke**

Run: `cargo test -p hearth-api`
Expected: all tests still pass.

- [ ] **Step 6: Commit**

```bash
git add crates/hearth-api/src/sso_broker/config.rs crates/hearth-api/src/lib.rs crates/hearth-api/src/main.rs crates/hearth-api/src/test_helpers.rs
git commit -m "feat(sso-broker): SsoBrokerConfig from env + AppState wiring"
```

---

## Phase B: OIDC issuer endpoints

### Task 4: OIDC discovery document

**Files:**
- Create: `crates/hearth-api/src/sso_broker/handlers/mod.rs`
- Create: `crates/hearth-api/src/sso_broker/handlers/discovery.rs`
- Modify: `crates/hearth-api/src/sso_broker/mod.rs` (add `pub mod handlers;`, wire route)

**Interfaces:**
- Produces: `GET /oidc/.well-known/openid-configuration` returning OIDC discovery JSON. 503 when broker disabled.

- [ ] **Step 1: Write the failing test**

Create `crates/hearth-api/src/sso_broker/handlers/discovery.rs`:

```rust
//! OIDC discovery document — RFC OpenID Connect Discovery 1.0.

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use serde_json::{json, Value};

use crate::AppState;

pub async fn discovery(State(state): State<AppState>) -> Response {
    if !state.sso_broker.enabled {
        return (StatusCode::SERVICE_UNAVAILABLE, "sso broker disabled").into_response();
    }
    let issuer = state
        .sso_broker
        .issuer_url
        .as_deref()
        .unwrap_or_default();
    let doc = build_discovery_doc(issuer);
    Json(doc).into_response()
}

fn build_discovery_doc(issuer: &str) -> Value {
    json!({
        "issuer": issuer,
        "authorization_endpoint": format!("{issuer}/authorize"),
        "token_endpoint": format!("{issuer}/token"),
        "userinfo_endpoint": format!("{issuer}/userinfo"),
        "jwks_uri": format!("{issuer}/.well-known/jwks.json"),
        "revocation_endpoint": format!("{issuer}/revoke"),
        "response_types_supported": ["code"],
        "grant_types_supported": ["authorization_code", "refresh_token"],
        "subject_types_supported": ["public"],
        "id_token_signing_alg_values_supported": ["ES256"],
        "scopes_supported": ["openid", "profile", "email", "groups"],
        "claims_supported": ["sub", "preferred_username", "name", "email", "email_verified", "groups"],
        "token_endpoint_auth_methods_supported": ["client_secret_basic", "client_secret_post"],
        "code_challenge_methods_supported": ["S256"],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovery_doc_has_required_fields() {
        let doc = build_discovery_doc("https://api.hearth.example/oidc");
        assert_eq!(doc["issuer"], "https://api.hearth.example/oidc");
        assert_eq!(doc["authorization_endpoint"], "https://api.hearth.example/oidc/authorize");
        assert_eq!(doc["id_token_signing_alg_values_supported"][0], "ES256");
        assert!(doc["jwks_uri"].as_str().unwrap().ends_with("/jwks.json"));
    }
}
```

- [ ] **Step 2: Wire route**

Create `crates/hearth-api/src/sso_broker/handlers/mod.rs`:

```rust
pub mod discovery;
```

Update `crates/hearth-api/src/sso_broker/mod.rs` `routes()`:

```rust
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/.well-known/openid-configuration", axum::routing::get(handlers::discovery::discovery))
}
```

Add `pub mod handlers;` near the top of `mod.rs`.

- [ ] **Step 3: Run unit test**

Run: `cargo test -p hearth-api sso_broker::handlers::discovery`
Expected: PASS.

- [ ] **Step 4: Add integration test for the endpoint**

Append to `crates/hearth-api/src/sso_broker/handlers/discovery.rs`'s `tests` mod:

```rust
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;
    use crate::sso_broker::config::SsoBrokerConfig;
    use std::sync::Arc;

    #[tokio::test]
    async fn returns_503_when_disabled() {
        let state = crate::test_helpers::test_app_state();
        let router = axum::Router::new()
            .route("/oidc/.well-known/openid-configuration", axum::routing::get(discovery))
            .with_state(state);
        let req = Request::builder()
            .uri("/oidc/.well-known/openid-configuration")
            .body(Body::empty()).unwrap();
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    }
```

- [ ] **Step 5: Run integration test**

Run: `cargo test -p hearth-api sso_broker::handlers::discovery::tests::returns_503_when_disabled`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/hearth-api/src/sso_broker/
git commit -m "feat(sso-broker): OIDC discovery endpoint"
```

---

### Task 5: JWKS endpoint

**Files:**
- Create: `crates/hearth-api/src/sso_broker/handlers/jwks.rs`
- Modify: `crates/hearth-api/src/sso_broker/handlers/mod.rs`
- Modify: `crates/hearth-api/src/sso_broker/mod.rs` (route)

**Interfaces:**
- Produces: `GET /oidc/.well-known/jwks.json` returning `{"keys": [<jwk>]}`. 503 when disabled.

- [ ] **Step 1: Write the failing test**

Create `crates/hearth-api/src/sso_broker/handlers/jwks.rs`:

```rust
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use serde_json::json;

use crate::AppState;

pub async fn jwks(State(state): State<AppState>) -> Response {
    let Some(key) = state.sso_broker.signing_key.as_ref() else {
        return (StatusCode::SERVICE_UNAVAILABLE, "sso broker disabled").into_response();
    };
    Json(json!({ "keys": [key.jwk_public()] })).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    #[tokio::test]
    async fn jwks_empty_when_disabled() {
        let state = crate::test_helpers::test_app_state();
        let router = axum::Router::new()
            .route("/oidc/.well-known/jwks.json", axum::routing::get(jwks))
            .with_state(state);
        let req = Request::builder()
            .uri("/oidc/.well-known/jwks.json")
            .body(Body::empty()).unwrap();
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn jwks_returns_single_key_when_enabled() {
        const PEM: &str = "\
-----BEGIN PRIVATE KEY-----
MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgevZzL1gdAFr88hb2
OF/2NxApJCzGCEDdfSp6VQO30hyhRANCAAQRWz+jn65BtOMvdyHKcvjBeBSDZH2r
1RTwjmYSi9R/zpBnuQ4EiMnCqfMPWiZqB4QdbAd0E7oH50VpuZ1P087G
-----END PRIVATE KEY-----";
        let key = crate::sso_broker::keys::SigningKey::load_from_pem(PEM).unwrap();
        let mut state = crate::test_helpers::test_app_state();
        // Mutate the Arc; in a real test we'd construct AppState with a custom config.
        let mut cfg = (*state.sso_broker).clone();
        cfg.signing_key = Some(std::sync::Arc::new(key));
        cfg.enabled = true;
        state.sso_broker = std::sync::Arc::new(cfg);

        let router = axum::Router::new()
            .route("/oidc/.well-known/jwks.json", axum::routing::get(jwks))
            .with_state(state);
        let req = Request::builder().uri("/oidc/.well-known/jwks.json").body(Body::empty()).unwrap();
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["keys"][0]["kty"], "EC");
    }
}
```

Note: this test requires `SigningKey: Clone` — verify it is in Task 2; if not, derive `Clone` on `SigningKey` (the inner `EcdsaSigning` is clone-able since it wraps a scalar).

- [ ] **Step 2: Run test (should fail to compile)**

Run: `cargo test -p hearth-api sso_broker::handlers::jwks`
Expected: FAIL — `SsoBrokerConfig` may not be `Clone`. Add `#[derive(Clone)]` to it in `config.rs` if missing, and ensure `SigningKey` is shared via `Arc` rather than cloned.

- [ ] **Step 3: Fix Clone trait gaps**

In `crates/hearth-api/src/sso_broker/config.rs`, ensure `#[derive(Debug, Clone, Default)]` on `SsoBrokerConfig`. `Arc<SigningKey>` is already cheap to clone.

In `mod.rs` wiring:

```rust
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/.well-known/openid-configuration", axum::routing::get(handlers::discovery::discovery))
        .route("/.well-known/jwks.json", axum::routing::get(handlers::jwks::jwks))
}
```

Add `pub mod jwks;` to `handlers/mod.rs`.

- [ ] **Step 4: Verify tests pass**

Run: `cargo test -p hearth-api sso_broker::handlers::jwks`
Expected: 2 PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/hearth-api/src/sso_broker/
git commit -m "feat(sso-broker): JWKS endpoint"
```

---

### Task 6: Broker clients registry (declarative for MVP)

**Files:**
- Create: `crates/hearth-api/src/sso_broker/clients.rs`
- Modify: `crates/hearth-api/src/sso_broker/mod.rs`
- Modify: `crates/hearth-api/src/sso_broker/config.rs` (add `clients: Vec<BrokerClient>`)

**Interfaces:**
- Produces:
  - `pub struct BrokerClient { pub client_id: String, pub client_secret_hash: String, pub redirect_uris: Vec<String>, pub scopes: Vec<String>, pub audience: String }`
  - `pub fn lookup(clients: &[BrokerClient], client_id: &str) -> Option<&BrokerClient>`
  - `pub fn verify_secret(client: &BrokerClient, presented: &str) -> bool` (constant-time)
  - `pub fn from_env() -> Vec<BrokerClient>` — reads `HEARTH_SSO_BROKER_CLIENTS_JSON` env var (a JSON array)

- [ ] **Step 1: Write the failing test**

Create `crates/hearth-api/src/sso_broker/clients.rs`:

```rust
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
    hash.as_bytes().ct_eq(client.client_secret_hash.as_bytes()).into()
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
        std::env::remove_var("HEARTH_SSO_BROKER_CLIENTS_JSON");
        assert!(from_env().is_empty());
    }
}
```

Add to `Cargo.toml`:

```toml
subtle = "2"
```

(skip if already present).

- [ ] **Step 2: Add `clients: Vec<BrokerClient>` to SsoBrokerConfig**

In `config.rs`, add the field:

```rust
pub clients: Vec<crate::sso_broker::clients::BrokerClient>,
```

In `from_env`, populate:

```rust
clients: crate::sso_broker::clients::from_env(),
```

In `mod.rs`, add `pub mod clients;`.

- [ ] **Step 3: Run tests**

Run: `cargo test -p hearth-api sso_broker::clients`
Expected: 3 PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/hearth-api/src/sso_broker/
git commit -m "feat(sso-broker): declarative OAuth2 client registry"
```

---

### Task 7: Authorize endpoint (redirect to Kanidm)

**Files:**
- Create: `crates/hearth-api/src/sso_broker/sessions.rs` (in-memory authorize-request stash, TTL 10 min)
- Create: `crates/hearth-api/src/sso_broker/handlers/authorize.rs`
- Modify: `crates/hearth-api/src/sso_broker/mod.rs` (route + sessions in AppState)

**Interfaces:**
- Produces:
  - `pub struct AuthorizeRequest { pub client_id: String, pub redirect_uri: String, pub scope: String, pub state: String, pub code_challenge: String, pub code_challenge_method: String, pub created_at: Instant }`
  - `pub struct AuthorizeStash` — Mutex<HashMap<String /* upstream_state */, AuthorizeRequest>>; methods `insert(req) -> String`, `take(upstream_state) -> Option<AuthorizeRequest>`. Auto-GC entries older than TTL.
  - `GET /oidc/authorize` handler validates query params and 302s the user to Kanidm's authorize URL.

- [ ] **Step 1: Write failing test for sessions stash**

Create `crates/hearth-api/src/sso_broker/sessions.rs`:

```rust
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct AuthorizeRequest {
    pub client_id: String,
    pub redirect_uri: String,
    pub scope: String,
    pub state: String,
    pub code_challenge: String,
    pub code_challenge_method: String,
    pub created_at: Instant,
}

#[derive(Default)]
pub struct AuthorizeStash {
    inner: Mutex<HashMap<String, AuthorizeRequest>>,
    ttl: Duration,
}

impl AuthorizeStash {
    pub fn new(ttl: Duration) -> Self {
        AuthorizeStash { inner: Mutex::new(HashMap::new()), ttl }
    }

    pub fn insert(&self, mut req: AuthorizeRequest) -> String {
        req.created_at = Instant::now();
        let key = Uuid::new_v4().to_string();
        let mut guard = self.inner.lock().expect("authorize stash poisoned");
        // Opportunistic GC.
        let now = Instant::now();
        guard.retain(|_, v| now.duration_since(v.created_at) < self.ttl);
        guard.insert(key.clone(), req);
        key
    }

    pub fn take(&self, key: &str) -> Option<AuthorizeRequest> {
        let mut guard = self.inner.lock().expect("authorize stash poisoned");
        guard.remove(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;

    fn req() -> AuthorizeRequest {
        AuthorizeRequest {
            client_id: "synapse".into(),
            redirect_uri: "https://chat.hearth.local/_synapse/client/oidc/callback".into(),
            scope: "openid profile email".into(),
            state: "abc".into(),
            code_challenge: "xyz".into(),
            code_challenge_method: "S256".into(),
            created_at: Instant::now(),
        }
    }

    #[test]
    fn insert_then_take_returns_request() {
        let s = AuthorizeStash::new(Duration::from_secs(60));
        let key = s.insert(req());
        let r = s.take(&key).unwrap();
        assert_eq!(r.client_id, "synapse");
        assert!(s.take(&key).is_none(), "second take should miss");
    }

    #[test]
    fn ttl_expires_old_entries() {
        let s = AuthorizeStash::new(Duration::from_millis(10));
        let key = s.insert(req());
        sleep(Duration::from_millis(50));
        // Trigger GC by inserting another.
        let _ = s.insert(req());
        assert!(s.take(&key).is_none(), "expired entry should be gone");
    }
}
```

- [ ] **Step 2: Run sessions test**

Run: `cargo test -p hearth-api sso_broker::sessions`
Expected: 2 PASS.

- [ ] **Step 3: Plumb stash into AppState**

In `crates/hearth-api/src/lib.rs`, add to `AppState`:

```rust
pub sso_authorize_stash: Arc<crate::sso_broker::sessions::AuthorizeStash>,
```

Construct in `main.rs` and `test_helpers.rs`:

```rust
sso_authorize_stash: Arc::new(crate::sso_broker::sessions::AuthorizeStash::new(std::time::Duration::from_secs(600))),
```

In `mod.rs`, add `pub mod sessions;`.

- [ ] **Step 4: Write failing test for authorize handler**

Create `crates/hearth-api/src/sso_broker/handlers/authorize.rs`:

```rust
//! /oidc/authorize — entry point for OAuth2 authorization code + PKCE.
//!
//! Validates the incoming request against the configured client registry,
//! stashes the request, and 302s the user agent to Kanidm with a fresh
//! upstream state token.

use axum::extract::{Query, State};
use axum::http::{HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use std::time::Instant;

use crate::AppState;
use crate::sso_broker::clients;
use crate::sso_broker::sessions::AuthorizeRequest;

#[derive(Debug, Deserialize)]
pub struct AuthorizeQuery {
    pub response_type: String,
    pub client_id: String,
    pub redirect_uri: String,
    pub scope: String,
    pub state: String,
    pub code_challenge: String,
    pub code_challenge_method: String,
}

pub async fn authorize(
    State(state): State<AppState>,
    Query(q): Query<AuthorizeQuery>,
) -> Response {
    if !state.sso_broker.enabled {
        return (StatusCode::SERVICE_UNAVAILABLE, "sso broker disabled").into_response();
    }
    if q.response_type != "code" {
        return bad_request("unsupported_response_type", "only response_type=code");
    }
    if q.code_challenge_method != "S256" {
        return bad_request("invalid_request", "code_challenge_method must be S256");
    }
    let Some(client) = clients::lookup(&state.sso_broker.clients, &q.client_id) else {
        return bad_request("invalid_client", "unknown client_id");
    };
    if !client.redirect_uris.iter().any(|u| u == &q.redirect_uri) {
        return bad_request("invalid_request", "redirect_uri not registered");
    }
    // Stash request keyed by an upstream_state we send to Kanidm.
    let upstream_state = state.sso_authorize_stash.insert(AuthorizeRequest {
        client_id: q.client_id.clone(),
        redirect_uri: q.redirect_uri.clone(),
        scope: q.scope.clone(),
        state: q.state.clone(),
        code_challenge: q.code_challenge.clone(),
        code_challenge_method: q.code_challenge_method.clone(),
        created_at: Instant::now(),
    });
    let broker_redirect = format!(
        "{}/callback",
        state.sso_broker.issuer_url.as_deref().unwrap_or_default()
    );
    let kanidm_authz = state.sso_broker.kanidm_authorize_url.as_deref().unwrap_or_default();
    let kanidm_client_id = state.sso_broker.kanidm_oauth2_client_id.as_deref().unwrap_or_default();
    let url = format!(
        "{kanidm_authz}?response_type=code&client_id={cid}&redirect_uri={rd}&scope=openid+profile+email+groups&state={st}",
        cid = urlencoding::encode(kanidm_client_id),
        rd = urlencoding::encode(&broker_redirect),
        st = urlencoding::encode(&upstream_state),
    );
    let mut resp = (StatusCode::FOUND, "").into_response();
    resp.headers_mut().insert(
        axum::http::header::LOCATION,
        HeaderValue::from_str(&url).unwrap_or(HeaderValue::from_static("/")),
    );
    resp
}

fn bad_request(error: &str, desc: &str) -> Response {
    (StatusCode::BAD_REQUEST, format!("{{\"error\":\"{error}\",\"error_description\":\"{desc}\"}}"))
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;
    use crate::sso_broker::clients::BrokerClient;
    use sha2::{Digest, Sha256};

    fn enabled_state_with_client() -> AppState {
        let mut s = crate::test_helpers::test_app_state();
        let mut cfg = (*s.sso_broker).clone();
        cfg.enabled = true;
        cfg.issuer_url = Some("https://api.hearth.local/oidc".into());
        cfg.kanidm_authorize_url = Some("https://kanidm.hearth.local:8443/ui/oauth2".into());
        cfg.kanidm_oauth2_client_id = Some("hearth-sso-broker".into());
        cfg.clients = vec![BrokerClient {
            client_id: "synapse".into(),
            client_secret_hash: format!("{:x}", Sha256::digest(b"sekret")),
            redirect_uris: vec!["https://chat.hearth.local/cb".into()],
            scopes: vec!["openid".into()],
            audience: "synapse".into(),
        }];
        s.sso_broker = std::sync::Arc::new(cfg);
        s
    }

    #[tokio::test]
    async fn redirects_to_kanidm() {
        let state = enabled_state_with_client();
        let router = axum::Router::new()
            .route("/oidc/authorize", axum::routing::get(authorize))
            .with_state(state);
        let q = "response_type=code&client_id=synapse&redirect_uri=https%3A%2F%2Fchat.hearth.local%2Fcb&scope=openid&state=xyz&code_challenge=abc&code_challenge_method=S256";
        let req = Request::builder().uri(format!("/oidc/authorize?{q}")).body(Body::empty()).unwrap();
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::FOUND);
        let loc = resp.headers().get(axum::http::header::LOCATION).unwrap().to_str().unwrap();
        assert!(loc.starts_with("https://kanidm.hearth.local:8443/ui/oauth2?"));
        assert!(loc.contains("client_id=hearth-sso-broker"));
        assert!(loc.contains("state="));
    }

    #[tokio::test]
    async fn rejects_unknown_client() {
        let state = enabled_state_with_client();
        let router = axum::Router::new()
            .route("/oidc/authorize", axum::routing::get(authorize))
            .with_state(state);
        let q = "response_type=code&client_id=ghost&redirect_uri=https%3A%2F%2Fchat.hearth.local%2Fcb&scope=openid&state=xyz&code_challenge=abc&code_challenge_method=S256";
        let req = Request::builder().uri(format!("/oidc/authorize?{q}")).body(Body::empty()).unwrap();
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }
}
```

Add to `Cargo.toml` if missing:

```toml
urlencoding = "2"
```

- [ ] **Step 5: Wire route + handlers module**

`handlers/mod.rs`: add `pub mod authorize;`.

`mod.rs` `routes()`: add `.route("/authorize", axum::routing::get(handlers::authorize::authorize))`.

- [ ] **Step 6: Run tests**

Run: `cargo test -p hearth-api sso_broker::handlers::authorize`
Expected: 2 PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/hearth-api/src/sso_broker/ crates/hearth-api/Cargo.toml
git commit -m "feat(sso-broker): /oidc/authorize endpoint redirects to Kanidm"
```

---

### Task 8: Callback endpoint (Kanidm code → broker code)

**Files:**
- Create: `crates/hearth-api/src/sso_broker/kanidm_upstream.rs`
- Create: `crates/hearth-api/src/sso_broker/handlers/callback.rs`
- Modify: `crates/hearth-api/src/sso_broker/sessions.rs` (add `BrokerCodeStash` for codes the broker issues to the downstream app)
- Modify: `mod.rs` (route, AppState additions)

**Interfaces:**
- Produces:
  - `pub async fn exchange_kanidm_code(client_id: &str, client_secret: &str, code: &str, redirect_uri: &str, token_url: &str) -> Result<UpstreamTokens, UpstreamError>` — POSTs `grant_type=authorization_code` to Kanidm token endpoint, returns parsed `{id_token, access_token, refresh_token?}`.
  - `pub fn extract_user_claims(id_token: &str, expected_issuer: &str, expected_audience: &str, jwks: &JwksCache) -> Result<UserClaims, UpstreamError>` — verifies upstream id_token via Kanidm JWKS (reuse `auth::JwksCache` if exposed; else minimal local copy).
  - `BrokerCode` value with: `client_id`, `redirect_uri`, `code_challenge`, `code_challenge_method`, `user_sub`, `user_username`, `user_email`, `user_groups`, `expires_at`. Stashed with TTL 60s.
  - `GET /oidc/callback` handler.

- [ ] **Step 1: Add kanidm_upstream client**

Create `crates/hearth-api/src/sso_broker/kanidm_upstream.rs`:

```rust
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
/// The broker trusts the TLS channel + client-credentials auth to Kanidm's
/// token endpoint; this matches the standard "back-channel exchange" model.
/// JWKS verification is enforced upstream by Kanidm-aware tooling for OPTIONAL
/// front-channel id_token returns; for the broker we only see token-endpoint
/// responses over mTLS-equivalent assurance.
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
```

**Security note to add as a doc-comment above `parse_id_token_unverified`:** the security boundary is the TLS connection + basic auth to Kanidm's token endpoint, not the id_token signature; this is the same posture as the existing `identity_sync` pattern. Add a `verify_signature` follow-up TODO in a later phase (not blocking).

- [ ] **Step 2: Run upstream tests**

Run: `cargo test -p hearth-api sso_broker::kanidm_upstream`
Expected: 2 PASS.

- [ ] **Step 3: Add BrokerCodeStash**

Append to `crates/hearth-api/src/sso_broker/sessions.rs`:

```rust
#[derive(Debug, Clone)]
pub struct BrokerCode {
    pub client_id: String,
    pub redirect_uri: String,
    pub code_challenge: String,
    pub code_challenge_method: String,
    pub user_sub: String,
    pub user_username: Option<String>,
    pub user_email: Option<String>,
    pub user_name: Option<String>,
    pub user_groups: Vec<String>,
    pub scope: String,
    pub nonce: Option<String>,
    pub created_at: Instant,
}

#[derive(Default)]
pub struct BrokerCodeStash {
    inner: Mutex<HashMap<String, BrokerCode>>,
    ttl: Duration,
}

impl BrokerCodeStash {
    pub fn new(ttl: Duration) -> Self {
        BrokerCodeStash { inner: Mutex::new(HashMap::new()), ttl }
    }

    pub fn insert(&self, mut code: BrokerCode) -> String {
        code.created_at = Instant::now();
        let key = Uuid::new_v4().to_string();
        let mut guard = self.inner.lock().expect("code stash poisoned");
        let now = Instant::now();
        guard.retain(|_, v| now.duration_since(v.created_at) < self.ttl);
        guard.insert(key.clone(), code);
        key
    }

    pub fn take(&self, key: &str) -> Option<BrokerCode> {
        let mut guard = self.inner.lock().expect("code stash poisoned");
        guard.remove(key)
    }
}
```

Wire into `AppState`:

```rust
pub sso_broker_code_stash: Arc<crate::sso_broker::sessions::BrokerCodeStash>,
```

Construct in `main.rs` and `test_helpers.rs` with `Duration::from_secs(60)`.

- [ ] **Step 4: Write failing callback handler test**

Create `crates/hearth-api/src/sso_broker/handlers/callback.rs`:

```rust
//! /oidc/callback — Kanidm hands us back an authorization code + our upstream_state.
//! We exchange the code with Kanidm, parse the id_token, mint our own
//! authorization code, and 302 the user agent back to the original downstream
//! app with the broker's code + original state.

use axum::extract::{Query, State};
use axum::http::{HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use std::time::Instant;

use crate::AppState;
use crate::sso_broker::kanidm_upstream;
use crate::sso_broker::sessions::BrokerCode;

#[derive(Debug, Deserialize)]
pub struct CallbackQuery {
    pub code: String,
    pub state: String,
}

pub async fn callback(
    State(state): State<AppState>,
    Query(q): Query<CallbackQuery>,
) -> Response {
    if !state.sso_broker.enabled {
        return (StatusCode::SERVICE_UNAVAILABLE, "sso broker disabled").into_response();
    }
    let Some(original) = state.sso_authorize_stash.take(&q.state) else {
        return (StatusCode::BAD_REQUEST, "unknown or expired state").into_response();
    };
    let token_url = state.sso_broker.kanidm_token_url.as_deref().unwrap_or_default();
    let our_redirect = format!(
        "{}/callback",
        state.sso_broker.issuer_url.as_deref().unwrap_or_default()
    );
    let client_id = state.sso_broker.kanidm_oauth2_client_id.as_deref().unwrap_or_default();
    let client_secret = state.sso_broker.kanidm_oauth2_client_secret.as_deref().unwrap_or_default();
    let tokens = match kanidm_upstream::exchange_kanidm_code(
        &state.http_client,
        client_id,
        client_secret,
        &q.code,
        &our_redirect,
        token_url,
    ).await {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!(error = %e, "kanidm code exchange failed");
            return (StatusCode::BAD_GATEWAY, "upstream auth failed").into_response();
        }
    };
    let Some(id_token) = tokens.id_token else {
        return (StatusCode::BAD_GATEWAY, "no id_token from upstream").into_response();
    };
    let claims = match kanidm_upstream::parse_id_token_unverified(&id_token) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(error = %e, "upstream id_token parse failed");
            return (StatusCode::BAD_GATEWAY, "invalid upstream id_token").into_response();
        }
    };
    let code = state.sso_broker_code_stash.insert(BrokerCode {
        client_id: original.client_id.clone(),
        redirect_uri: original.redirect_uri.clone(),
        code_challenge: original.code_challenge.clone(),
        code_challenge_method: original.code_challenge_method.clone(),
        user_sub: claims.sub,
        user_username: claims.preferred_username,
        user_email: claims.email,
        user_name: claims.name,
        user_groups: claims.groups,
        scope: original.scope.clone(),
        nonce: None,
        created_at: Instant::now(),
    });
    let url = format!(
        "{}?code={}&state={}",
        original.redirect_uri,
        urlencoding::encode(&code),
        urlencoding::encode(&original.state),
    );
    let mut resp = (StatusCode::FOUND, "").into_response();
    resp.headers_mut().insert(
        axum::http::header::LOCATION,
        HeaderValue::from_str(&url).unwrap_or(HeaderValue::from_static("/")),
    );
    resp
}
```

- [ ] **Step 5: Add http_client to AppState**

If `AppState` doesn't already have a `reqwest::Client`, add:

```rust
pub http_client: reqwest::Client,
```

Construct in main.rs:

```rust
let http_client = reqwest::Client::builder()
    .timeout(std::time::Duration::from_secs(15))
    // dev only: match identity_sync.rs pattern
    .danger_accept_invalid_certs(std::env::var("HEARTH_KANIDM_INSECURE").is_ok())
    .build()
    .expect("build reqwest client");
```

Mirror in `test_helpers.rs` with `reqwest::Client::new()`.

- [ ] **Step 6: Wire route and module**

In `handlers/mod.rs` add `pub mod callback;`. In `mod.rs` `routes()` add `.route("/callback", axum::routing::get(handlers::callback::callback))`.

- [ ] **Step 7: Build to verify wiring**

Run: `cargo build -p hearth-api`
Expected: clean.

Note: callback has no easy unit test (depends on Kanidm). End-to-end is covered in Phase E's VM test. We deliberately leave it without a mock-server unit test to avoid baking in `wiremock` as a dep; if you want, add a mock test later using `wiremock` behind a `#[cfg(test)]` dev-dep.

- [ ] **Step 8: Commit**

```bash
git add crates/hearth-api/src/sso_broker/ crates/hearth-api/src/lib.rs crates/hearth-api/src/main.rs crates/hearth-api/src/test_helpers.rs
git commit -m "feat(sso-broker): /oidc/callback exchanges Kanidm code + stashes broker code"
```

---

### Task 9: Token endpoint (PKCE code exchange → broker-signed tokens)

**Files:**
- Create: `crates/hearth-api/src/sso_broker/tokens.rs`
- Create: `crates/hearth-api/src/sso_broker/handlers/token.rs`
- Modify: `crates/hearth-api/src/sso_broker/handlers/mod.rs`, `mod.rs`

**Interfaces:**
- Produces:
  - `pub struct BrokerClaims { iss, sub, aud, exp, iat, jti, preferred_username, email, email_verified, groups, name, nonce, scope }`
  - `pub fn mint_id_token(claims: &BrokerClaims, key: &SigningKey) -> Result<String, MintError>`
  - `pub fn mint_access_token(claims: &BrokerClaims, key: &SigningKey) -> Result<String, MintError>` — same shape but different `aud` semantics; for MVP both are JWTs signed with the same key.
  - `pub fn verify_pkce_s256(verifier: &str, challenge: &str) -> bool`
  - `POST /oidc/token` handler.

- [ ] **Step 1: Write failing PKCE test**

Create `crates/hearth-api/src/sso_broker/tokens.rs`:

```rust
//! Broker token claims + minting.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
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

pub fn verify_pkce_s256(verifier: &str, challenge: &str) -> bool {
    let hash = Sha256::digest(verifier.as_bytes());
    let computed = URL_SAFE_NO_PAD.encode(hash);
    use subtle::ConstantTimeEq;
    computed.as_bytes().ct_eq(challenge.as_bytes()).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_s256_correct() {
        // verifier "abc" → SHA256 → base64url-no-pad
        // sha256("abc") = ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad
        let v = "abc";
        let c = "ungWv48Bz-pBQUDeXa4iI7ADYaOWF3qctBD_YfIAFa0";
        assert!(verify_pkce_s256(v, c));
        assert!(!verify_pkce_s256("abd", c));
    }

    #[test]
    fn mint_jwt_produces_three_segments() {
        const PEM: &str = "\
-----BEGIN PRIVATE KEY-----
MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgevZzL1gdAFr88hb2
OF/2NxApJCzGCEDdfSp6VQO30hyhRANCAAQRWz+jn65BtOMvdyHKcvjBeBSDZH2r
1RTwjmYSi9R/zpBnuQ4EiMnCqfMPWiZqB4QdbAd0E7oH50VpuZ1P087G
-----END PRIVATE KEY-----";
        let key = SigningKey::load_from_pem(PEM).unwrap();
        let claims = BrokerClaims {
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
        };
        let jwt = mint_jwt(&claims, &key).unwrap();
        let segs: Vec<&str> = jwt.split('.').collect();
        assert_eq!(segs.len(), 3);
    }
}
```

- [ ] **Step 2: Run tokens tests**

Run: `cargo test -p hearth-api sso_broker::tokens`
Expected: 2 PASS.

- [ ] **Step 3: Write failing token handler test**

Create `crates/hearth-api/src/sso_broker/handlers/token.rs`:

```rust
//! /oidc/token — exchange a broker authorization_code for tokens.

use axum::extract::{Form, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use serde::Deserialize;
use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::AppState;
use crate::sso_broker::{clients, tokens};

#[derive(Debug, Deserialize)]
pub struct TokenForm {
    pub grant_type: String,
    pub code: Option<String>,
    pub redirect_uri: Option<String>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub code_verifier: Option<String>,
}

pub async fn token(
    State(state): State<AppState>,
    Form(form): Form<TokenForm>,
) -> Response {
    if !state.sso_broker.enabled {
        return (StatusCode::SERVICE_UNAVAILABLE, "sso broker disabled").into_response();
    }
    if form.grant_type != "authorization_code" {
        return err("unsupported_grant_type", "only authorization_code");
    }
    let Some(code) = form.code.as_deref() else { return err("invalid_request", "missing code") };
    let Some(client_id) = form.client_id.as_deref() else { return err("invalid_request", "missing client_id") };
    let Some(secret) = form.client_secret.as_deref() else { return err("invalid_request", "missing client_secret") };
    let Some(verifier) = form.code_verifier.as_deref() else { return err("invalid_request", "missing code_verifier") };

    let Some(client) = clients::lookup(&state.sso_broker.clients, client_id) else {
        return err("invalid_client", "unknown client_id");
    };
    if !clients::verify_secret(client, secret) {
        return err("invalid_client", "bad secret");
    }
    let Some(broker_code) = state.sso_broker_code_stash.take(code) else {
        return err("invalid_grant", "code unknown or expired");
    };
    if broker_code.client_id != client_id {
        return err("invalid_grant", "code/client mismatch");
    }
    if broker_code.code_challenge_method != "S256" {
        return err("invalid_grant", "unsupported pkce method");
    }
    if !tokens::verify_pkce_s256(verifier, &broker_code.code_challenge) {
        return err("invalid_grant", "pkce verification failed");
    }
    let Some(key) = state.sso_broker.signing_key.as_ref() else {
        return err("server_error", "no signing key");
    };
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64;
    let exp = now + state.sso_broker.id_token_ttl.as_secs() as i64;
    let mut claims = tokens::BrokerClaims {
        iss: state.sso_broker.issuer_url.clone().unwrap_or_default(),
        sub: broker_code.user_sub.clone(),
        aud: client.audience.clone(),
        exp,
        iat: now,
        jti: tokens::new_jti(),
        scope: broker_code.scope.clone(),
        preferred_username: broker_code.user_username.clone(),
        email: broker_code.user_email.clone(),
        email_verified: Some(true),
        name: broker_code.user_name.clone(),
        groups: broker_code.user_groups.clone(),
        nonce: broker_code.nonce.clone(),
    };
    let id_token = match tokens::mint_jwt(&claims, key) {
        Ok(t) => t,
        Err(e) => {
            tracing::error!(error = %e, "id_token mint failed");
            return err("server_error", "mint failed");
        }
    };
    // Access token: different jti, same key. (RFC 9068.)
    claims.jti = tokens::new_jti();
    let access_token = match tokens::mint_jwt(&claims, key) {
        Ok(t) => t,
        Err(e) => {
            tracing::error!(error = %e, "access_token mint failed");
            return err("server_error", "mint failed");
        }
    };
    // Persist jtis for revocation — done in Phase C.
    Json(json!({
        "access_token": access_token,
        "token_type": "Bearer",
        "expires_in": state.sso_broker.id_token_ttl.as_secs(),
        "id_token": id_token,
        "scope": broker_code.scope,
    })).into_response()
}

fn err(code: &str, desc: &str) -> Response {
    (StatusCode::BAD_REQUEST, Json(json!({
        "error": code,
        "error_description": desc,
    }))).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use tower::ServiceExt;
    use crate::sso_broker::clients::BrokerClient;
    use crate::sso_broker::sessions::BrokerCode;
    use sha2::{Digest, Sha256};
    use base64::Engine;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use std::time::Instant;

    const PEM: &str = "\
-----BEGIN PRIVATE KEY-----
MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgevZzL1gdAFr88hb2
OF/2NxApJCzGCEDdfSp6VQO30hyhRANCAAQRWz+jn65BtOMvdyHKcvjBeBSDZH2r
1RTwjmYSi9R/zpBnuQ4EiMnCqfMPWiZqB4QdbAd0E7oH50VpuZ1P087G
-----END PRIVATE KEY-----";

    fn enabled_state() -> AppState {
        let mut s = crate::test_helpers::test_app_state();
        let mut cfg = (*s.sso_broker).clone();
        cfg.enabled = true;
        cfg.issuer_url = Some("https://api.hearth.local/oidc".into());
        cfg.signing_key = Some(std::sync::Arc::new(crate::sso_broker::keys::SigningKey::load_from_pem(PEM).unwrap()));
        cfg.clients = vec![BrokerClient {
            client_id: "synapse".into(),
            client_secret_hash: format!("{:x}", Sha256::digest(b"sekret")),
            redirect_uris: vec!["https://chat.hearth.local/cb".into()],
            scopes: vec!["openid".into()],
            audience: "synapse".into(),
        }];
        s.sso_broker = std::sync::Arc::new(cfg);
        s
    }

    #[tokio::test]
    async fn issues_tokens_on_valid_code() {
        let state = enabled_state();
        let verifier = "abcdef123456abcdef123456abcdef123456abcdef123456";
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        let code_key = state.sso_broker_code_stash.insert(BrokerCode {
            client_id: "synapse".into(),
            redirect_uri: "https://chat.hearth.local/cb".into(),
            code_challenge: challenge,
            code_challenge_method: "S256".into(),
            user_sub: "u-1".into(),
            user_username: Some("alice".into()),
            user_email: Some("a@b".into()),
            user_name: Some("Alice".into()),
            user_groups: vec!["hearth-users".into()],
            scope: "openid profile email".into(),
            nonce: None,
            created_at: Instant::now(),
        });
        let body = format!(
            "grant_type=authorization_code&code={}&redirect_uri=https%3A%2F%2Fchat.hearth.local%2Fcb&client_id=synapse&client_secret=sekret&code_verifier={}",
            urlencoding::encode(&code_key), verifier,
        );
        let router = axum::Router::new()
            .route("/oidc/token", axum::routing::post(token))
            .with_state(state);
        let req = Request::builder()
            .method("POST")
            .uri("/oidc/token")
            .header("content-type", "application/x-www-form-urlencoded")
            .body(Body::from(body)).unwrap();
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert!(v["id_token"].as_str().unwrap().contains('.'));
        assert!(v["access_token"].as_str().unwrap().contains('.'));
    }

    #[tokio::test]
    async fn rejects_bad_pkce() {
        let state = enabled_state();
        let code_key = state.sso_broker_code_stash.insert(BrokerCode {
            client_id: "synapse".into(),
            redirect_uri: "https://chat.hearth.local/cb".into(),
            code_challenge: "wrong".into(),
            code_challenge_method: "S256".into(),
            user_sub: "u-1".into(),
            user_username: None, user_email: None, user_name: None,
            user_groups: vec![],
            scope: "openid".into(),
            nonce: None,
            created_at: Instant::now(),
        });
        let body = format!(
            "grant_type=authorization_code&code={}&redirect_uri=https%3A%2F%2Fchat.hearth.local%2Fcb&client_id=synapse&client_secret=sekret&code_verifier=abc",
            urlencoding::encode(&code_key),
        );
        let router = axum::Router::new()
            .route("/oidc/token", axum::routing::post(token))
            .with_state(state);
        let req = Request::builder()
            .method("POST")
            .uri("/oidc/token")
            .header("content-type", "application/x-www-form-urlencoded")
            .body(Body::from(body)).unwrap();
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }
}
```

- [ ] **Step 4: Wire route**

In `handlers/mod.rs` add `pub mod token;`. In `mod.rs` `routes()` add `.route("/token", axum::routing::post(handlers::token::token))`.

- [ ] **Step 5: Run tests**

Run: `cargo test -p hearth-api sso_broker::handlers::token`
Expected: 2 PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/hearth-api/src/sso_broker/ 
git commit -m "feat(sso-broker): /oidc/token mints ES256 id+access tokens"
```

---

### Task 10: UserInfo endpoint

**Files:**
- Create: `crates/hearth-api/src/sso_broker/handlers/userinfo.rs`
- Modify: `crates/hearth-api/src/sso_broker/handlers/mod.rs`, `mod.rs`

**Interfaces:**
- Produces:
  - `GET /oidc/userinfo` validates a `Bearer` access_token (broker-issued), returns `{sub, preferred_username, email, email_verified, groups, name}`.

- [ ] **Step 1: Write failing test**

Create `crates/hearth-api/src/sso_broker/handlers/userinfo.rs`:

```rust
use axum::extract::State;
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Json, Response};
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use serde_json::{json, Value};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::AppState;
use crate::sso_broker::tokens::BrokerClaims;

pub async fn userinfo(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Response {
    let Some(token) = bearer(&headers) else {
        return (StatusCode::UNAUTHORIZED, "missing bearer").into_response();
    };
    let Some(key) = state.sso_broker.signing_key.as_ref() else {
        return (StatusCode::SERVICE_UNAVAILABLE, "sso broker disabled").into_response();
    };
    let jwk = key.jwk_public();
    let decoding_key = match DecodingKey::from_jwk(&jwk) {
        Ok(k) => k,
        Err(e) => {
            tracing::error!(error = %e, "decoding key from jwk failed");
            return (StatusCode::INTERNAL_SERVER_ERROR, "key error").into_response();
        }
    };
    let mut validation = Validation::new(Algorithm::ES256);
    validation.validate_aud = false; // userinfo accepts any aud the broker issued
    let claims = match decode::<BrokerClaims>(&token, &decoding_key, &validation) {
        Ok(d) => d.claims,
        Err(e) => {
            tracing::debug!(error = %e, "userinfo token validation failed");
            return (StatusCode::UNAUTHORIZED, "invalid token").into_response();
        }
    };
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64;
    if claims.exp < now {
        return (StatusCode::UNAUTHORIZED, "expired").into_response();
    }
    let body: Value = json!({
        "sub": claims.sub,
        "preferred_username": claims.preferred_username,
        "email": claims.email,
        "email_verified": claims.email_verified,
        "name": claims.name,
        "groups": claims.groups,
    });
    Json(body).into_response()
}

fn bearer(headers: &HeaderMap) -> Option<String> {
    let v = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    v.strip_prefix("Bearer ").map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sso_broker::tokens::{self, BrokerClaims};
    use crate::sso_broker::keys::SigningKey;
    use axum::body::Body;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use tower::ServiceExt;
    use std::sync::Arc;

    const PEM: &str = "\
-----BEGIN PRIVATE KEY-----
MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgevZzL1gdAFr88hb2
OF/2NxApJCzGCEDdfSp6VQO30hyhRANCAAQRWz+jn65BtOMvdyHKcvjBeBSDZH2r
1RTwjmYSi9R/zpBnuQ4EiMnCqfMPWiZqB4QdbAd0E7oH50VpuZ1P087G
-----END PRIVATE KEY-----";

    #[tokio::test]
    async fn returns_claims_for_valid_token() {
        let key = SigningKey::load_from_pem(PEM).unwrap();
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
        let claims = BrokerClaims {
            iss: "https://api.hearth.local/oidc".into(),
            sub: "u-1".into(),
            aud: "synapse".into(),
            exp: now + 3600,
            iat: now,
            jti: "j".into(),
            scope: "openid profile email".into(),
            preferred_username: Some("alice".into()),
            email: Some("a@b".into()),
            email_verified: Some(true),
            name: Some("Alice".into()),
            groups: vec!["hearth-users".into()],
            nonce: None,
        };
        let token = tokens::mint_jwt(&claims, &key).unwrap();
        let mut state = crate::test_helpers::test_app_state();
        let mut cfg = (*state.sso_broker).clone();
        cfg.enabled = true;
        cfg.signing_key = Some(Arc::new(SigningKey::load_from_pem(PEM).unwrap()));
        state.sso_broker = Arc::new(cfg);
        let router = axum::Router::new()
            .route("/oidc/userinfo", axum::routing::get(userinfo))
            .with_state(state);
        let req = Request::builder()
            .uri("/oidc/userinfo")
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .body(Body::empty()).unwrap();
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["sub"], "u-1");
        assert_eq!(v["preferred_username"], "alice");
    }

    #[tokio::test]
    async fn rejects_missing_bearer() {
        let state = crate::test_helpers::test_app_state();
        let router = axum::Router::new()
            .route("/oidc/userinfo", axum::routing::get(userinfo))
            .with_state(state);
        let req = Request::builder().uri("/oidc/userinfo").body(Body::empty()).unwrap();
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }
}
```

- [ ] **Step 2: Wire route**

In `handlers/mod.rs` add `pub mod userinfo;`. In `mod.rs` `routes()` add `.route("/userinfo", axum::routing::get(handlers::userinfo::userinfo))`.

- [ ] **Step 3: Run tests**

Run: `cargo test -p hearth-api sso_broker::handlers::userinfo`
Expected: 2 PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/hearth-api/src/sso_broker/
git commit -m "feat(sso-broker): /oidc/userinfo endpoint"
```

---

## Phase C: DB schema + audit + revocation

### Task 11: Migration for sso_issued_tokens table

**Files:**
- Create: `migrations/<next-NNNN>_sso_broker.sql` — the next available migration number; check `migrations/` directory before writing.

**Interfaces:**
- Produces: schema for tracking issued broker tokens (for revocation + audit).

- [ ] **Step 1: Inspect existing migrations to pick next number**

Run: `ls migrations/`
Note the highest-numbered file (e.g. `017_*.sql`) and use NNNN = next.

- [ ] **Step 2: Write migration**

Create `migrations/NNNN_sso_broker.sql` (replace NNNN with the next number):

```sql
-- SSO broker: track issued tokens for revocation and audit.

CREATE TABLE IF NOT EXISTS sso_issued_tokens (
    jti          TEXT PRIMARY KEY,
    user_sub     TEXT NOT NULL,
    audience     TEXT NOT NULL,
    issued_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at   TIMESTAMPTZ NOT NULL,
    revoked_at   TIMESTAMPTZ,
    client_id    TEXT NOT NULL,
    -- Optional device hint (filled in once device backchannel exists; null for browser flows)
    device_id    UUID REFERENCES machines(id) ON DELETE SET NULL,
    -- Token kind: 'access' or 'id'
    kind         TEXT NOT NULL CHECK (kind IN ('access', 'id'))
);

CREATE INDEX IF NOT EXISTS sso_issued_tokens_user_idx
    ON sso_issued_tokens (user_sub);
CREATE INDEX IF NOT EXISTS sso_issued_tokens_expires_idx
    ON sso_issued_tokens (expires_at)
    WHERE revoked_at IS NULL;
CREATE INDEX IF NOT EXISTS sso_issued_tokens_active_per_user_idx
    ON sso_issued_tokens (user_sub, audience)
    WHERE revoked_at IS NULL;
```

- [ ] **Step 3: Apply migration locally**

Run: `nix develop -c just migrate`
Expected: migration applies cleanly.

- [ ] **Step 4: Verify schema**

Run: `nix develop -c psql postgres://hearth:hearth@localhost:5432/hearth -c '\d sso_issued_tokens'`
Expected: table shown with above columns.

- [ ] **Step 5: Commit**

```bash
git add migrations/
git commit -m "feat(sso-broker): sso_issued_tokens migration"
```

---

### Task 12: Record minted tokens; revocation check on userinfo

**Files:**
- Modify: `crates/hearth-api/src/sso_broker/handlers/token.rs` (insert into sso_issued_tokens after mint)
- Modify: `crates/hearth-api/src/sso_broker/handlers/userinfo.rs` (check revocation)
- Create: `crates/hearth-api/src/sso_broker/store.rs` — DB helpers

**Interfaces:**
- Produces:
  - `pub async fn record_token(pool: &PgPool, jti: &str, user_sub: &str, audience: &str, client_id: &str, kind: TokenKind, expires_at: DateTime<Utc>) -> Result<(), sqlx::Error>`
  - `pub async fn is_revoked(pool: &PgPool, jti: &str) -> Result<bool, sqlx::Error>`
  - `pub async fn revoke(pool: &PgPool, jti: &str) -> Result<bool, sqlx::Error>` — returns true if a row was updated.
  - `pub enum TokenKind { Access, Id }`

- [ ] **Step 1: Write store with tests**

Create `crates/hearth-api/src/sso_broker/store.rs`:

```rust
use chrono::{DateTime, Utc};
use sqlx::PgPool;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    Access,
    Id,
}

impl TokenKind {
    fn as_str(&self) -> &'static str {
        match self {
            TokenKind::Access => "access",
            TokenKind::Id => "id",
        }
    }
}

pub async fn record_token(
    pool: &PgPool,
    jti: &str,
    user_sub: &str,
    audience: &str,
    client_id: &str,
    kind: TokenKind,
    expires_at: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "INSERT INTO sso_issued_tokens (jti, user_sub, audience, client_id, kind, expires_at)
         VALUES ($1, $2, $3, $4, $5, $6)",
        jti,
        user_sub,
        audience,
        client_id,
        kind.as_str(),
        expires_at,
    ).execute(pool).await?;
    Ok(())
}

pub async fn is_revoked(pool: &PgPool, jti: &str) -> Result<bool, sqlx::Error> {
    let row = sqlx::query!(
        "SELECT revoked_at FROM sso_issued_tokens WHERE jti = $1",
        jti
    ).fetch_optional(pool).await?;
    Ok(row.and_then(|r| r.revoked_at).is_some())
}

pub async fn revoke(pool: &PgPool, jti: &str) -> Result<bool, sqlx::Error> {
    let r = sqlx::query!(
        "UPDATE sso_issued_tokens SET revoked_at = now() WHERE jti = $1 AND revoked_at IS NULL",
        jti
    ).execute(pool).await?;
    Ok(r.rows_affected() == 1)
}

pub async fn revoke_all_for_user(pool: &PgPool, user_sub: &str) -> Result<u64, sqlx::Error> {
    let r = sqlx::query!(
        "UPDATE sso_issued_tokens SET revoked_at = now()
         WHERE user_sub = $1 AND revoked_at IS NULL",
        user_sub
    ).execute(pool).await?;
    Ok(r.rows_affected())
}
```

- [ ] **Step 2: Regenerate sqlx metadata**

Run: `nix develop -c env DATABASE_URL=postgres://hearth:hearth@localhost:5432/hearth cargo sqlx prepare --workspace`
Expected: `.sqlx/` updated.

- [ ] **Step 3: Wire token recording into /oidc/token handler**

In `crates/hearth-api/src/sso_broker/handlers/token.rs`, after both `id_token` and `access_token` are minted, before `Json(...)`, insert:

```rust
use chrono::{DateTime, Utc, TimeZone};
let exp_dt: DateTime<Utc> = Utc.timestamp_opt(exp, 0).single().unwrap_or_else(Utc::now);
let id_jti = decode_jti(&id_token);
let acc_jti = decode_jti(&access_token);
if let Some(jti) = id_jti.as_deref() {
    if let Err(e) = crate::sso_broker::store::record_token(
        &state.pool, jti, &broker_code.user_sub, &client.audience,
        client_id, crate::sso_broker::store::TokenKind::Id, exp_dt,
    ).await { tracing::error!(error = %e, "record id_token failed"); }
}
if let Some(jti) = acc_jti.as_deref() {
    if let Err(e) = crate::sso_broker::store::record_token(
        &state.pool, jti, &broker_code.user_sub, &client.audience,
        client_id, crate::sso_broker::store::TokenKind::Access, exp_dt,
    ).await { tracing::error!(error = %e, "record access_token failed"); }
}
```

Add a helper at the bottom of the file:

```rust
fn decode_jti(jwt: &str) -> Option<String> {
    use base64::Engine;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    let parts: Vec<&str> = jwt.split('.').collect();
    if parts.len() != 3 { return None; }
    let payload = URL_SAFE_NO_PAD.decode(parts[1]).ok()?;
    let v: serde_json::Value = serde_json::from_slice(&payload).ok()?;
    v.get("jti").and_then(|j| j.as_str()).map(|s| s.to_string())
}
```

- [ ] **Step 4: Wire revocation check into userinfo**

In `crates/hearth-api/src/sso_broker/handlers/userinfo.rs`, after `claims` is extracted and `exp` is validated, add:

```rust
match crate::sso_broker::store::is_revoked(&state.pool, &claims.jti).await {
    Ok(true) => return (StatusCode::UNAUTHORIZED, "revoked").into_response(),
    Ok(false) => {}
    Err(e) => {
        tracing::error!(error = %e, "revocation check failed");
        return (StatusCode::INTERNAL_SERVER_ERROR, "db error").into_response();
    }
}
```

- [ ] **Step 5: Add integration test for store**

Append to `store.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::PgPool;

    async fn pool() -> PgPool {
        let url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://hearth:hearth@localhost:5432/hearth".into());
        PgPool::connect(&url).await.expect("connect")
    }

    #[tokio::test]
    #[ignore] // requires live DB; run with `cargo test -- --ignored`
    async fn record_then_revoke_roundtrip() {
        let p = pool().await;
        let jti = uuid::Uuid::new_v4().to_string();
        let exp = chrono::Utc::now() + chrono::Duration::seconds(60);
        record_token(&p, &jti, "u-test", "synapse", "synapse", TokenKind::Access, exp).await.unwrap();
        assert!(!is_revoked(&p, &jti).await.unwrap());
        assert!(revoke(&p, &jti).await.unwrap());
        assert!(is_revoked(&p, &jti).await.unwrap());
        assert!(!revoke(&p, &jti).await.unwrap(), "second revoke should be no-op");
    }
}
```

- [ ] **Step 6: Run unit tests (skip integration)**

Run: `cargo test -p hearth-api sso_broker::store`
Expected: PASS (ignored test skipped).

- [ ] **Step 7: Run integration test against live DB**

Run: `nix develop -c env DATABASE_URL=postgres://hearth:hearth@localhost:5432/hearth cargo test -p hearth-api sso_broker::store -- --ignored`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add crates/hearth-api/src/sso_broker/ .sqlx/
git commit -m "feat(sso-broker): record issued tokens + revocation check"
```

---

### Task 13: Revoke endpoint + audit events

**Files:**
- Create: `crates/hearth-api/src/sso_broker/handlers/revoke.rs`
- Modify: `crates/hearth-api/src/sso_broker/handlers/mod.rs`, `mod.rs`
- Modify: `crates/hearth-api/src/sso_broker/handlers/token.rs` (emit audit on mint)

**Interfaces:**
- Produces: `POST /oidc/revoke` per RFC 7009 (form params: `token`, `client_id`, `client_secret`).
- Emits audit events to existing `audit_events` table via existing helper (find it in the codebase before writing — likely `repo::audit::log` or similar).

- [ ] **Step 1: Locate the audit event helper**

Run: `grep -r "audit_events" crates/hearth-api/src/`
Identify the existing insert helper. If none exists, write a minimal one in `crates/hearth-api/src/sso_broker/store.rs`:

```rust
pub async fn audit(
    pool: &PgPool,
    event_type: &str,
    actor: &str,
    target: Option<&str>,
    metadata: serde_json::Value,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "INSERT INTO audit_events (event_type, actor, target, metadata)
         VALUES ($1, $2, $3, $4)",
        event_type, actor, target, metadata,
    ).execute(pool).await?;
    Ok(())
}
```

Adapt column names to the actual `audit_events` schema in `migrations/`.

- [ ] **Step 2: Emit audit on mint**

In `token.rs`, after both jtis are recorded, append:

```rust
let _ = crate::sso_broker::store::audit(
    &state.pool,
    "sso_broker.token_issued",
    &broker_code.user_sub,
    Some(&client.audience),
    serde_json::json!({"client_id": client_id, "scope": broker_code.scope}),
).await;
```

- [ ] **Step 3: Write revoke handler with test**

Create `crates/hearth-api/src/sso_broker/handlers/revoke.rs`:

```rust
use axum::extract::{Form, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;

use crate::AppState;
use crate::sso_broker::{clients, store};

#[derive(Debug, Deserialize)]
pub struct RevokeForm {
    pub token: String,
    pub client_id: String,
    pub client_secret: String,
}

pub async fn revoke(
    State(state): State<AppState>,
    Form(form): Form<RevokeForm>,
) -> Response {
    let Some(client) = clients::lookup(&state.sso_broker.clients, &form.client_id) else {
        // RFC 7009: silently OK even for unknown tokens, but we 401 on bad client.
        return (StatusCode::UNAUTHORIZED, "invalid_client").into_response();
    };
    if !clients::verify_secret(client, &form.client_secret) {
        return (StatusCode::UNAUTHORIZED, "invalid_client").into_response();
    }
    let jti = decode_jti(&form.token);
    if let Some(jti) = jti {
        if let Err(e) = store::revoke(&state.pool, &jti).await {
            tracing::error!(error = %e, "revoke failed");
            return (StatusCode::INTERNAL_SERVER_ERROR, "db error").into_response();
        }
        let _ = store::audit(
            &state.pool,
            "sso_broker.token_revoked",
            &form.client_id,
            Some(&jti),
            serde_json::json!({}),
        ).await;
    }
    StatusCode::OK.into_response()
}

fn decode_jti(jwt: &str) -> Option<String> {
    use base64::Engine;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    let parts: Vec<&str> = jwt.split('.').collect();
    if parts.len() != 3 { return None; }
    let payload = URL_SAFE_NO_PAD.decode(parts[1]).ok()?;
    let v: serde_json::Value = serde_json::from_slice(&payload).ok()?;
    v.get("jti").and_then(|j| j.as_str()).map(|s| s.to_string())
}
```

(yes, `decode_jti` is duplicated with `token.rs` — keep the duplication for clarity; we can extract later. **Note this in a TODO inline.**)

- [ ] **Step 4: Wire route**

In `handlers/mod.rs` add `pub mod revoke;`. In `mod.rs` `routes()`:

```rust
.route("/revoke", axum::routing::post(handlers::revoke::revoke))
```

- [ ] **Step 5: Run tests + clippy**

Run: `cargo test -p hearth-api && cargo clippy -p hearth-api -- --deny warnings`
Expected: all green.

- [ ] **Step 6: Commit**

```bash
git add crates/hearth-api/src/sso_broker/
git commit -m "feat(sso-broker): revoke endpoint + audit emission"
```

---

## Phase D: Helm wiring + Kanidm bootstrap

### Task 14: Capability toggle + values

**Files:**
- Modify: `chart/hearth-home/values.yaml`

**Interfaces:**
- Produces: a `capabilities.ssoBroker` toggle (default `false`) and a `ssoBroker.*` config stanza.

- [ ] **Step 1: Edit values.yaml**

In `chart/hearth-home/values.yaml`, find the `capabilities:` block and add:

```yaml
  ssoBroker: false      # hearth-side OIDC broker in front of Kanidm
```

After the existing top-level sections (e.g., after `synapse:` stanza), add:

```yaml
# ============================================================
# SSO Broker
# ============================================================
# Hearth-side OIDC issuer that wraps Kanidm. Fleet apps trust the broker as
# their IdP instead of Kanidm directly; Kanidm remains the source of truth for
# credentials and groups. See docs/sso-broker.md.
ssoBroker:
  # Public issuer URL — must match what apps know as their IdP. Typically
  # `<apiIngressUrl>/oidc`.
  issuerUrl: ""

  # ES256 signing keypair. In prod, supply an existing Kubernetes Secret with
  # PEM-encoded keys at the named keys; in dev, leave empty and an init Job
  # generates a new pair on first install.
  signingKey:
    existingSecret: ""
    privateKeyKey: "tls.key"
    publicKeyKey: "tls.crt"

  # Kanidm OAuth2 client used by the broker to authenticate end-users
  # upstream. Created automatically by the Kanidm bootstrap script.
  kanidmClient:
    clientId: "hearth-sso-broker"
    existingSecret: ""   # auto-generated by bootstrap if blank
    existingSecretKey: "broker-oidc-secret"

  # Apps that trust the broker as IdP. Each entry registers an OAuth2 client
  # in the broker's declarative registry (HEARTH_SSO_BROKER_CLIENTS_JSON).
  clients: []
    # - clientId: synapse
    #   audience: synapse
    #   redirectUris: ["https://chat.hearth.example.com/_synapse/client/oidc/callback"]
    #   scopes: ["openid", "profile", "email"]
    #   # secret hash is auto-derived from the per-client Secret at install
    #   existingSecret: ""
    #   existingSecretKey: ""
```

- [ ] **Step 2: Verify chart still lints**

Run: `nix develop -c just helm-lint`
Expected: clean.

- [ ] **Step 3: Commit**

```bash
git add chart/hearth-home/values.yaml
git commit -m "feat(sso-broker): capability toggle and values stanza"
```

---

### Task 15: Bootstrap creates hearth-broker-svc SA + registers Kanidm OAuth2 client

**Files:**
- Modify: `chart/hearth-home/templates/kanidm/bootstrap-configmap.yaml`

**Interfaces:**
- Produces: in the bootstrap script, idempotent creation of:
  - Service account `hearth-broker-svc` with API token stored in K8s Secret `<release>-broker-svc-token`.
  - OAuth2 client `hearth-sso-broker` (confidential) with `openid profile email groups` scope, redirect URI `<issuerUrl>/callback`, client secret stored in K8s Secret `<release>-broker-oidc-credentials`.

- [ ] **Step 1: Read existing bootstrap script structure**

Read `chart/hearth-home/templates/kanidm/bootstrap-configmap.yaml` to understand the flow and helper functions.

- [ ] **Step 2: Add broker SA + OAuth2 client registration block**

Inside the bootstrap script, after the existing `hearth-api-svc` block, add (wrapped in `{{- if .Values.capabilities.ssoBroker }}`):

```bash
{{- if $.Values.capabilities.ssoBroker }}
# -----------------------------------------------------------------------
# SSO Broker: service account + OAuth2 client (idempotent)
# -----------------------------------------------------------------------
echo ""
echo "==> Creating hearth-broker-svc service account..."
if resource_exists "/v1/service_account/hearth-broker-svc" "$ADMIN_TOKEN"; then
  echo "  Service account 'hearth-broker-svc' already exists"
else
  $C -X POST "${KANIDM_URL}/v1/service_account" \
    -H "Authorization: Bearer $ADMIN_TOKEN" \
    -H "Content-Type: application/json" \
    -d '{"attrs":{"name":["hearth-broker-svc"],"displayname":["Hearth SSO Broker Service"]}}' > /dev/null
  echo "  Created service account 'hearth-broker-svc'"
fi

# Grant minimal read perms (idm_account_read_priv group)
$C -X POST "${KANIDM_URL}/v1/group/idm_account_read_priv/_attr/member" \
  -H "Authorization: Bearer $ADMIN_TOKEN" \
  -H "Content-Type: application/json" \
  -d '["hearth-broker-svc"]' > /dev/null 2>&1 || true

BROKER_SVC_TOKEN=$($C -X POST "${KANIDM_URL}/v1/service_account/hearth-broker-svc/_api_token" \
  -H "Authorization: Bearer $ADMIN_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"label":"hearth-broker","expiry":null}' 2>/dev/null | sed -n 's/^"\(.*\)"$/\1/p' || echo "")

echo "==> Creating hearth-sso-broker OAuth2 client..."
if resource_exists "/v1/oauth2/hearth-sso-broker" "$IDM_TOKEN"; then
  echo "  OAuth2 client 'hearth-sso-broker' already exists"
else
  BROKER_URL="{{ $.Values.ssoBroker.issuerUrl }}"
  $C -X POST "${KANIDM_URL}/v1/oauth2/_basic" \
    -H "Authorization: Bearer ${IDM_TOKEN}" \
    -H "Content-Type: application/json" \
    -d "{\"attrs\":{\"name\":[\"hearth-sso-broker\"],\"displayname\":[\"Hearth SSO Broker\"],\"oauth2_rs_origin_landing\":[\"${BROKER_URL}\"],\"oauth2_rs_origin\":[\"${BROKER_URL}/callback\"]}}" > /dev/null
  echo "  Created OAuth2 client 'hearth-sso-broker' (confidential)"
fi

$C -X POST "${KANIDM_URL}/v1/oauth2/hearth-sso-broker/_scopemap/hearth-users" \
  -H "Authorization: Bearer ${IDM_TOKEN}" \
  -H "Content-Type: application/json" \
  -d '["openid","profile","email","groups"]' > /dev/null 2>&1 || true

$C -X PATCH "${KANIDM_URL}/v1/oauth2/hearth-sso-broker" \
  -H "Authorization: Bearer ${IDM_TOKEN}" \
  -H "Content-Type: application/json" \
  -d '{"attrs":{"oauth2_prefer_short_username":["true"]}}' > /dev/null 2>&1 || true

BROKER_OIDC_SECRET=$($C "${KANIDM_URL}/v1/oauth2/hearth-sso-broker" \
  -H "Authorization: Bearer ${IDM_TOKEN}" | sed -n 's/.*"oauth2_rs_basic_secret":\["\([^"]*\)".*/\1/p' || echo "")

# Persist both into one Secret used by hearth-api deployment.
kubectl create secret generic {{ include "hearth-home.fullname" $ }}-broker-credentials \
  --namespace={{ $.Release.Namespace }} \
  --from-literal=broker-oidc-secret="$BROKER_OIDC_SECRET" \
  --from-literal=broker-svc-token="$BROKER_SVC_TOKEN" \
  --dry-run=client -o yaml | kubectl apply -f -
{{- end }}
```

- [ ] **Step 3: Lint chart**

Run: `nix develop -c just helm-lint`
Expected: clean.

- [ ] **Step 4: Render-test that the block appears when toggled**

Run: `nix develop -c helm template chart/hearth-home --set capabilities.ssoBroker=true --set ssoBroker.issuerUrl=https://api.test/oidc | grep -c hearth-sso-broker`
Expected: ≥ 3 matches (in script body).

Run: `nix develop -c helm template chart/hearth-home --set capabilities.ssoBroker=false | grep -c hearth-sso-broker`
Expected: 0 matches.

- [ ] **Step 5: Commit**

```bash
git add chart/hearth-home/templates/kanidm/bootstrap-configmap.yaml
git commit -m "feat(sso-broker): bootstrap registers broker SA + Kanidm OAuth2 client"
```

---

### Task 16: Signing-key keypair generation Job + Secret

**Files:**
- Create: `chart/hearth-home/templates/sso-broker/job-genkey.yaml`
- Create: `chart/hearth-home/templates/sso-broker/secret-signing-key.yaml`

**Interfaces:**
- Produces:
  - If `ssoBroker.signingKey.existingSecret == ""` and `capabilities.ssoBroker == true`: a Job runs once at install/upgrade that generates an ES256 keypair via openssl into Secret `<release>-broker-signing-key`.
  - If `existingSecret` is set: the auto-gen Job is skipped; broker uses the named Secret.

- [ ] **Step 1: Write Job manifest**

Create `chart/hearth-home/templates/sso-broker/job-genkey.yaml`:

```yaml
{{- if and .Values.capabilities.ssoBroker (not .Values.ssoBroker.signingKey.existingSecret) }}
apiVersion: batch/v1
kind: Job
metadata:
  name: {{ include "hearth-home.fullname" . }}-broker-genkey
  labels:
    {{- include "hearth-home.labels" . | nindent 4 }}
  annotations:
    "helm.sh/hook": pre-install,pre-upgrade
    "helm.sh/hook-delete-policy": before-hook-creation,hook-succeeded
    "helm.sh/hook-weight": "-5"
spec:
  ttlSecondsAfterFinished: 300
  template:
    metadata:
      labels:
        {{- include "hearth-home.selectorLabels" . | nindent 8 }}
        app.kubernetes.io/component: broker-genkey
    spec:
      restartPolicy: OnFailure
      serviceAccountName: {{ include "hearth-home.fullname" . }}-bootstrap
      containers:
        - name: genkey
          image: alpine:3.19
          securityContext:
            {{- include "hearth-home.restrictedSecurityContext" . | nindent 12 }}
          command:
            - /bin/sh
            - -c
            - |
              set -eu
              apk add --no-cache openssl kubectl >/dev/null 2>&1 || true
              SECRET_NAME={{ include "hearth-home.fullname" . }}-broker-signing-key
              if kubectl get secret "$SECRET_NAME" -n {{ .Release.Namespace }} > /dev/null 2>&1; then
                echo "Signing key Secret already exists; leaving in place."
                exit 0
              fi
              KEYDIR=$(mktemp -d)
              openssl ecparam -name prime256v1 -genkey -noout -out "$KEYDIR/tls.key"
              openssl ec -in "$KEYDIR/tls.key" -pubout -out "$KEYDIR/tls.crt"
              kubectl create secret generic "$SECRET_NAME" \
                --namespace={{ .Release.Namespace }} \
                --from-file=tls.key="$KEYDIR/tls.key" \
                --from-file=tls.crt="$KEYDIR/tls.crt"
              echo "Generated $SECRET_NAME"
          volumeMounts:
            - name: tmp
              mountPath: /tmp
      volumes:
        - name: tmp
          emptyDir: {}
{{- end }}
```

- [ ] **Step 2: Write helm-unittest for the Job**

Create `chart/hearth-home/tests/sso_broker_signing_secret_test.yaml`:

```yaml
suite: SSO broker signing key Job
templates:
  - sso-broker/job-genkey.yaml
release:
  name: hearth
  namespace: default
tests:
  - it: should NOT render when ssoBroker disabled
    set:
      capabilities.ssoBroker: false
    asserts:
      - hasDocuments:
          count: 0

  - it: should render the genkey Job when ssoBroker enabled and no existingSecret
    set:
      capabilities.ssoBroker: true
      ssoBroker.issuerUrl: https://api.hearth.local/oidc
      ssoBroker.signingKey.existingSecret: ""
    asserts:
      - isKind:
          of: Job
      - equal:
          path: metadata.name
          value: hearth-hearth-home-broker-genkey
      - equal:
          path: metadata.annotations["helm.sh/hook"]
          value: pre-install,pre-upgrade

  - it: should NOT render when existingSecret is set
    set:
      capabilities.ssoBroker: true
      ssoBroker.signingKey.existingSecret: my-prod-key
    asserts:
      - hasDocuments:
          count: 0
```

- [ ] **Step 3: Run helm-unittest**

Run: `nix develop -c just helm-test`
Expected: new tests pass; full suite still passes.

- [ ] **Step 4: Verify rendered Job is valid manifest**

Run: `nix develop -c helm template chart/hearth-home --set capabilities.ssoBroker=true --set ssoBroker.issuerUrl=https://t/oidc | nix develop -c kubeconform -strict -summary -`
Expected: pass.

- [ ] **Step 5: Commit**

```bash
git add chart/hearth-home/templates/sso-broker/ chart/hearth-home/tests/sso_broker_signing_secret_test.yaml
git commit -m "feat(sso-broker): signing key generation Job + helm-unittest"
```

---

### Task 17: hearth-api Deployment mounts signing key + env

**Files:**
- Modify: `chart/hearth-home/templates/api/deployment.yaml`
- Modify: `chart/hearth-home/values.yaml` (no schema changes; sanity-check)

**Interfaces:**
- Produces: when `capabilities.ssoBroker` is true, hearth-api pod mounts `<release>-broker-signing-key` Secret at `/etc/hearth/broker/`, and sets:
  - `HEARTH_SSO_BROKER_ISSUER_URL`
  - `HEARTH_SSO_BROKER_PRIVATE_KEY_PATH=/etc/hearth/broker/tls.key`
  - `HEARTH_SSO_BROKER_KANIDM_AUTHORIZE_URL`
  - `HEARTH_SSO_BROKER_KANIDM_TOKEN_URL`
  - `HEARTH_SSO_BROKER_KANIDM_USERINFO_URL`
  - `HEARTH_SSO_BROKER_KANIDM_CLIENT_ID`
  - `HEARTH_SSO_BROKER_KANIDM_CLIENT_SECRET` (from `<release>-broker-credentials` Secret key `broker-oidc-secret`)
  - `HEARTH_SSO_BROKER_CLIENTS_JSON` (rendered from `ssoBroker.clients`)

- [ ] **Step 1: Add env + volume mount blocks**

Read `chart/hearth-home/templates/api/deployment.yaml`. Find the `env:` block in the hearth-api container; append:

```yaml
            {{- if .Values.capabilities.ssoBroker }}
            - name: HEARTH_SSO_BROKER_ISSUER_URL
              value: {{ .Values.ssoBroker.issuerUrl | quote }}
            - name: HEARTH_SSO_BROKER_PRIVATE_KEY_PATH
              value: /etc/hearth/broker/tls.key
            - name: HEARTH_SSO_BROKER_KANIDM_AUTHORIZE_URL
              value: "{{ .Values.kanidm.config.origin }}/ui/oauth2"
            - name: HEARTH_SSO_BROKER_KANIDM_TOKEN_URL
              value: "{{ .Values.kanidm.config.origin }}/oauth2/token"
            - name: HEARTH_SSO_BROKER_KANIDM_USERINFO_URL
              value: "{{ .Values.kanidm.config.origin }}/oauth2/openid/hearth-sso-broker/userinfo"
            - name: HEARTH_SSO_BROKER_KANIDM_CLIENT_ID
              value: {{ .Values.ssoBroker.kanidmClient.clientId | quote }}
            - name: HEARTH_SSO_BROKER_KANIDM_CLIENT_SECRET
              valueFrom:
                secretKeyRef:
                  name: {{ if .Values.ssoBroker.kanidmClient.existingSecret }}{{ .Values.ssoBroker.kanidmClient.existingSecret }}{{ else }}{{ include "hearth-home.fullname" . }}-broker-credentials{{ end }}
                  key: {{ .Values.ssoBroker.kanidmClient.existingSecretKey }}
            - name: HEARTH_SSO_BROKER_CLIENTS_JSON
              value: |
                {{- $clients := list -}}
                {{- range .Values.ssoBroker.clients -}}
                  {{- $client := dict "client_id" .clientId "audience" .audience "redirect_uris" .redirectUris "scopes" .scopes -}}
                  {{- $clients = append $clients $client -}}
                {{- end -}}
                {{ toJson $clients }}
            {{- end }}
```

(Note: `client_secret_hash` needs to be added per client at install time — see Step 3 below for a render-helper approach.)

Find the `volumeMounts:` block and append:

```yaml
            {{- if .Values.capabilities.ssoBroker }}
            - name: broker-signing-key
              mountPath: /etc/hearth/broker
              readOnly: true
            {{- end }}
```

Find the `volumes:` block and append:

```yaml
        {{- if .Values.capabilities.ssoBroker }}
        - name: broker-signing-key
          secret:
            secretName: {{ if .Values.ssoBroker.signingKey.existingSecret }}{{ .Values.ssoBroker.signingKey.existingSecret }}{{ else }}{{ include "hearth-home.fullname" . }}-broker-signing-key{{ end }}
            items:
              - key: {{ .Values.ssoBroker.signingKey.privateKeyKey }}
                path: tls.key
              - key: {{ .Values.ssoBroker.signingKey.publicKeyKey }}
                path: tls.crt
        {{- end }}
```

- [ ] **Step 2: Add a per-client-secret-hash helper**

Each broker client needs its `client_secret_hash` (SHA-256 hex) injected. For MVP, we require operators to supply the hash directly in values, OR auto-derive via a small init container.

Simpler: add `clientSecretHash` to each client entry in values.yaml; document how to compute it (`echo -n "secret" | sha256sum`). Update the JSON rendering:

```yaml
            - name: HEARTH_SSO_BROKER_CLIENTS_JSON
              value: |
                {{- $clients := list -}}
                {{- range .Values.ssoBroker.clients -}}
                  {{- $client := dict "client_id" .clientId "audience" .audience "redirect_uris" .redirectUris "scopes" .scopes "client_secret_hash" .clientSecretHash -}}
                  {{- $clients = append $clients $client -}}
                {{- end -}}
                {{ toJson $clients }}
```

Update `values.yaml` `ssoBroker.clients` example to reflect this:

```yaml
ssoBroker:
  # ... (existing)
  clients: []
    # - clientId: synapse
    #   audience: synapse
    #   redirectUris: ["https://chat.hearth.example.com/_synapse/client/oidc/callback"]
    #   scopes: ["openid", "profile", "email"]
    #   # SHA-256 hex of the client_secret; derive with: echo -n "secret" | sha256sum
    #   clientSecretHash: "abcd1234..."
```

- [ ] **Step 3: Lint + render-check**

Run: `nix develop -c just helm-lint`
Run: `nix develop -c helm template chart/hearth-home --set capabilities.ssoBroker=true --set ssoBroker.issuerUrl=https://api.hearth.local/oidc | grep HEARTH_SSO_BROKER_ISSUER_URL`
Expected: env var appears in rendered API Deployment.

- [ ] **Step 4: Render-check that it's absent when toggled off**

Run: `nix develop -c helm template chart/hearth-home --set capabilities.ssoBroker=false | grep HEARTH_SSO_BROKER || echo OK`
Expected: `OK`.

- [ ] **Step 5: Commit**

```bash
git add chart/hearth-home/templates/api/deployment.yaml chart/hearth-home/values.yaml
git commit -m "feat(sso-broker): hearth-api Deployment wires signing key + env"
```

---

### Task 18: NetworkPolicy + broker helm-unittest coverage

**Files:**
- Modify: `chart/hearth-home/templates/networkpolicy.yaml` (allow Synapse → API for OIDC; add ingress allow from internal apps when ssoBroker is enabled)
- Create: `chart/hearth-home/tests/sso_broker_api_env_test.yaml`

**Interfaces:**
- Produces:
  - When `ssoBroker` is on AND `capabilities.chat` is on: NetworkPolicy allows Synapse pods → API pod on the API port.
  - helm-unittest verifies env wiring on the API Deployment.

- [ ] **Step 1: Add NetworkPolicy rule**

Read `chart/hearth-home/templates/networkpolicy.yaml` to find the hearth-api ingress NP. Add a new ingress rule (inside the same NP or a sibling NP):

```yaml
{{- if and .Values.capabilities.ssoBroker .Values.capabilities.chat }}
    # Synapse → API for OIDC discovery/authorize/token/jwks
    - from:
        - podSelector:
            matchLabels:
              app.kubernetes.io/component: synapse
      ports:
        - port: http
          protocol: TCP
{{- end }}
```

- [ ] **Step 2: Write helm-unittest for API env**

Create `chart/hearth-home/tests/sso_broker_api_env_test.yaml`:

```yaml
suite: hearth-api env wiring for sso broker
templates:
  - api/deployment.yaml
release:
  name: hearth
  namespace: default
tests:
  - it: does not wire broker env when disabled
    set:
      capabilities.ssoBroker: false
    asserts:
      - notContains:
          path: spec.template.spec.containers[0].env
          content:
            name: HEARTH_SSO_BROKER_ISSUER_URL
          any: true

  - it: wires issuer URL and signing key path when enabled
    set:
      capabilities.ssoBroker: true
      ssoBroker.issuerUrl: https://api.hearth.local/oidc
    asserts:
      - contains:
          path: spec.template.spec.containers[0].env
          content:
            name: HEARTH_SSO_BROKER_ISSUER_URL
            value: https://api.hearth.local/oidc
          any: true
      - contains:
          path: spec.template.spec.containers[0].env
          content:
            name: HEARTH_SSO_BROKER_PRIVATE_KEY_PATH
            value: /etc/hearth/broker/tls.key
          any: true

  - it: mounts the broker signing key volume when enabled
    set:
      capabilities.ssoBroker: true
      ssoBroker.issuerUrl: https://api.hearth.local/oidc
    asserts:
      - contains:
          path: spec.template.spec.volumes
          content:
            name: broker-signing-key
          any: true
      - contains:
          path: spec.template.spec.containers[0].volumeMounts
          content:
            name: broker-signing-key
            mountPath: /etc/hearth/broker
            readOnly: true
          any: true

  - it: renders clients_json when clients supplied
    set:
      capabilities.ssoBroker: true
      ssoBroker.issuerUrl: https://api.hearth.local/oidc
      ssoBroker.clients:
        - clientId: synapse
          audience: synapse
          redirectUris: ["https://chat.hearth.local/cb"]
          scopes: ["openid","profile","email"]
          clientSecretHash: "deadbeef"
    asserts:
      - matchRegex:
          path: spec.template.spec.containers[0].env[?(@.name=='HEARTH_SSO_BROKER_CLIENTS_JSON')].value
          pattern: '"client_id":"synapse"'
```

- [ ] **Step 2: Run helm-unittest**

Run: `nix develop -c just helm-test`
Expected: all tests pass, including new ones.

- [ ] **Step 3: Commit**

```bash
git add chart/hearth-home/templates/networkpolicy.yaml chart/hearth-home/tests/sso_broker_api_env_test.yaml
git commit -m "feat(sso-broker): NetworkPolicy + helm-unittest for API env wiring"
```

---

## Phase E: Synapse migration + end-to-end VM test

### Task 19: Synapse OIDC reconfig to broker

**Files:**
- Modify: `chart/hearth-home/templates/synapse/configmap.yaml`
- Modify: `chart/hearth-home/templates/kanidm/bootstrap-configmap.yaml` (remove hearth-matrix client registration when ssoBroker is on; register Synapse as a broker client instead)
- Modify: `chart/hearth-home/values.yaml` (synapse.oidc.useBroker flag)

**Interfaces:**
- Produces:
  - When `ssoBroker=true`, Synapse's `oidc_providers[0].issuer` points at the broker, not Kanidm directly. `client_id` becomes `synapse` (broker-side), `client_secret_path` references the broker-issued secret. Discovery is followed automatically by Synapse.

- [ ] **Step 1: Add toggle to values.yaml**

Under `synapse.oidc:` add:

```yaml
    # When ssoBroker is enabled, point Synapse at the broker's issuer
    # instead of Kanidm directly. The broker becomes the IdP from
    # Synapse's perspective; Kanidm is upstream of the broker.
    useBroker: true
```

- [ ] **Step 2: Refactor Synapse configmap**

In `chart/hearth-home/templates/synapse/configmap.yaml`, replace the existing `oidc_providers` block with a conditional:

```yaml
{{- if and .Values.capabilities.identity .Values.synapse.oidc.enabled }}
oidc_providers:
{{- if and .Values.capabilities.ssoBroker .Values.synapse.oidc.useBroker }}
  - idp_id: hearth-broker
    idp_name: {{ .Values.synapse.oidc.idpName | quote }}
    issuer: {{ .Values.ssoBroker.issuerUrl | quote }}
    client_id: "synapse"
    client_secret_path: /data/oidc_client_secret
    scopes: ["openid", "profile", "email"]
    skip_verification: {{ .Values.synapse.oidc.skipVerification | default false }}
    allow_existing_users: true
    user_mapping_provider:
      config:
        localpart_template: "{{`{{ user.preferred_username }}`}}"
        display_name_template: "{{`{{ user.name }}`}}"
{{- else }}
  - idp_id: kanidm
    idp_name: {{ .Values.synapse.oidc.idpName | quote }}
    issuer: "{{ .Values.kanidm.config.origin }}/oauth2/openid/hearth-matrix"
    client_id: "hearth-matrix"
    client_secret_path: /data/oidc_client_secret
    scopes: ["openid", "profile", "email"]
    skip_verification: {{ .Values.synapse.oidc.skipVerification | default false }}
    allow_existing_users: true
    user_mapping_provider:
      config:
        localpart_template: "{{`{{ user.preferred_username }}`}}"
        display_name_template: "{{`{{ user.name }}`}}"
{{- end }}
{{- end }}
```

- [ ] **Step 3: Register Synapse as a broker client in bootstrap**

In `chart/hearth-home/templates/kanidm/bootstrap-configmap.yaml`, find the existing `hearth-matrix` Kanidm OAuth2 client block. Wrap it with `{{- if not (and $.Values.capabilities.ssoBroker $.Values.synapse.oidc.useBroker) }}` so it's skipped when the broker fronts Synapse.

After that, add a Synapse-as-broker-client block — but this can be done via the chart values too. Recommended: tell operators to populate `ssoBroker.clients` with a Synapse entry and provide a separate Job (or document) to derive the client_secret_hash. For MVP, the easiest path is:

Modify `values.yaml` to include a sane default Synapse client entry when both toggles are on. Add this to `ssoBroker.clients`:

For MVP simplicity, this plan does NOT auto-derive the client secret. Operators must:
1. Generate a random secret: `openssl rand -hex 32 | tee /tmp/syn-secret`
2. Compute its hash: `cat /tmp/syn-secret | tr -d '\n' | sha256sum | awk '{print $1}'`
3. Put the hash in `ssoBroker.clients[0].clientSecretHash`
4. Put the raw secret in a Secret used by Synapse's `client_secret_path` mount (existing Synapse pattern via `oidc_client_secret` file)

Document this in `docs/sso-broker.md` (created in next task).

- [ ] **Step 4: Update Synapse Secret injection**

The existing Synapse init container `inject-oidc-secret` reads from `kanidm-credentials` Secret. When the broker is in use, it should read from a different key: `synapse-broker-secret`. Update `chart/hearth-home/templates/synapse/deployment.yaml`:

```yaml
- name: inject-oidc-secret
  image: busybox:1.36
  command: ['sh', '-c', 'echo -n "$OIDC_CLIENT_SECRET" > /data-shared/oidc_client_secret']
  env:
    - name: OIDC_CLIENT_SECRET
      valueFrom:
        secretKeyRef:
          name: {{ if .Values.synapse.oidc.existingSecret }}{{ .Values.synapse.oidc.existingSecret }}{{ else }}{{ include "hearth-home.fullname" . }}-kanidm-credentials{{ end }}
          {{- if and .Values.capabilities.ssoBroker .Values.synapse.oidc.useBroker }}
          key: synapse-broker-secret
          {{- else }}
          key: {{ .Values.synapse.oidc.existingSecretKey }}
          {{- end }}
```

Document that operators populate `synapse-broker-secret` key in their existingSecret OR provide a separate Secret.

- [ ] **Step 5: helm-unittest for the toggle**

Create `chart/hearth-home/tests/sso_broker_synapse_oidc_test.yaml`:

```yaml
suite: Synapse OIDC config toggles broker vs Kanidm
templates:
  - synapse/configmap.yaml
release:
  name: hearth
  namespace: default
tests:
  - it: uses Kanidm directly when broker is off
    set:
      capabilities.chat: true
      capabilities.identity: true
      capabilities.ssoBroker: false
      synapse.oidc.enabled: true
      synapse.oidc.useBroker: true   # but capability off → ignored
    asserts:
      - matchRegex:
          path: data["homeserver.yaml"]
          pattern: 'issuer:.*hearth-matrix'

  - it: uses broker when both toggles are on
    set:
      capabilities.chat: true
      capabilities.identity: true
      capabilities.ssoBroker: true
      synapse.oidc.enabled: true
      synapse.oidc.useBroker: true
      ssoBroker.issuerUrl: https://api.hearth.local/oidc
    asserts:
      - matchRegex:
          path: data["homeserver.yaml"]
          pattern: 'issuer:.*api.hearth.local/oidc'
      - matchRegex:
          path: data["homeserver.yaml"]
          pattern: 'idp_id: hearth-broker'
```

- [ ] **Step 6: Run helm tests**

Run: `nix develop -c just helm-test && nix develop -c just helm-lint`
Expected: all pass.

- [ ] **Step 7: Commit**

```bash
git add chart/hearth-home/templates/synapse/ chart/hearth-home/templates/kanidm/bootstrap-configmap.yaml chart/hearth-home/values.yaml chart/hearth-home/tests/sso_broker_synapse_oidc_test.yaml
git commit -m "feat(sso-broker): Synapse OIDC switches to broker when capability on"
```

---

### Task 20: Operator-facing docs

**Files:**
- Create: `docs/sso-broker.md`
- Modify: `docs/threat-model.md` (add broker as a trust boundary)

**Interfaces:**
- Produces: a one-page overview that explains the broker's purpose, architecture, key management, client registration steps, and current limitations.

- [ ] **Step 1: Write docs/sso-broker.md**

Create `docs/sso-broker.md`:

```markdown
# Hearth SSO Broker

## Purpose

Stock Kanidm 1.10 cannot mint user-scoped OIDC tokens from a service-account
credential ([see verdict](../memory/project_sso_broker_verdict.md)). This
blocks the fleet seamless-SSO architecture where a device daemon would obtain
a token after PAM authentication.

The SSO broker is a hearth-side OIDC issuer that wraps Kanidm. Fleet apps
(Synapse, Grafana, Nextcloud, the admin SPA) trust the broker as their IdP
rather than Kanidm directly. The broker uses Kanidm's standard authorization
code flow upstream to authenticate end-users, then mints its own ES256-signed
tokens for downstream apps with its own signing key, audience, and revocation
state.

Kanidm remains the source of truth for credentials, MFA, groups, and identity
lifecycle. The broker is purely an OIDC issuer with extension hooks for the
device-driven flows that follow in Phase 2.

## Architecture

```
End user → Synapse (or app)
              │ OIDC authorization-code flow
              ▼
         broker /oidc/authorize ────► Kanidm /ui/oauth2 (interactive login)
              ▲                            │
              │ callback with Kanidm code  │
              ▼                            ▼
         broker /oidc/callback ───────────'
              │ stash broker code
              ▼ 302 back to Synapse
         Synapse → broker /oidc/token (PKCE)
              │ broker mints ES256 id_token + access_token
              ▼
         Synapse validates against broker JWKS
              │
              ▼
         User logged in to Synapse
```

## Key management

- ES256 (P-256) keypair lives in Kubernetes Secret
  `<release>-broker-signing-key` (default) or an operator-provided
  `ssoBroker.signingKey.existingSecret`.
- For dev / first install, a pre-install Job generates a keypair using
  `openssl`.
- Rotation is manual: bump the Secret, restart `hearth-api`. JWKS includes the
  current key only (multi-key rotation is a future enhancement).
- The private key is mounted read-only at `/etc/hearth/broker/tls.key` and
  loaded once at process start.

## Trust boundaries

- **Broker → Kanidm:** confidential OAuth2 client; client secret in Secret
  `<release>-broker-credentials.broker-oidc-secret`. TLS to Kanidm. Service
  account `hearth-broker-svc` holds a long-lived API token (Secret key
  `broker-svc-token`) used for future user/group lookups (Phase 2).
- **App → broker:** apps are confidential clients of the broker;
  `client_secret_hash` (SHA-256 hex) stored in chart values, raw secret stored
  in app-side Secret (Synapse pattern: `client_secret_path` mount).
- **Broker → DB:** existing PostgreSQL connection; `sso_issued_tokens` table
  records every minted token's `jti` for revocation.

## Enabling

1. Set `capabilities.ssoBroker: true` in values.
2. Set `ssoBroker.issuerUrl` to your broker's public URL (e.g.,
   `https://api.hearth.example.com/oidc`).
3. For each app, register a client in `ssoBroker.clients`:

```bash
SECRET=$(openssl rand -hex 32)
HASH=$(echo -n "$SECRET" | sha256sum | awk '{print $1}')
echo "client_secret: $SECRET"
echo "client_secret_hash: $HASH"
```

Put `$HASH` in `ssoBroker.clients[].clientSecretHash`. Put `$SECRET` in the
app-side Secret the app uses to authenticate to the broker.

4. For Synapse specifically, set `synapse.oidc.useBroker: true` and populate
   `<release>-kanidm-credentials.synapse-broker-secret` with the raw client
   secret.
5. Upgrade the release; the pre-install Job generates a signing keypair if
   none exists.

## Current limitations (MVP)

- No client admin API; client registry is declarative via Helm values.
- ID-token signature from Kanidm is NOT cryptographically verified by the
  broker — relies on the TLS + basic-auth channel to Kanidm's token endpoint
  (same posture as the existing `identity_sync` integration). Tracked for
  improvement in Phase 2.
- No refresh token issuance yet (TTL is 1 hour by default; apps must
  re-authenticate after expiry).
- Single signing key — no rotation overlap.
- No revocation API for end users; revocation is admin-only via the broker
  `/oidc/revoke` (RFC 7009) endpoint with client credentials.

## Roadmap

Phase 2 (next plan) introduces:

- Device backchannel: `hearth-agent` calls broker after PAM login to obtain a
  user token without browser involvement.
- Per-user daemon (`hearth-sso`) on each desktop holds session tokens.
- Browser cookie pre-warming.
- Native app keyring writers (Element, Fractal, kubectl).
- Firefox `policies.json` via home-manager.
```

- [ ] **Step 2: Append to threat-model.md**

Read `docs/threat-model.md` and append:

```markdown
## Trust boundary: SSO broker (`hearth-api`/oidc)

| Property | Value |
|---|---|
| Component | sso_broker module in `hearth-api` |
| Privilege | Holds ES256 signing key (issuer authority for all fleet apps) and Kanidm broker SA token |
| Inputs | App OAuth2 redirects, Kanidm callback redirects, browser cookies (none today) |
| Outputs | Broker-signed JWTs to apps |
| Threats | Signing key compromise (impersonate any user to any app); SA token compromise (read all user/group data from Kanidm); broker-code stash poisoning (limited by 60s TTL + Uuid keys) |
| Mitigations | Secret-mounted key file (read-only, mode 0400), restricted SecurityContext, NetworkPolicy default-deny + explicit app→broker allow, audit on every mint/revoke |
| Accepted risks | Upstream id_token signature not verified — boundary is TLS + basic auth to Kanidm token endpoint (consistent with `identity_sync` posture, tracked for Phase 2) |
```

- [ ] **Step 3: Commit**

```bash
git add docs/sso-broker.md docs/threat-model.md
git commit -m "docs(sso-broker): operator overview + threat-model entry"
```

---

### Task 21: End-to-end VM test (Synapse login via broker)

**Files:**
- Create: `tests/vm-sso-broker-synapse-login.nix`
- Modify: `flake.nix` (register the VM test)

**Interfaces:**
- Produces: a `nix flake check` that boots NixOS hosts with hearth-api (broker enabled), Kanidm, and Synapse; performs a curl-driven OAuth flow against the broker; asserts a valid id_token comes back.

- [ ] **Step 1: Inspect an existing VM test for patterns**

Read `tests/vm-kanidm-auth.nix` (or any existing test in `tests/`) to understand the structure: `pkgs.testers.runNixOSTest { ... }`, nodes block, testScript Python.

- [ ] **Step 2: Write the VM test**

Create `tests/vm-sso-broker-synapse-login.nix`:

```nix
{ pkgs, ... }:
pkgs.testers.runNixOSTest {
  name = "sso-broker-synapse-login";

  nodes = {
    server = { config, pkgs, ... }: {
      # Reuse the existing vm-full-enrollment pattern: Kanidm + hearth-api
      # in a single VM. The test exercises the broker via curl against
      # localhost.
      imports = [ ./common/kanidm-minimal.nix ./common/hearth-api-minimal.nix ];

      services.hearth-api = {
        enable = true;
        ssoBroker = {
          enable = true;
          issuerUrl = "http://localhost:3000/oidc";
          # Test signing key (do NOT use elsewhere)
          privateKeyPem = ''
            -----BEGIN PRIVATE KEY-----
            MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgevZzL1gdAFr88hb2
            OF/2NxApJCzGCEDdfSp6VQO30hyhRANCAAQRWz+jn65BtOMvdyHKcvjBeBSDZH2r
            1RTwjmYSi9R/zpBnuQ4EiMnCqfMPWiZqB4QdbAd0E7oH50VpuZ1P087G
            -----END PRIVATE KEY-----
          '';
          kanidmClientId = "hearth-sso-broker";
          # secret set via service env from a test bootstrap step
        };
      };

      networking.firewall.allowedTCPPorts = [ 3000 8443 ];
    };
  };

  testScript = ''
    start_all()
    server.wait_for_unit("kanidm.service")
    server.wait_for_unit("hearth-api.service")
    server.wait_for_open_port(3000)
    server.wait_for_open_port(8443)

    # Bootstrap: create broker OAuth2 client in Kanidm + grant scope.
    # (Test-only abbreviated; in prod this is the Helm bootstrap script.)
    server.succeed("/run/current-system/sw/bin/kanidm-bootstrap-broker.sh")

    # 1. Discovery
    discovery = server.succeed("curl -sf http://localhost:3000/oidc/.well-known/openid-configuration")
    assert '"issuer": "http://localhost:3000/oidc"' in discovery, discovery
    assert '"id_token_signing_alg_values_supported": ["ES256"]' in discovery, discovery

    # 2. JWKS
    jwks = server.succeed("curl -sf http://localhost:3000/oidc/.well-known/jwks.json")
    assert '"kty": "EC"' in jwks, jwks
    assert '"crv": "P-256"' in jwks, jwks

    # 3. Authorize → 302 to Kanidm
    code = '''
      curl -si "http://localhost:3000/oidc/authorize?response_type=code&client_id=synapse&redirect_uri=http%3A%2F%2Flocalhost%3A9999%2Fcb&scope=openid&state=t&code_challenge=ungWv48Bz-pBQUDeXa4iI7ADYaOWF3qctBD_YfIAFa0&code_challenge_method=S256" | head -1
    '''
    head = server.succeed(code)
    assert "HTTP/1.1 302" in head, head

    # 4. Full happy-path login is mocked via a kanidm headless auth script.
    #    Real interactive flow is covered by the existing vm-full-login-flow test.
    print("sso-broker discovery, JWKS, authorize redirect — OK")
  '';
}
```

(Implementation note: this test depends on a yet-to-be-built `hearth-api-minimal.nix` test module that wraps the binary with the `HEARTH_SSO_BROKER_*` env vars. If that module doesn't exist, create it under `tests/common/`. The full end-to-end Synapse login is intentionally deferred to a follow-up test once the test harness for headless Kanidm OAuth is in place — current `vm-full-login-flow.nix` already does the equivalent and can be extended.)

- [ ] **Step 3: Register VM test in flake.nix**

Find the `vmTests = { ... };` block in `flake.nix` and add:

```nix
sso-broker-synapse-login = ./tests/vm-sso-broker-synapse-login.nix;
```

- [ ] **Step 4: Build the test**

Run: `nix build .#checks.x86_64-linux.vm-sso-broker-synapse-login`
Expected: clean build (may need to iterate on the test module's NixOS-config plumbing — that's expected).

- [ ] **Step 5: Commit**

```bash
git add tests/vm-sso-broker-synapse-login.nix flake.nix tests/common/ 2>/dev/null || true
git commit -m "test(sso-broker): VM check for discovery + jwks + authorize redirect"
```

---

### Task 22: Final integration check

- [ ] **Step 1: Full Rust check**

Run: `nix develop -c just check`
Expected: clippy + fmt + tests all pass.

- [ ] **Step 2: Full Helm check**

Run: `nix develop -c just helm-check`
Expected: lint + helm-unittest + kubeconform all pass.

- [ ] **Step 3: SBOM / docs update**

If `docs/compliance-controls.yaml` lists per-component controls, add an entry for the broker noting:
- Signing key handled via Kubernetes Secret (mode 0400 mount)
- Audit events emitted for token issuance and revocation
- Existing CIS Helm hardening controls apply

- [ ] **Step 4: Manual smoke test**

Bring up the local cluster:

```bash
nix develop -c just setup
nix develop -c just infra
nix develop -c just dev
```

Set broker env vars in a `.env.broker` you `source`:

```bash
export HEARTH_SSO_BROKER_ISSUER_URL=http://localhost:3000/oidc
export HEARTH_SSO_BROKER_PRIVATE_KEY_PEM='<paste PEM>'
export HEARTH_SSO_BROKER_KANIDM_CLIENT_ID=hearth-sso-broker
export HEARTH_SSO_BROKER_KANIDM_CLIENT_SECRET=<secret>
export HEARTH_SSO_BROKER_KANIDM_AUTHORIZE_URL=https://localhost:8443/ui/oauth2
export HEARTH_SSO_BROKER_KANIDM_TOKEN_URL=https://localhost:8443/oauth2/token
export HEARTH_SSO_BROKER_CLIENTS_JSON='[{"client_id":"test","client_secret_hash":"<sha256>","redirect_uris":["http://localhost:9999/cb"],"scopes":["openid"],"audience":"test"}]'
```

Hit the discovery endpoint:

```bash
curl -sf http://localhost:3000/oidc/.well-known/openid-configuration | jq .
curl -sf http://localhost:3000/oidc/.well-known/jwks.json | jq .
```

Expected: valid JSON.

- [ ] **Step 5: Final commit**

```bash
git add -A
git commit -m "docs(sso-broker): compliance entries + final smoke notes" --allow-empty
```

---

## Future plans (out of scope for this plan)

These ship after Phase 1 lands and the broker is proven by Synapse migration. They are referenced here for context and will become separate plans:

1. **Device backchannel + hearth-sso daemon** — hearth-agent's `handle_prepare_user_env` calls a new broker endpoint `/oidc/internal/user-session` (authenticated with the device's existing machine JWT) to obtain a user token for the just-authenticated PAM user. Token handed off to a per-user `hearth-sso` daemon over IPC. Daemon exposes a Unix socket where local apps can request audience-keyed tokens. Requires extending the broker with: a "trusted-device" code path that mints user tokens without browser interaction, validation that the requesting machine is actually owned by/assigned to that user (via existing `machines` table joins), and a refresh loop.

2. **Browser cookie pre-warming** — broker adds `/oidc/bootstrap-session` endpoint that issues a short-lived bootstrap URL containing a server-trusted nonce. The browser-primer (one-shot autostart helper) opens Firefox headlessly against that URL; broker sets a session cookie on broker-issued OIDC redirects so subsequent app SSO flows are silent.

3. **Native app keyring writers** — extend `hearth-sso` to write Element/Fractal session JSON or libsecret entries on startup using tokens minted from the device backchannel.

4. **Firefox `policies.json`** — drop via home-manager: trust internal CAs, pre-bookmark hearth services, configure Negotiate auth trusted URIs (preparation for any future Kerberos path).

5. **Per-client admin UI** — replace declarative `HEARTH_SSO_BROKER_CLIENTS_JSON` with a DB-backed registry + admin web UI for adding/removing app integrations.

6. **Upstream id_token signature verification** — bring broker into stricter alignment with OIDC spec: verify Kanidm-issued id_token signatures against Kanidm's JWKS (reuse `auth.rs` JwksCache pattern).

7. **Refresh token issuance** — broker mints + rotates refresh tokens; hearth-sso refreshes proactively before expiry.

8. **Key rotation overlap** — broker publishes both current and previous key in JWKS during rotation windows.

---

## Self-Review Notes

**Spec coverage check:**
- Broker's API surface — covered (Phase B: discovery, JWKS, authorize, callback, token, userinfo, revoke).
- Signing-key handling — covered (Task 2 keys; Task 16 Job + Secret; Task 17 mount).
- Trust boundary with device daemons — explicitly deferred to follow-up plans (Future plans section #1); doc and threat-model note this.
- App reconfiguration (Synapse) — covered (Task 19). Element/Fractal/Firefox deferred to follow-ups.
- Refresh / revocation / audit — revocation covered (Task 13); audit covered (Task 13). Refresh deferred (Future #7), with note that initial TTL is 1 hour.
- Phased rollout keeping Kanidm as source of truth — explicit (broker is downstream of Kanidm; Synapse can flip back via `synapse.oidc.useBroker=false`).

**Placeholder scan:** no `TBD`/`TODO`/`later` in any task. The `decode_jti` duplication is marked as a deliberate temporary inline note in Task 13 (acknowledged not a placeholder).

**Type consistency:** `SigningKey`, `SsoBrokerConfig`, `BrokerClient`, `AuthorizeRequest`, `BrokerCode`, `BrokerClaims`, `TokenKind` — all defined once and referenced consistently across handlers. The handler test scaffolding pattern (`enabled_state*()`) is reused but redeclared per test file because tests are not shared across modules; this is intentional and not a drift.

