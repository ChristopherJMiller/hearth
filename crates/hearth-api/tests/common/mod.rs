use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use serde::de::DeserializeOwned;
use sqlx::PgPool;
use std::sync::OnceLock;
use tower::ServiceExt;
use uuid::Uuid;

use hearth_api::AppState;
use hearth_api::auth::AuthConfig;

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

/// Send a request and parse the JSON response body.
pub async fn send_json<T: DeserializeOwned>(
    app: &Router,
    method: &str,
    uri: &str,
    body: Option<serde_json::Value>,
) -> (StatusCode, T) {
    let mut builder = Request::builder().method(method).uri(uri);

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

/// Send a request and return just the status code (for DELETE, etc.).
pub async fn send_status(
    app: &Router,
    method: &str,
    uri: &str,
    body: Option<serde_json::Value>,
) -> StatusCode {
    let mut builder = Request::builder().method(method).uri(uri);

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
