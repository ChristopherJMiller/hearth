use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use jsonwebtoken::jwk::JwkSet;
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use serde::de::DeserializeOwned;
use sqlx::PgPool;
use std::sync::{Arc, OnceLock};
use std::time::Instant;
use tokio::sync::RwLock;
use tower::ServiceExt;
use uuid::Uuid;

use hearth_api::AppState;
use hearth_api::auth::{AuthConfig, JwksCache};

/// Per-test isolated database. Creates a fresh DB with migrations.
/// Call `cleanup()` to drop the database, or let it leak (cleaned up next run).
pub struct TestDb {
    pub pool: PgPool,
    #[allow(dead_code)]
    db_name: String,
    #[allow(dead_code)]
    admin_pool: PgPool,
}

impl TestDb {
    pub async fn new() -> Self {
        let admin_url = "postgres://hearth:hearth@localhost:5432/hearth";
        let admin_pool = PgPool::connect(admin_url)
            .await
            .expect("failed to connect to admin database — is PostgreSQL running?");

        let db_name = format!("hearth_test_{}", Uuid::new_v4().simple());

        sqlx::query(&format!("CREATE DATABASE \"{db_name}\""))
            .execute(&admin_pool)
            .await
            .expect("failed to create test database");

        let test_url = format!("postgres://hearth:hearth@localhost:5432/{db_name}");
        let pool = PgPool::connect(&test_url)
            .await
            .expect("failed to connect to test database");

        sqlx::migrate!("../../migrations")
            .run(&pool)
            .await
            .expect("failed to run migrations on test database");

        Self {
            pool,
            db_name,
            admin_pool,
        }
    }
}

impl TestDb {
    /// Clean up the test database. Optional — if not called, the DB leaks
    /// (harmless for local dev, cleaned up by re-running tests or manually).
    #[allow(dead_code)]
    pub async fn cleanup(self) {
        self.pool.close().await;
        // Terminate lingering connections
        let _ = sqlx::query(&format!(
            "SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname = '{}'",
            self.db_name
        ))
        .execute(&self.admin_pool)
        .await;
        let _ = sqlx::query(&format!("DROP DATABASE IF EXISTS \"{}\"", self.db_name))
            .execute(&self.admin_pool)
            .await;
    }
}

/// Returns a shared PrometheusHandle. Safe to call from multiple tests.
fn metrics_handle() -> PrometheusHandle {
    static HANDLE: OnceLock<PrometheusHandle> = OnceLock::new();
    HANDLE
        .get_or_init(|| {
            PrometheusBuilder::new()
                .install_recorder()
                .expect("failed to install metrics recorder")
        })
        .clone()
}

/// Build a test app with auth disabled (dev mode).
pub async fn test_app() -> (Router, TestDb) {
    let db = TestDb::new().await;
    let state = AppState {
        pool: db.pool.clone(),
        auth_config: AuthConfig {
            oidc_issuers: vec![],
            oidc_audiences: vec![],
            machine_token_secret: Some(b"test-secret-key-for-machine-tokens".to_vec()),
            jwks_cache: Default::default(),
        },
    };
    let router = hearth_api::build_router(state, "/nonexistent", metrics_handle());
    (router, db)
}

// ---------------------------------------------------------------------------
// Auth-enabled test app
// ---------------------------------------------------------------------------

/// Test RSA private key (PKCS#8 PEM) for signing JWTs in tests.
const TEST_RSA_PRIVATE_PEM: &[u8] = br#"-----BEGIN PRIVATE KEY-----
MIIEvgIBADANBgkqhkiG9w0BAQEFAASCBKgwggSkAgEAAoIBAQCYLlsz69MaYGLI
5T87UyO9sJpsU3BcOE1PSG+aRjbjSJKEeQInIFdXDubw4EH2Q9LV8n+Rbwj8vC+Z
kaqiJd/wUhW3fWYaRTW5HAmCBorI/Z3GIvmnpKAc8UC2CZtwHDuI3cICXSLg4yWI
En2MpmwzYml7cOiEYyWbePXswps1a5MvJoCkYxfUXHBgRKYSmTxFQ6C99P0iaxWh
Uf2V1yheMKQxD9KINSVRgd/5/w10jPPaodlMC0VfnTlAA1Z5QyC9QcMAv9g5XGHU
qTAFIhNXFZDW/6A21OtFKLGkRp/tmEWRAe9RNzvW9KM4kytNBJ+m/mdqres6Q94O
E3mMx1avAgMBAAECggEAKbP+Y0abVa2XNJV6FABPGZL1Zn0hG+yD5xejGcRnEzbW
i/37RIyc0qsvR1A1U063ztCC+/BRJq1NYAimmYPGj/12nQ98tPNFayJPSrYPP1Ac
9eiswK+g/v1v7dLJKVpbSED5AxvRzI4CwXzLsgkDOruby2ugFHN1IVtvDUAxN1yE
tI9PoSaUrPbYzQ9qV6S6lWw6fv5A+YCEbhX/YckOmdNq/a5v2JFmo6vAhwwDqxfs
zVLCKkSX7YcLMC/vfmEzqWYzEEreyBB0SbgLz9zFYaLy7vWPYOfQufrB32O9lXfL
omdx5rLLbBGDxIx/L/AjwJXy6FfAVh54wzX02RtPLQKBgQDN9k1C8ChsAZq9kU4X
sTQvZZnsRArHNwicI1NQU7RY8CkTl5Y7MB92KkgKs9o/esE2vIAI0lHsmV7SkORB
a5ts7G/bPijWjx3y9k5IQ78TZc8vosfg5cmjxbBSXRPXgQarx/6TjeoE9mMmhknl
f8ERHtiDoFUsjNixe+3+j9tNdQKBgQC9JyxMULIRUC4KpQ1qERlp7G4oqOo0NmI9
h66BBB+gRWljNmFQ0/juKc63ZIpbMtq+LRV/si58W8dhvd2fupev5sYGXw9pVmHr
1zOprBF4B3MsPlL7g1Dx/UWOx0ktQYlpAREm+iniJP84D6QVWjZ7nPh9buyqjV0M
vt27CBhbEwKBgQCY/f7CXHcKU97INimWO9GR65z8/kYrWVwR78OxhZwP/MXmgdHc
wZB9TEcbfCIAyYTAziElbGXipMLlEzAa0H4x2Sf16iSXzNPoaMIZuAk1tYjDz909
2DOCbhTd+boFeRilffhDT0WozgU01sgJrG6T8x/OFsluQGmu3WoakG3NCQKBgFBf
P8RhmXgf0KB3R37lKx5F9vzR4Uo0PYQVjgGGBgYs2D3u0mTs3N4d55cnYl6j+ixK
rdLnnDb9LHgMnAoN1/xHG6eDZuIEcXErrOkQkw+kYrzO8qWqQ4+/ZXnoleBj84Yp
jOc57ugHfmaMxtTH01Ss+y0ZH/jMPlh3FXIuW2L/AoGBAI5BlTpmnW6oRig10Z35
cfelbzD0A91N3ERNMvUhWEqlYwl6bDCmxZFHWASPzXs+Hye3gHPNh+hpTp/OHOk8
EQbooKQebY/q9fxqCl+8HNSDFy64pwCPvMiOEBC/qI+Ff3txuql0w2ztG+/6doJn
74oLFO/dJ8QB5WTlLtPBHog1
-----END PRIVATE KEY-----"#;

/// JWK Set JSON matching the test RSA public key above.
const TEST_JWKS_JSON: &str = r#"{"keys":[{"kty":"RSA","alg":"RS256","use":"sig","kid":"test-key-1","n":"mC5bM-vTGmBiyOU_O1MjvbCabFNwXDhNT0hvmkY240iShHkCJyBXVw7m8OBB9kPS1fJ_kW8I_LwvmZGqoiXf8FIVt31mGkU1uRwJggaKyP2dxiL5p6SgHPFAtgmbcBw7iN3CAl0i4OMliBJ9jKZsM2Jpe3DohGMlm3j17MKbNWuTLyaApGMX1FxwYESmEpk8RUOgvfT9ImsVoVH9ldcoXjCkMQ_SiDUlUYHf-f8NdIzz2qHZTAtFX505QANWeUMgvUHDAL_YOVxh1KkwBSITVxWQ1v-gNtTrRSixpEaf7ZhFkQHvUTc71vSjOJMrTQSfpv5naq3rOkPeDhN5jMdWrw","e":"AQAB"}]}"#;

pub const TEST_MACHINE_SECRET: &[u8] = b"test-secret-key-for-machine-tokens";

/// Returned by `test_app_with_auth` so tests can mint their own JWTs.
pub struct AuthTestContext {
    pub router: Router,
    #[allow(dead_code)]
    pub db: TestDb,
    pub encoding_key: jsonwebtoken::EncodingKey,
    pub machine_secret: Vec<u8>,
}

impl AuthTestContext {
    /// Mint an RS256 user JWT with the given claims.
    pub fn mint_user_jwt(&self, sub: &str, username: &str, groups: &[&str]) -> String {
        self.mint_user_jwt_with_exp(
            sub,
            username,
            groups,
            jsonwebtoken::get_current_timestamp() + 3600,
        )
    }

    /// Mint an RS256 user JWT with a custom expiration.
    pub fn mint_user_jwt_with_exp(
        &self,
        sub: &str,
        username: &str,
        groups: &[&str],
        exp: u64,
    ) -> String {
        let claims = serde_json::json!({
            "sub": sub,
            "preferred_username": username,
            "groups": groups,
            "exp": exp,
            "iat": jsonwebtoken::get_current_timestamp(),
        });
        let mut header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256);
        header.kid = Some("test-key-1".to_string());
        jsonwebtoken::encode(&header, &claims, &self.encoding_key).unwrap()
    }

    /// Mint an HS256 machine token.
    pub fn mint_machine_jwt(&self, machine_id: Uuid) -> String {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let claims = serde_json::json!({
            "sub": format!("machine:{machine_id}"),
            "machine_id": machine_id.to_string(),
            "exp": now + 3600,
            "iat": now,
        });
        let key = jsonwebtoken::EncodingKey::from_secret(&self.machine_secret);
        jsonwebtoken::encode(&jsonwebtoken::Header::default(), &claims, &key).unwrap()
    }
}

/// Build a test app with auth ENABLED (RS256 user tokens + HS256 machine tokens).
pub async fn test_app_with_auth() -> AuthTestContext {
    let db = TestDb::new().await;

    let jwks: JwkSet = serde_json::from_str(TEST_JWKS_JSON).unwrap();
    let jwks_cache = Arc::new(RwLock::new(JwksCache {
        keyset: jwks,
        fetched_at: Some(Instant::now()),
    }));

    let machine_secret = TEST_MACHINE_SECRET.to_vec();

    let state = AppState {
        pool: db.pool.clone(),
        auth_config: AuthConfig {
            oidc_issuers: vec!["https://test-idp".to_string()],
            oidc_audiences: vec![],
            machine_token_secret: Some(machine_secret.clone()),
            jwks_cache,
        },
    };

    let router = hearth_api::build_router(state, "/nonexistent", metrics_handle());
    let encoding_key = jsonwebtoken::EncodingKey::from_rsa_pem(TEST_RSA_PRIVATE_PEM).unwrap();

    AuthTestContext {
        router,
        db,
        encoding_key,
        machine_secret,
    }
}

// ---------------------------------------------------------------------------
// Request helpers
// ---------------------------------------------------------------------------

/// Send a request with an optional Bearer token and parse the JSON response body.
pub async fn send_json<T: DeserializeOwned>(
    app: &Router,
    method: &str,
    uri: &str,
    body: Option<serde_json::Value>,
    token: Option<&str>,
) -> (StatusCode, T) {
    let mut builder = Request::builder().method(method).uri(uri);

    if let Some(t) = token {
        builder = builder.header("authorization", format!("Bearer {t}"));
    }

    let req_body = if let Some(json) = body {
        builder = builder.header("content-type", "application/json");
        Body::from(serde_json::to_vec(&json).unwrap())
    } else {
        Body::empty()
    };

    let req = builder.body(req_body).unwrap();
    let response: axum::response::Response = app.clone().oneshot(req).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let parsed: T = serde_json::from_slice(&bytes).unwrap_or_else(|e| {
        panic!(
            "failed to parse response body as {}: {e}\nbody: {}",
            std::any::type_name::<T>(),
            String::from_utf8_lossy(&bytes)
        );
    });
    (status, parsed)
}

/// Send a request with an optional Bearer token and return just the status code.
pub async fn send_status(
    app: &Router,
    method: &str,
    uri: &str,
    body: Option<serde_json::Value>,
    token: Option<&str>,
) -> StatusCode {
    let mut builder = Request::builder().method(method).uri(uri);

    if let Some(t) = token {
        builder = builder.header("authorization", format!("Bearer {t}"));
    }

    let req_body = if let Some(json) = body {
        builder = builder.header("content-type", "application/json");
        Body::from(serde_json::to_vec(&json).unwrap())
    } else {
        Body::empty()
    };

    let req = builder.body(req_body).unwrap();
    let response: axum::response::Response = app.clone().oneshot(req).await.unwrap();
    response.status()
}
