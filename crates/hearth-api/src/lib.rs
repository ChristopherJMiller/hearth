pub mod auth;
pub mod build;
mod cache_token;
pub mod db;
pub mod deployment_fsm;
mod deployment_monitor;
pub mod error;
pub mod headscale;
pub mod health_check;
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

/// Sweep pending user env configs and enqueue build jobs.
pub async fn user_env_build_sweep_run(pool: PgPool, cancel: CancellationToken) {
    let interval = std::time::Duration::from_secs(30);
    // Run the first sweep immediately so seeded configs are picked up on startup.
    let mut first_run = true;
    loop {
        tokio::select! {
            () = cancel.cancelled() => {
                tracing::info!("user env build sweep shutting down");
                break;
            }
            () = async {
                if first_run {
                    first_run = false;
                } else {
                    tokio::time::sleep(interval).await;
                }
            } => {
                match repo::get_pending_user_config_builds(&pool, 20).await {
                    Ok(configs) => {
                        for config in configs {
                            let hash = config.config_hash.unwrap_or_default();
                            if hash.is_empty() { continue; }
                            // Transition to 'building' to prevent re-enqueueing
                            // on the next sweep cycle.
                            if let Err(e) = repo::set_user_config_building(&pool, &config.username, &hash).await {
                                tracing::warn!(username = %config.username, error = %e, "failed to mark config as building");
                                continue;
                            }
                            match repo::enqueue_user_env_build(&pool, &config.username, &hash).await {
                                Ok(job) => {
                                    tracing::info!(
                                        username = %config.username,
                                        job_id = %job.id,
                                        "enqueued user env build"
                                    );
                                }
                                Err(e) => {
                                    tracing::warn!(
                                        username = %config.username,
                                        error = %e,
                                        "failed to enqueue user env build"
                                    );
                                }
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "failed to query pending user config builds");
                    }
                }
            }
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub auth_config: AuthConfig,
    pub headscale: Option<headscale::HeadscaleClient>,
    /// Attic binary cache URL for user environment closures.
    pub cache_url: Option<String>,
    /// When set, only these nixpkgs attributes are allowed in user
    /// `extra_packages` overrides. `None` means all packages are allowed.
    pub package_allowlist: Option<std::collections::HashSet<String>>,
    /// Available platform services (built from env vars at startup).
    pub services: Vec<hearth_common::api_types::ServiceInfo>,
    /// Matrix server name for directory contact derivation (e.g. `hearth.local`).
    pub matrix_server_name: Option<String>,
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

pub fn user_config_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/{username}/config",
            get(routes::user_configs::get_config).put(routes::user_configs::upsert_config),
        )
        .route(
            "/{username}/config/build",
            post(routes::user_configs::trigger_build),
        )
        .route(
            "/{username}/env-closure",
            get(routes::user_configs::get_env_closure),
        )
        .route(
            "/{username}/env-closure/report-failure",
            post(routes::user_configs::report_closure_failure),
        )
}

pub fn auth_me_route() -> Router<AppState> {
    Router::new().route("/me", get(routes::auth_me::me))
}

pub fn me_config_routes() -> Router<AppState> {
    Router::new().route(
        "/config",
        get(routes::me_config::get_my_config).put(routes::me_config::update_my_config),
    )
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

pub fn services_routes() -> Router<AppState> {
    Router::new().route("/", get(routes::services::list_services))
}

pub fn directory_routes() -> Router<AppState> {
    Router::new().route("/people", get(routes::directory::list_people))
}

pub fn compliance_routes() -> Router<AppState> {
    Router::new()
        .route("/drift", get(routes::compliance::list_drift))
        .route(
            "/policies",
            get(routes::compliance::list_policies).post(routes::compliance::create_policy),
        )
        .route(
            "/policies/{id}",
            get(routes::compliance::get_policy)
                .put(routes::compliance::update_policy)
                .delete(routes::compliance::delete_policy),
        )
        .route(
            "/deployments/{id}/results",
            get(routes::compliance::deployment_results),
        )
        .route(
            "/deployments/{id}/summary",
            get(routes::compliance::deployment_summary),
        )
        .route(
            "/sboms/{deployment_id}",
            get(routes::compliance::list_sboms),
        )
        .route(
            "/sboms/{deployment_id}/{machine_id}",
            get(routes::compliance::download_sbom),
        )
        .route(
            "/machines/{machine_id}/sbom",
            get(routes::compliance::machine_current_sbom),
        )
}

/// Build the service directory from environment variables.
pub fn build_services_from_env() -> Vec<hearth_common::api_types::ServiceInfo> {
    use hearth_common::api_types::{ServiceCategory, ServiceInfo};

    let definitions: &[(&str, &str, &str, ServiceCategory, &str, &str)] = &[
        (
            "HEARTH_SERVER_URL",
            "hearth",
            "Hearth Console",
            ServiceCategory::Infrastructure,
            "Fleet management console",
            "flame",
        ),
        (
            "HEARTH_CHAT_URL",
            "chat",
            "Hearth Chat",
            ServiceCategory::Communication,
            "Corporate chat powered by Matrix",
            "message-square",
        ),
        (
            "HEARTH_CLOUD_URL",
            "cloud",
            "Cloud Storage",
            ServiceCategory::Storage,
            "File storage and collaboration powered by Nextcloud",
            "cloud",
        ),
        (
            "HEARTH_IDENTITY_URL",
            "identity",
            "Identity Management",
            ServiceCategory::Identity,
            "Account management and single sign-on",
            "shield",
        ),
        (
            "HEARTH_GRAFANA_URL",
            "monitoring",
            "Monitoring",
            ServiceCategory::Infrastructure,
            "Fleet monitoring dashboards (Grafana)",
            "activity",
        ),
    ];

    let services: Vec<ServiceInfo> = definitions
        .iter()
        .filter_map(|(env_var, id, name, category, description, icon)| {
            std::env::var(env_var).ok().map(|url| ServiceInfo {
                id: (*id).into(),
                name: (*name).into(),
                category: *category,
                url,
                description: Some((*description).into()),
                icon: Some((*icon).into()),
            })
        })
        .collect();

    if !services.is_empty() {
        tracing::info!(count = services.len(), "service directory loaded");
    }

    services
}

/// Build the complete application router.
pub fn build_router(state: AppState, web_dist: &str, metrics_handle: PrometheusHandle) -> Router {
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
        .nest("/api/v1/compliance", compliance_routes())
        .route("/api/v1/stats", get(routes::stats::fleet_stats))
        .route("/api/v1/audit", get(routes::audit::list_audit_events))
        .nest(
            "/api/v1/machines/{machine_id}/environments",
            environments_routes(),
        )
        .route(
            "/api/v1/machines/{machine_id}/users/{username}/desktop-prefs",
            put(routes::user_configs::sync_desktop_prefs),
        )
        .nest("/api/v1/users", user_config_routes())
        .nest("/api/v1/services", services_routes())
        .nest("/api/v1/directory", directory_routes())
        .nest("/api/v1/me", me_config_routes())
        .nest("/api/v1/fleet-config", {
            let cache = routes::fleet_config::new_tarball_cache();
            Router::new()
                .route("/flake.tar.gz", get(routes::fleet_config::flake_tarball))
                .route("/latest", get(routes::fleet_config::flake_latest))
                .route("/{hash}/flake.tar.gz", get(routes::fleet_config::flake_tarball_by_hash))
                .with_state(cache)
        })
        .route(
            "/api/v1/cache-token",
            post(routes::cache_token::get_cache_token),
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
