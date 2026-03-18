use crate::common::{send_json, test_app};
use hearth_common::api_types::{EnrollmentResponse, EnrollmentStatus};
use serde_json::json;

#[tokio::test]
#[ignore]
async fn enroll_creates_pending_machine() {
    let (app, _db) = test_app().await;

    let body = json!({
        "hostname": "new-device-01",
        "hardware_fingerprint": "abc123",
        "os_version": "NixOS 24.05"
    });

    let (status, resp): (_, EnrollmentResponse) =
        send_json(&app, "POST", "/api/v1/enroll", Some(body), None).await;

    // In dev mode the user is dev-admin (hearth-admins). The first enrollment
    // by an admin is auto-approved (bootstrap), so it should still be 201
    // but the status will be "approved" and a machine_token will be present.
    assert_eq!(status, 201);
    assert_eq!(resp.status, EnrollmentStatus::Approved);
    assert!(resp.machine_token.is_some(), "first enrollment by admin should auto-approve and mint token");
}

#[tokio::test]
#[ignore]
async fn second_enrollment_stays_pending() {
    let (app, _db) = test_app().await;

    // First enrollment: auto-approved (admin bootstrap)
    let body1 = json!({ "hostname": "bootstrap-device" });
    let (_, resp1): (_, EnrollmentResponse) =
        send_json(&app, "POST", "/api/v1/enroll", Some(body1), None).await;
    assert_eq!(resp1.status, EnrollmentStatus::Approved);

    // Second enrollment: should stay pending (no longer bootstrapping)
    let body2 = json!({ "hostname": "second-device" });
    let (status, resp2): (_, EnrollmentResponse) =
        send_json(&app, "POST", "/api/v1/enroll", Some(body2), None).await;
    assert_eq!(status, 201);
    assert_eq!(resp2.status, EnrollmentStatus::Pending);
    assert!(resp2.machine_token.is_none());
}

#[tokio::test]
#[ignore]
async fn approve_enrollment() {
    let (app, _db) = test_app().await;

    // Bootstrap first machine to exit auto-approve mode
    let body1 = json!({ "hostname": "bootstrap" });
    let (_, _): (_, EnrollmentResponse) =
        send_json(&app, "POST", "/api/v1/enroll", Some(body1), None).await;

    // Enroll a second device (will be pending)
    let body2 = json!({ "hostname": "pending-device" });
    let (_, resp): (_, EnrollmentResponse) =
        send_json(&app, "POST", "/api/v1/enroll", Some(body2), None).await;
    assert_eq!(resp.status, EnrollmentStatus::Pending);

    // Approve it
    let approve_uri = format!("/api/v1/machines/{}/approve", resp.machine_id);
    let approve_body = json!({
        "role": "developer",
        "admin": "test-admin"
    });
    let (status, approved): (_, EnrollmentResponse) =
        send_json(&app, "POST", &approve_uri, Some(approve_body), None).await;
    assert_eq!(status, 200);
    assert_eq!(approved.status, EnrollmentStatus::Approved);
    assert!(approved.machine_token.is_some(), "approval should mint a machine token");
}

#[tokio::test]
#[ignore]
async fn enrollment_status_returns_pending() {
    let (app, _db) = test_app().await;

    // Bootstrap
    let body1 = json!({ "hostname": "bootstrap" });
    let (_, _): (_, EnrollmentResponse) =
        send_json(&app, "POST", "/api/v1/enroll", Some(body1), None).await;

    // Enroll (pending)
    let body2 = json!({ "hostname": "status-check-device" });
    let (_, resp): (_, EnrollmentResponse) =
        send_json(&app, "POST", "/api/v1/enroll", Some(body2), None).await;

    // Check status
    let status_uri = format!("/api/v1/machines/{}/enrollment-status", resp.machine_id);
    let (status, check): (_, EnrollmentResponse) =
        send_json(&app, "GET", &status_uri, None, None).await;
    assert_eq!(status, 200);
    assert_eq!(check.status, EnrollmentStatus::Pending);
    assert!(check.machine_token.is_none());
}
