mod db;
mod error;
mod repo;
mod routes;

use axum::Router;
use axum::routing::{get, post};
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;
use tracing::info;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
}

fn machines_routes() -> Router<AppState> {
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
}

fn heartbeat_routes() -> Router<AppState> {
    Router::new().route("/", post(routes::heartbeat::record_heartbeat))
}

fn catalog_routes() -> Router<AppState> {
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

fn request_routes() -> Router<AppState> {
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

fn enrollment_routes() -> Router<AppState> {
    Router::new()
        .route("/enroll", post(routes::enrollment::enroll))
        .route("/machines/{id}/approve", post(routes::enrollment::approve))
        .route(
            "/machines/{id}/enrollment-status",
            get(routes::enrollment::enrollment_status),
        )
}

fn environments_routes() -> Router<AppState> {
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

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "hearth_api=info,tower_http=debug".into()),
        )
        .init();

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://hearth:hearth@localhost:5432/hearth".into());

    let pool = PgPoolOptions::new()
        .max_connections(20)
        .connect(&database_url)
        .await
        .expect("failed to connect to database");

    info!("connected to database");

    sqlx::migrate!("../../migrations")
        .run(&pool)
        .await
        .expect("failed to run migrations");

    info!("migrations applied");

    let state = AppState { pool };

    // Serve the Vite-built catalog SPA from the dist directory.
    // Falls back to the SPA handler for client-side routes.
    let catalog_dist =
        std::env::var("HEARTH_WEB_DIST").unwrap_or_else(|_| "web/apps/catalog/dist".to_string());

    let catalog_spa = Router::new()
        .fallback(get(routes::web::catalog_spa_fallback))
        .nest_service("/", ServeDir::new(&catalog_dist));

    let app = Router::new()
        .route("/healthz", get(routes::health::healthz))
        .nest("/api/v1/machines", machines_routes())
        .nest("/api/v1/heartbeat", heartbeat_routes())
        .nest("/api/v1/catalog", catalog_routes())
        .nest("/api/v1/requests", request_routes())
        .nest("/api/v1", enrollment_routes())
        .nest(
            "/api/v1/machines/{machine_id}/environments",
            environments_routes(),
        )
        .nest("/catalog", catalog_spa)
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .expect("failed to bind to port 3000");

    info!("hearth-api listening on 0.0.0.0:3000");
    axum::serve(listener, app).await.expect("server error");
}
