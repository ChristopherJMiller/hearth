pub mod auth;
pub mod build;
mod cache_token;
pub mod db;
pub mod deployment_fsm;
mod deployment_monitor;
pub mod error;
mod health_check;
pub mod identity_sync;
pub mod metrics;
pub mod repo;
pub mod rollout;
mod routes;

use auth::AuthConfig;
use axum::Router;
use axum::middleware;
use axum::routing::{get, post, put};
use metrics_exporter_prometheus::PrometheusHandle;
use sqlx::PgPool;
use tokio_util::sync::CancellationToken;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::{DefaultOnResponse, TraceLayer};
use tracing::Level;

/// Start the deployment monitor background loop.
pub async fn deployment_monitor_run(pool: PgPool, cancel: CancellationToken) {
    deployment_monitor::run(pool, cancel).await;
}

/// Start the identity sync background loop.
pub async fn identity_sync_run(pool: PgPool, auth_config: AuthConfig, cancel: CancellationToken) {
    identity_sync::run(pool, auth_config, cancel).await;
}

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub auth_config: AuthConfig,
}

pub fn machines_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/",
            get(routes::machines::list_machines).post(routes::machines::create_machine),
        )
        .route(
            "/{id}",
            get(routes::machines::get_machine)
                .put(routes::machines::update_machine)
                .delete(routes::machines::delete_machine),
        )
        .route(
            "/{id}/target-state",
            get(routes::machines::get_target_state),
        )
        .route(
            "/{id}/actions",
            get(routes::actions::list_actions).post(routes::actions::create_action),
        )
}

pub fn heartbeat_routes() -> Router<AppState> {
    Router::new().route("/", post(routes::heartbeat::record_heartbeat))
}

pub fn catalog_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/",
            get(routes::catalog::list_catalog).post(routes::catalog::create_catalog_entry),
        )
        .route(
            "/{id}",
            get(routes::catalog::get_catalog_entry)
                .put(routes::catalog::update_catalog_entry)
                .delete(routes::catalog::delete_catalog_entry),
        )
        .route("/{id}/request", post(routes::catalog::request_software))
}

pub fn request_routes() -> Router<AppState> {
    Router::new()
        .route("/", get(routes::requests::list_requests))
        .route("/{id}/approve", post(routes::requests::approve_request))
        .route("/{id}/deny", post(routes::requests::deny_request))
        .route("/{id}/claim", post(routes::requests::claim_install))
        .route(
            "/{id}/result",
            post(routes::requests::report_install_result),
        )
}

pub fn deployments_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/",
            get(routes::deployments::list_deployments).post(routes::deployments::create_deployment),
        )
        .route("/{id}", get(routes::deployments::get_deployment))
        .route(
            "/{id}/status",
            put(routes::deployments::update_deployment_status),
        )
        .route(
            "/{id}/rollback",
            post(routes::deployments::rollback_deployment),
        )
        .route(
            "/{id}/machines",
            get(routes::deployments::list_deployment_machines),
        )
        .route(
            "/{id}/machines/{machine_id}",
            put(routes::deployments::update_machine_status),
        )
        .route("/build", post(routes::deployments::trigger_build))
}

pub fn build_job_routes() -> Router<AppState> {
    Router::new()
        .route("/", get(routes::build_jobs::list_build_jobs))
        .route("/{id}", get(routes::build_jobs::get_build_job))
}

pub fn enrollment_routes() -> Router<AppState> {
    Router::new()
        .route("/enroll", post(routes::enrollment::enroll))
        .route("/machines/{id}/approve", post(routes::enrollment::approve))
        .route(
            "/machines/{id}/enrollment-status",
            get(routes::enrollment::enrollment_status),
        )
}

pub fn environments_routes() -> Router<AppState> {
    Router::new()
        .route("/", get(routes::environments::list_environments))
        .route(
            "/{username}",
            get(routes::environments::get_environment)
                .put(routes::environments::upsert_environment),
        )
        .route(
            "/{username}/login",
            post(routes::environments::record_login),
        )
}

pub fn auth_me_route() -> Router<AppState> {
    Router::new().route("/me", get(routes::auth_me::me))
}

pub fn action_result_routes() -> Router<AppState> {
    Router::new().route("/{id}/result", post(routes::actions::report_action_result))
}

pub fn reports_routes() -> Router<AppState> {
    Router::new()
        .route("/compliance", get(routes::reports::compliance_report))
        .route("/deployments", get(routes::reports::deployment_timeline))
        .route("/enrollments", get(routes::reports::enrollment_timeline))
}

/// Build the complete application router.
pub fn build_router(
    state: AppState,
    web_dist: &str,
    metrics_handle: PrometheusHandle,
) -> Router {
    // Serve the unified SPA as a fallback for all non-API routes.
    let spa = ServeDir::new(web_dist).not_found_service(ServeFile::new(
        std::path::Path::new(web_dist).join("index.html"),
    ));

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/healthz", get(routes::health::healthz))
        .route(
            "/metrics",
            get(metrics::metrics_handler).with_state(metrics_handle),
        )
        .nest("/api/v1/machines", machines_routes())
        .nest("/api/v1/heartbeat", heartbeat_routes())
        .nest("/api/v1/catalog", catalog_routes())
        .nest("/api/v1/requests", request_routes())
        .nest("/api/v1/deployments", deployments_routes())
        .nest("/api/v1/build-jobs", build_job_routes())
        .nest("/api/v1", enrollment_routes())
        .nest("/api/v1/auth", auth_me_route())
        .nest("/api/v1/actions", action_result_routes())
        .nest("/api/v1/reports", reports_routes())
        .route("/api/v1/stats", get(routes::stats::fleet_stats))
        .route("/api/v1/audit", get(routes::audit::list_audit_events))
        .nest(
            "/api/v1/machines/{machine_id}/environments",
            environments_routes(),
        )
        .fallback_service(spa)
        .layer(middleware::from_fn(metrics::track_request))
        .layer(cors)
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(|request: &axum::http::Request<_>| {
                    tracing::info_span!(
                        "http_request",
                        method = %request.method(),
                        uri = %request.uri(),
                    )
                })
                .on_response(
                    DefaultOnResponse::new()
                        .level(Level::INFO)
                        .include_headers(false)
                        .latency_unit(tower_http::LatencyUnit::Millis),
                ),
        )
        .with_state(state)
}
