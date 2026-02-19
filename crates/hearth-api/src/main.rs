use hearth_api::auth::AuthConfig;
use hearth_api::{AppState, build_router};
use sqlx::postgres::PgPoolOptions;
use tokio_util::sync::CancellationToken;
use tracing::info;

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

    let auth_config = AuthConfig::from_env();
    info!(
        auth_enabled = auth_config.is_enabled(),
        "auth configuration loaded"
    );

    let state = AppState { pool, auth_config };

    // Spawn deployment monitor background task
    let cancel = CancellationToken::new();
    let monitor_pool = state.pool.clone();
    tokio::spawn(hearth_api::deployment_monitor_run(
        monitor_pool,
        cancel.clone(),
    ));

    let catalog_dist =
        std::env::var("HEARTH_WEB_DIST").unwrap_or_else(|_| "web/apps/catalog/dist".to_string());

    let console_dist = std::env::var("HEARTH_CONSOLE_DIST")
        .unwrap_or_else(|_| "web/apps/console/dist".to_string());

    let app = build_router(state, &catalog_dist, &console_dist);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .expect("failed to bind to port 3000");

    info!("hearth-api listening on 0.0.0.0:3000");
    axum::serve(listener, app).await.expect("server error");

    // Signal background tasks to stop on shutdown
    cancel.cancel();
}
