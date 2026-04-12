//! End-to-end tests for the build pipeline: enrollment → build job → config gen → feedback.

use crate::common::{send_json, test_app};
use hearth_common::api_types::{EnrollmentResponse, EnrollmentStatus};
use serde_json::json;

// ---------------------------------------------------------------------------
// Config generation includes Approved machines
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn config_gen_includes_approved_machines() {
    let (_app, db) = test_app().await;

    // Create a machine in Approved status (simulating post-approval state).
    let enroll_req = hearth_common::api_types::CreateMachineRequest {
        hostname: "approved-device".into(),
        hardware_fingerprint: Some("fingerprint-a".into()),
        role: Some("default".into()),
        tags: None,
    };
    let row = hearth_api::repo::create_machine(&db.pool, &enroll_req)
        .await
        .unwrap();
    let machine: hearth_common::api_types::Machine = row.into();
    let machine_id = machine.id;

    // Set enrollment status to Approved.
    hearth_api::repo::approve_enrollment(&db.pool, machine_id, "default", None, None, None)
        .await
        .unwrap();

    // Generate fleet config with a target filter for this machine.
    let filter = json!({"machine_ids": [machine_id.to_string()]});
    let fleet = hearth_api::build::config_gen::generate_fleet_config(&db.pool, Some(&filter))
        .await
        .unwrap();

    assert_eq!(
        fleet.machines.len(),
        1,
        "Approved machine should be included in fleet config"
    );
    assert_eq!(fleet.machines[0].hostname, "approved-device");
}

#[tokio::test]
#[ignore]
async fn config_gen_excludes_pending_machines() {
    let (_app, db) = test_app().await;

    // Create a machine (defaults to Pending).
    let enroll_req = hearth_common::api_types::CreateMachineRequest {
        hostname: "pending-device".into(),
        hardware_fingerprint: Some("fingerprint-b".into()),
        role: Some("default".into()),
        tags: None,
    };
    let row = hearth_api::repo::create_machine(&db.pool, &enroll_req)
        .await
        .unwrap();
    let machine: hearth_common::api_types::Machine = row.into();

    let filter = json!({"machine_ids": [machine.id.to_string()]});
    let fleet = hearth_api::build::config_gen::generate_fleet_config(&db.pool, Some(&filter))
        .await
        .unwrap();

    assert_eq!(
        fleet.machines.len(),
        0,
        "Pending machine should NOT be included in fleet config"
    );
}

// ---------------------------------------------------------------------------
// Enrollment creates a build job with correct flake ref
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn enrollment_creates_build_job() {
    // HEARTH_FLAKE_REF must be set for enrollment to queue a build.
    // SAFETY: test-only; no other threads reading this env var concurrently.
    unsafe { std::env::set_var("HEARTH_FLAKE_REF", "path:/test/repo") };

    let (app, db) = test_app().await;

    let body = json!({
        "hostname": "build-test-device",
        "hardware_fingerprint": "fp-build-test"
    });
    let (status, resp): (_, EnrollmentResponse) =
        send_json(&app, "POST", "/api/v1/enroll", Some(body), None).await;
    assert_eq!(status, 201);
    // First enrollment by admin is auto-approved.
    assert_eq!(resp.status, EnrollmentStatus::Approved);

    // A build job should have been created for this machine.
    let jobs = hearth_api::repo::list_build_jobs(&db.pool, None)
        .await
        .unwrap();
    assert!(
        !jobs.is_empty(),
        "auto-approved enrollment should create a build job"
    );

    let job = &jobs[0];
    assert_eq!(job.flake_ref, "path:/test/repo");
    assert_eq!(job.status, hearth_api::db::BuildJobStatusDb::Pending);

    // target_filter should reference the machine.
    let filter = job.target_filter.as_ref().unwrap();
    let machine_ids = filter["machine_ids"].as_array().unwrap();
    assert_eq!(machine_ids.len(), 1);
    assert_eq!(
        machine_ids[0].as_str().unwrap(),
        resp.machine_id.to_string()
    );
}

// ---------------------------------------------------------------------------
// Enrollment status returns build error when build fails
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn enrollment_status_shows_build_error() {
    // SAFETY: test-only; no other threads reading this env var concurrently.
    unsafe { std::env::set_var("HEARTH_FLAKE_REF", "path:/test/repo") };

    let (app, db) = test_app().await;

    // Enroll and auto-approve.
    let body = json!({
        "hostname": "build-error-device",
        "hardware_fingerprint": "fp-build-err"
    });
    let (_, resp): (_, EnrollmentResponse) =
        send_json(&app, "POST", "/api/v1/enroll", Some(body), None).await;
    assert_eq!(resp.status, EnrollmentStatus::Approved);

    // Find and fail the build job.
    let jobs = hearth_api::repo::list_build_jobs(&db.pool, None)
        .await
        .unwrap();
    assert!(!jobs.is_empty());
    hearth_api::repo::fail_build_job(
        &db.pool,
        jobs[0].id,
        "nix eval failed: boot.lanzaboote does not exist",
    )
    .await
    .unwrap();

    // Poll enrollment status — should include the build error.
    let status_uri = format!("/api/v1/machines/{}/enrollment-status", resp.machine_id);
    let (status, check): (_, EnrollmentResponse) =
        send_json(&app, "GET", &status_uri, None, None).await;
    assert_eq!(status, 200);
    assert_eq!(
        check.build_status.as_deref(),
        Some("failed"),
        "build_status should be 'failed'"
    );
    assert!(
        check
            .build_error
            .as_deref()
            .unwrap_or("")
            .contains("lanzaboote"),
        "build_error should contain the error message"
    );
    assert!(
        check.target_closure.is_none(),
        "target_closure should be None when build failed"
    );
}

#[tokio::test]
#[ignore]
async fn enrollment_status_shows_pending_build() {
    // SAFETY: test-only; no other threads reading this env var concurrently.
    unsafe { std::env::set_var("HEARTH_FLAKE_REF", "path:/test/repo") };

    let (app, _db) = test_app().await;

    let body = json!({
        "hostname": "build-pending-device",
        "hardware_fingerprint": "fp-build-pending"
    });
    let (_, resp): (_, EnrollmentResponse) =
        send_json(&app, "POST", "/api/v1/enroll", Some(body), None).await;
    assert_eq!(resp.status, EnrollmentStatus::Approved);

    // Poll status — build should be pending.
    let status_uri = format!("/api/v1/machines/{}/enrollment-status", resp.machine_id);
    let (status, check): (_, EnrollmentResponse) =
        send_json(&app, "GET", &status_uri, None, None).await;
    assert_eq!(status, 200);
    assert_eq!(
        check.build_status.as_deref(),
        Some("pending"),
        "build_status should be 'pending' for unclaimed job"
    );
    assert!(check.build_error.is_none());
}

// ---------------------------------------------------------------------------
// Fleet config tarball endpoint
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn fleet_config_tarball_returns_gzip() {
    // Point at the repo root so the tarball can find flake.nix, modules/, etc.
    let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    // SAFETY: test-only; no other threads reading this env var concurrently.
    unsafe { std::env::set_var("HEARTH_FLAKE_DIR", repo_root) };

    let (app, _db) = test_app().await;

    let req = axum::http::Request::builder()
        .uri("/api/v1/fleet-config/flake.tar.gz")
        .body(axum::body::Body::empty())
        .unwrap();
    let response = tower::ServiceExt::oneshot(app, req).await.unwrap();

    assert_eq!(response.status(), 200);

    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(content_type, "application/gzip");

    // Read the body and verify it's a valid gzip+tar.
    let bytes = http_body_util::BodyExt::collect(response.into_body())
        .await
        .unwrap()
        .to_bytes();

    // Should be non-trivially sized (contains flake.nix at minimum).
    assert!(
        bytes.len() > 100,
        "tarball should be non-trivial, got {} bytes",
        bytes.len()
    );

    // Decompress and check for expected entries.
    let decoder = flate2::read::GzDecoder::new(&bytes[..]);
    let mut archive = tar::Archive::new(decoder);
    let entries: Vec<String> = archive
        .entries()
        .unwrap()
        .filter_map(|e| {
            e.ok()
                .and_then(|e| e.path().ok().map(|p| p.to_string_lossy().to_string()))
        })
        .collect();

    assert!(
        entries.iter().any(|e| e == "flake.nix"),
        "tarball should contain flake.nix, got: {entries:?}"
    );
    assert!(
        entries.iter().any(|e| e == "flake.lock"),
        "tarball should contain flake.lock"
    );
    assert!(
        entries.iter().any(|e| e.starts_with("modules/")),
        "tarball should contain modules/"
    );
    assert!(
        entries.iter().any(|e| e.starts_with("lib/")),
        "tarball should contain lib/"
    );
}

// ---------------------------------------------------------------------------
// Build job claiming and worker integration
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn build_job_claim_and_status_progression() {
    let (_app, db) = test_app().await;

    // Enqueue a job.
    let job = hearth_api::repo::enqueue_build_job(
        &db.pool,
        "path:/test/repo",
        Some(&json!({"machine_ids": ["aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee"]})),
        1,
        1,
        1.0,
    )
    .await
    .unwrap();
    assert_eq!(job.status, hearth_api::db::BuildJobStatusDb::Pending);

    // Claim it (simulating a worker).
    let claimed = hearth_api::repo::claim_build_job(&db.pool, "worker-test-1")
        .await
        .unwrap();
    assert!(claimed.is_some(), "should claim the pending job");
    let claimed = claimed.unwrap();
    assert_eq!(claimed.id, job.id);
    assert_eq!(claimed.status, hearth_api::db::BuildJobStatusDb::Claimed);
    assert_eq!(claimed.worker_id.as_deref(), Some("worker-test-1"));

    // Update to evaluating.
    hearth_api::repo::update_build_job_status(
        &db.pool,
        job.id,
        hearth_api::db::BuildJobStatusDb::Evaluating,
    )
    .await
    .unwrap();

    // Fail it.
    hearth_api::repo::fail_build_job(&db.pool, job.id, "eval error: missing attribute")
        .await
        .unwrap();

    let final_job = hearth_api::repo::get_build_job(&db.pool, job.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(final_job.status, hearth_api::db::BuildJobStatusDb::Failed);
    assert_eq!(
        final_job.error_message.as_deref(),
        Some("eval error: missing attribute")
    );
}

#[tokio::test]
#[ignore]
async fn latest_build_job_for_machine_query() {
    let (_app, db) = test_app().await;

    let machine_id = uuid::Uuid::new_v4();
    let filter = json!({"machine_ids": [machine_id.to_string()]});

    // No jobs yet.
    let result = hearth_api::repo::latest_build_job_for_machine(&db.pool, machine_id)
        .await
        .unwrap();
    assert!(result.is_none());

    // Create a job targeting this machine.
    let job = hearth_api::repo::enqueue_build_job(&db.pool, "path:/test", Some(&filter), 1, 1, 1.0)
        .await
        .unwrap();

    // Should find it.
    let result = hearth_api::repo::latest_build_job_for_machine(&db.pool, machine_id)
        .await
        .unwrap();
    assert!(result.is_some());
    assert_eq!(result.unwrap().id, job.id);

    // Create a second job — should return the newer one.
    let job2 =
        hearth_api::repo::enqueue_build_job(&db.pool, "path:/test-v2", Some(&filter), 1, 1, 1.0)
            .await
            .unwrap();

    let result = hearth_api::repo::latest_build_job_for_machine(&db.pool, machine_id)
        .await
        .unwrap();
    assert_eq!(result.unwrap().id, job2.id);
}
