use hearth_api::auth::AuthConfig;
use hearth_api::{AppState, build_router};
use sqlx::postgres::PgPoolOptions;
use tokio_util::sync::CancellationToken;
use tracing::info;

#[tokio::main]
async fn main() {
    // --- Structured logging (text or JSON) ---
    let log_format = std::env::var("LOG_FORMAT").unwrap_or_default();
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "hearth_api=info,tower_http=debug".into());

    if log_format == "json" {
        tracing_subscriber::fmt()
            .json()
            .with_env_filter(env_filter)
            .init();
    } else {
        tracing_subscriber::fmt().with_env_filter(env_filter).init();
    }

    // --- Prometheus metrics ---
    let metrics_handle = hearth_api::metrics::init();

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

    let headscale = hearth_api::headscale::HeadscaleClient::from_env();
    if let Some(ref hs) = headscale {
        info!(url = hs.url(), "headscale integration enabled");
    }

    let state = AppState {
        pool,
        auth_config: auth_config.clone(),
        headscale,
    };

    // Spawn background tasks
    let cancel = CancellationToken::new();

    // Deployment monitor
    let monitor_pool = state.pool.clone();
    tokio::spawn(hearth_api::deployment_monitor_run(
        monitor_pool,
        cancel.clone(),
    ));

    // Identity sync
    let sync_pool = state.pool.clone();
    tokio::spawn(hearth_api::identity_sync_run(
        sync_pool,
        auth_config,
        cancel.clone(),
    ));

    // Fleet gauge refresher (updates Prometheus gauges every 30s)
    let gauge_pool = state.pool.clone();
    tokio::spawn(hearth_api::metrics::refresh_fleet_gauges(
        gauge_pool,
        std::time::Duration::from_secs(30),
    ));

    let web_dist =
        std::env::var("HEARTH_WEB_DIST").unwrap_or_else(|_| "web/apps/hearth/dist".to_string());

    let app = build_router(state, &web_dist, metrics_handle);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .expect("failed to bind to port 3000");

    info!("hearth-api listening on 0.0.0.0:3000");
    axum::serve(listener, app).await.expect("server error");

    // Signal background tasks to stop on shutdown
    cancel.cancel();
}
