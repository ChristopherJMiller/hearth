//! hearth-build-worker: standalone process that polls the PostgreSQL build job
//! queue and executes the full build pipeline (eval → build → cache push →
//! deployment creation).
//!
//! Designed to run as one or more instances alongside the API server. Uses
//! `SELECT ... FOR UPDATE SKIP LOCKED` for safe concurrent job claiming.

use serde::Deserialize;
use sqlx::postgres::PgPoolOptions;
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use hearth_api::build::{orchestrator, user_env};
use hearth_api::db::BuildJobStatusDb;
use hearth_api::repo;

#[tokio::main]
async fn main() {
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "hearth_build_worker=info".into());

    if std::env::var("LOG_FORMAT").as_deref() == Ok("json") {
        tracing_subscriber::fmt()
            .json()
            .with_env_filter(env_filter)
            .init();
    } else {
        tracing_subscriber::fmt().with_env_filter(env_filter).init();
    }

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://hearth:hearth@localhost:5432/hearth".into());

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("failed to connect to database");

    info!("connected to database");

    // Run migrations (same as the API — idempotent).
    sqlx::migrate!("../../migrations")
        .run(&pool)
        .await
        .expect("failed to run migrations");

    info!("migrations applied");

    let worker_id = format!("worker-{}", &Uuid::new_v4().to_string()[..8]);
    let poll_interval = Duration::from_secs(
        std::env::var("HEARTH_WORKER_POLL_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(5),
    );

    info!(
        worker_id = %worker_id,
        poll_interval_secs = poll_interval.as_secs(),
        "build worker starting"
    );

    let cancel = CancellationToken::new();

    // Handle SIGTERM/SIGINT for graceful shutdown.
    let cancel_clone = cancel.clone();
    tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to listen for ctrl_c");
        info!("shutdown signal received");
        cancel_clone.cancel();
    });

    run_worker_loop(&pool, &worker_id, poll_interval, &cancel).await;

    info!("build worker shut down");
}

async fn run_worker_loop(
    pool: &sqlx::PgPool,
    worker_id: &str,
    poll_interval: Duration,
    cancel: &CancellationToken,
) {
    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                info!("cancellation received, exiting worker loop");
                break;
            }
            _ = tokio::time::sleep(poll_interval) => {
                // Poll for machine-level build jobs.
                match repo::claim_build_job(pool, worker_id).await {
                    Ok(Some(job)) => {
                        info!(
                            job_id = %job.id,
                            flake_ref = %job.flake_ref,
                            "claimed build job"
                        );
                        execute_job(pool, job.id, &job.flake_ref, job.target_filter.as_ref(), job.canary_size, job.batch_size, job.failure_threshold).await;
                    }
                    Ok(None) => {
                        // No machine jobs — try user env builds.
                    }
                    Err(e) => {
                        error!(error = %e, "failed to claim build job");
                    }
                }

                // Poll for per-user environment build jobs.
                match repo::claim_user_env_build(pool, worker_id).await {
                    Ok(Some(job)) => {
                        info!(
                            job_id = %job.id,
                            username = %job.username,
                            "claimed user env build job"
                        );
                        execute_user_env_job(pool, &job).await;
                    }
                    Ok(None) => {}
                    Err(e) => {
                        error!(error = %e, "failed to claim user env build job");
                    }
                }
            }
        }
    }
}

async fn execute_job(
    pool: &sqlx::PgPool,
    job_id: uuid::Uuid,
    flake_ref: &str,
    target_filter: Option<&serde_json::Value>,
    canary_size: i32,
    batch_size: i32,
    failure_threshold: f64,
) {
    // Mark as evaluating.
    if let Err(e) = repo::update_build_job_status(pool, job_id, BuildJobStatusDb::Evaluating).await
    {
        error!(job_id = %job_id, error = %e, "failed to update job status to evaluating");
        return;
    }

    // Run the full build pipeline. The orchestrator handles:
    // 1. Fleet config generation
    // 2. Flake evaluation (nix-eval-jobs)
    // 3. Parallel builds (nix build)
    // 4. Cache push (attic)
    // 5. Deployment + canary setup
    //
    // We update job status at a coarse granularity since the orchestrator
    // is an atomic unit — intermediate status is visible via tracing logs.
    match orchestrator::run_build_pipeline(
        pool,
        flake_ref,
        target_filter,
        canary_size,
        batch_size,
        failure_threshold,
    )
    .await
    {
        Ok(result) => {
            info!(
                job_id = %job_id,
                deployment_id = %result.deployment_id,
                machines = result.total_machines,
                built = result.closures_built,
                pushed = result.closures_pushed,
                "build pipeline completed"
            );

            if let Err(e) = repo::complete_build_job(
                pool,
                job_id,
                result.deployment_id,
                &result.closure,
                result.closures_built as i32,
                result.closures_pushed as i32,
                result.total_machines as i32,
            )
            .await
            {
                error!(job_id = %job_id, error = %e, "failed to mark job as completed");
            }
        }
        Err(e) => {
            let error_msg = format!("{e}");
            warn!(job_id = %job_id, error = %error_msg, "build pipeline failed");

            if let Err(db_err) = repo::fail_build_job(pool, job_id, &error_msg).await {
                error!(job_id = %job_id, error = %db_err, "failed to mark job as failed");
            }
        }
    }
}

async fn execute_user_env_job(pool: &sqlx::PgPool, job: &hearth_api::db::UserEnvBuildJobRow) {
    // Look up the user's full config.
    let config = match repo::get_user_config(pool, &job.username).await {
        Ok(Some(c)) => c,
        Ok(None) => {
            error!(username = %job.username, "user config not found for build job");
            let _ = repo::fail_user_env_build(pool, job.id, "user config not found").await;
            return;
        }
        Err(e) => {
            error!(username = %job.username, error = %e, "failed to fetch user config");
            let _ = repo::fail_user_env_build(pool, job.id, &format!("db error: {e}")).await;
            return;
        }
    };

    let flake_ref = match resolve_flake_ref().await {
        Ok(r) => r,
        Err(e) => {
            error!(job_id = %job.id, error = %e, "failed to resolve flake ref");
            let _ =
                repo::fail_user_env_build(pool, job.id, &format!("flake ref error: {e}")).await;
            return;
        }
    };
    let cache_name = std::env::var("ATTIC_CACHE_NAME").ok();

    match user_env::build_user_env(&config, &flake_ref, cache_name.as_deref()).await {
        Ok(closure) => {
            info!(
                job_id = %job.id,
                username = %job.username,
                %closure,
                "per-user build completed"
            );
            if let Err(e) = repo::complete_user_env_build(pool, job.id, &closure).await {
                error!(job_id = %job.id, error = %e, "failed to complete user env build");
            }
        }
        Err(e) => {
            let error_msg = format!("{e}");
            warn!(
                job_id = %job.id,
                username = %job.username,
                error = %error_msg,
                "per-user build failed"
            );
            if let Err(db_err) = repo::fail_user_env_build(pool, job.id, &error_msg).await {
                error!(job_id = %job.id, error = %db_err, "failed to mark user env build as failed");
            }
        }
    }
}

#[derive(Deserialize)]
struct FlakeLatestResponse {
    tarball_url: String,
}

/// Resolve the flake ref for the current build.
///
/// If `HEARTH_FLAKE_REF` looks like a full flake ref (contains "tarball+" or
/// "git+"), use it directly. Otherwise, treat it as an API server base URL and
/// query `/api/v1/fleet-config/latest` to get a content-addressed tarball URL.
async fn resolve_flake_ref() -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let raw = std::env::var("HEARTH_FLAKE_REF")
        .map_err(|_| "HEARTH_FLAKE_REF not set")?;

    // If it's already a full flake ref (legacy or explicit), use it directly
    if raw.contains("tarball+") || raw.contains("git+") || raw.contains("path:") {
        debug!(flake_ref = %raw, "using explicit flake ref");
        return Ok(raw);
    }

    // Treat as API server base URL — query /latest for content-addressed URL
    let latest_url = format!("{}/api/v1/fleet-config/latest", raw.trim_end_matches('/'));
    debug!(%latest_url, "querying fleet-config latest");

    let resp = reqwest::get(&latest_url).await?;
    if !resp.status().is_success() {
        return Err(format!(
            "fleet-config/latest returned {}: {}",
            resp.status(),
            resp.text().await.unwrap_or_default()
        ).into());
    }

    let latest: FlakeLatestResponse = resp.json().await?;
    info!(flake_ref = %latest.tarball_url, "resolved content-addressed flake ref");
    Ok(latest.tarball_url)
}
