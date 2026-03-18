use crate::common::{create_machine_http, send_json, send_status, test_app, test_app_with_auth};
use hearth_common::api_types::{CatalogEntry, Machine, SoftwareRequest, SoftwareRequestStatus};
use serde_json::json;
use uuid::Uuid;

/// Create a catalog entry + machine + pending software request for testing.
async fn setup_request(app: &axum::Router) -> (CatalogEntry, Machine, SoftwareRequest) {
    let cat_body = json!({
        "name": "Test App",
        "install_method": "flatpak",
        "flatpak_ref": "org.test.App",
        "approval_required": true
    });
    let (_, entry): (_, CatalogEntry) =
        send_json(app, "POST", "/api/v1/catalog", Some(cat_body), None).await;

    let hostname = format!("req-host-{}", Uuid::new_v4().simple());
    let machine = create_machine_http(app, &hostname).await;

    let req_uri = format!("/api/v1/catalog/{}/request", entry.id);
    let (_, request): (_, SoftwareRequest) = send_json(
        app,
        "POST",
        &req_uri,
        Some(json!({ "machine_id": machine.id, "username": "testuser" })),
        None,
    )
    .await;

    (entry, machine, request)
}

// ---------------------------------------------------------------------------
// Dev-mode (no auth) tests
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn list_requests_empty() {
    let (app, _db) = test_app().await;

    let (status, requests): (_, Vec<SoftwareRequest>) =
        send_json(&app, "GET", "/api/v1/requests", None, None).await;
    assert_eq!(status, 200);
    assert!(requests.is_empty());
}

#[tokio::test]
#[ignore]
async fn list_requests_with_data() {
    let (app, _db) = test_app().await;
    let (_entry, _machine, request) = setup_request(&app).await;

    let (status, requests): (_, Vec<SoftwareRequest>) =
        send_json(&app, "GET", "/api/v1/requests", None, None).await;
    assert_eq!(status, 200);
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].id, request.id);
}

#[tokio::test]
#[ignore]
async fn list_requests_filter_by_status() {
    let (app, _db) = test_app().await;
    let (_entry, _machine, _request) = setup_request(&app).await;

    // Filter by pending
    let (status, pending): (_, Vec<SoftwareRequest>) =
        send_json(&app, "GET", "/api/v1/requests?status=pending", None, None).await;
    assert_eq!(status, 200);
    assert_eq!(pending.len(), 1);

    // Filter by approved (none yet)
    let (status, approved): (_, Vec<SoftwareRequest>) =
        send_json(&app, "GET", "/api/v1/requests?status=approved", None, None).await;
    assert_eq!(status, 200);
    assert!(approved.is_empty());
}

#[tokio::test]
#[ignore]
async fn approve_request_workflow() {
    let (app, _db) = test_app().await;
    let (_entry, _machine, request) = setup_request(&app).await;

    // Approve
    let approve_uri = format!("/api/v1/requests/{}/approve", request.id);
    let (status, approved): (_, SoftwareRequest) = send_json(
        &app,
        "POST",
        &approve_uri,
        Some(json!({ "admin": "admin-user" })),
        None,
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(approved.status, SoftwareRequestStatus::Approved);
    assert_eq!(approved.resolved_by.as_deref(), Some("admin-user"));

    // Claim install
    let claim_uri = format!("/api/v1/requests/{}/claim", request.id);
    let (status, claimed): (_, SoftwareRequest) =
        send_json(&app, "POST", &claim_uri, None, None).await;
    assert_eq!(status, 200);
    assert_eq!(claimed.status, SoftwareRequestStatus::Installing);

    // Report success
    let result_uri = format!("/api/v1/requests/{}/result", request.id);
    let (status, installed): (_, SoftwareRequest) = send_json(
        &app,
        "POST",
        &result_uri,
        Some(json!({ "request_id": request.id, "success": true })),
        None,
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(installed.status, SoftwareRequestStatus::Installed);
}

#[tokio::test]
#[ignore]
async fn deny_request() {
    let (app, _db) = test_app().await;
    let (_entry, _machine, request) = setup_request(&app).await;

    let deny_uri = format!("/api/v1/requests/{}/deny", request.id);
    let (status, denied): (_, SoftwareRequest) = send_json(
        &app,
        "POST",
        &deny_uri,
        Some(json!({ "admin": "admin-user" })),
        None,
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(denied.status, SoftwareRequestStatus::Denied);
}

#[tokio::test]
#[ignore]
async fn install_failure_workflow() {
    let (app, _db) = test_app().await;
    let (_entry, _machine, request) = setup_request(&app).await;

    // Approve → claim → report failure
    let approve_uri = format!("/api/v1/requests/{}/approve", request.id);
    send_json::<SoftwareRequest>(
        &app,
        "POST",
        &approve_uri,
        Some(json!({ "admin": "admin" })),
        None,
    )
    .await;

    let claim_uri = format!("/api/v1/requests/{}/claim", request.id);
    send_json::<SoftwareRequest>(&app, "POST", &claim_uri, None, None).await;

    let result_uri = format!("/api/v1/requests/{}/result", request.id);
    let (status, failed): (_, SoftwareRequest) = send_json(
        &app,
        "POST",
        &result_uri,
        Some(json!({ "request_id": request.id, "success": false })),
        None,
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(failed.status, SoftwareRequestStatus::Failed);
}

#[tokio::test]
#[ignore]
async fn approve_nonexistent_request_returns_404() {
    let (app, _db) = test_app().await;

    let uri = format!("/api/v1/requests/{}/approve", Uuid::new_v4());
    let status = send_status(&app, "POST", &uri, Some(json!({ "admin": "admin" })), None).await;
    assert_eq!(status, 404);
}

// ---------------------------------------------------------------------------
// Auth-enabled tests
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn approve_request_requires_operator() {
    let ctx = test_app_with_auth().await;

    // Viewer cannot approve
    let viewer_token = ctx.mint_user_jwt("viewer-1", "viewer", &["hearth-viewers"]);
    let uri = format!("/api/v1/requests/{}/approve", Uuid::new_v4());
    let status = send_status(
        &ctx.router,
        "POST",
        &uri,
        Some(json!({ "admin": "viewer" })),
        Some(&viewer_token),
    )
    .await;
    assert_eq!(status, 403);

    // Operator can attempt (will get 404 since request doesn't exist, not 403)
    let op_token = ctx.mint_user_jwt("op-1", "operator", &["hearth-operators"]);
    let status = send_status(
        &ctx.router,
        "POST",
        &uri,
        Some(json!({ "admin": "operator" })),
        Some(&op_token),
    )
    .await;
    assert_eq!(status, 404);
}
