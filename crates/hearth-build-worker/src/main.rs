//! hearth-build-worker: standalone process that polls the PostgreSQL build job
//! queue and executes the full build pipeline (eval → build → cache push →
//! deployment creation).
//!
//! Designed to run as one or more instances alongside the API server. Uses
//! `SELECT ... FOR UPDATE SKIP LOCKED` for safe concurrent job claiming.

use sqlx::postgres::PgPoolOptions;
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};
use uuid::Uuid;

use hearth_api::build::orchestrator;
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
                        // No jobs available — sleep and retry.
                    }
                    Err(e) => {
                        error!(error = %e, "failed to claim build job");
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
