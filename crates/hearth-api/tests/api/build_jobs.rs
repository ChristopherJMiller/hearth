use crate::common::{assert_requires_user_identity, send_json, send_status, test_app, test_app_with_auth};
use hearth_common::api_types::BuildJob;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Dev-mode (no auth) tests
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn list_build_jobs_empty() {
    let (app, _db) = test_app().await;

    let (status, jobs): (_, Vec<BuildJob>) =
        send_json(&app, "GET", "/api/v1/build-jobs", None, None).await;
    assert_eq!(status, 200);
    assert!(jobs.is_empty());
}

#[tokio::test]
#[ignore]
async fn list_build_jobs_with_data() {
    let (app, db) = test_app().await;

    // Insert a build job directly via repo
    let job = hearth_api::repo::enqueue_build_job(
        &db.pool,
        "github:myorg/fleet#nixosConfigurations",
        None,
        1,
        5,
        0.2,
    )
    .await
    .unwrap();

    let (status, jobs): (_, Vec<BuildJob>) =
        send_json(&app, "GET", "/api/v1/build-jobs", None, None).await;
    assert_eq!(status, 200);
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].id, job.id);
    assert_eq!(jobs[0].flake_ref, "github:myorg/fleet#nixosConfigurations");
}

#[tokio::test]
#[ignore]
async fn list_build_jobs_filter_by_status() {
    let (app, db) = test_app().await;

    // Create two jobs
    let _pending = hearth_api::repo::enqueue_build_job(
        &db.pool,
        "github:myorg/fleet#a",
        None,
        1,
        5,
        0.2,
    )
    .await
    .unwrap();

    let to_fail = hearth_api::repo::enqueue_build_job(
        &db.pool,
        "github:myorg/fleet#b",
        None,
        1,
        5,
        0.2,
    )
    .await
    .unwrap();

    // Fail the second one
    hearth_api::repo::fail_build_job(&db.pool, to_fail.id, "nix eval failed")
        .await
        .unwrap();

    // Filter by pending
    let (status, pending_jobs): (_, Vec<BuildJob>) =
        send_json(&app, "GET", "/api/v1/build-jobs?status=pending", None, None).await;
    assert_eq!(status, 200);
    assert_eq!(pending_jobs.len(), 1);

    // Filter by failed
    let (status, failed_jobs): (_, Vec<BuildJob>) =
        send_json(&app, "GET", "/api/v1/build-jobs?status=failed", None, None).await;
    assert_eq!(status, 200);
    assert_eq!(failed_jobs.len(), 1);
    assert_eq!(failed_jobs[0].error_message.as_deref(), Some("nix eval failed"));
}

#[tokio::test]
#[ignore]
async fn list_build_jobs_invalid_status_returns_400() {
    let (app, _db) = test_app().await;

    let status = send_status(
        &app,
        "GET",
        "/api/v1/build-jobs?status=invalid",
        None,
        None,
    )
    .await;
    assert_eq!(status, 400);
}

#[tokio::test]
#[ignore]
async fn get_build_job() {
    let (app, db) = test_app().await;

    let job = hearth_api::repo::enqueue_build_job(
        &db.pool,
        "github:myorg/fleet#test",
        None,
        2,
        10,
        0.1,
    )
    .await
    .unwrap();

    let uri = format!("/api/v1/build-jobs/{}", job.id);
    let (status, fetched): (_, BuildJob) = send_json(&app, "GET", &uri, None, None).await;
    assert_eq!(status, 200);
    assert_eq!(fetched.id, job.id);
    assert_eq!(fetched.canary_size, 2);
    assert_eq!(fetched.batch_size, 10);
}

#[tokio::test]
#[ignore]
async fn get_nonexistent_build_job_returns_404() {
    let (app, _db) = test_app().await;

    let uri = format!("/api/v1/build-jobs/{}", Uuid::new_v4());
    let status = send_status(&app, "GET", &uri, None, None).await;
    assert_eq!(status, 404);
}

// ---------------------------------------------------------------------------
// Auth-enabled tests
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn build_jobs_require_user_identity() {
    let ctx = test_app_with_auth().await;
    assert_requires_user_identity(&ctx, &["/api/v1/build-jobs"]).await;
}
