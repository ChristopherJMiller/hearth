use crate::common::{
    assert_requires_user_identity, create_machine_http, send_json, test_app, test_app_with_auth,
};
use serde_json::Value;

// ---------------------------------------------------------------------------
// Dev-mode (no auth) tests
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn compliance_report_empty() {
    let (app, _db) = test_app().await;

    let (status, report): (_, Value) =
        send_json(&app, "GET", "/api/v1/reports/compliance", None, None).await;
    assert_eq!(status, 200);
    assert_eq!(report["total"], 0);
    assert_eq!(report["compliant"], 0);
    assert_eq!(report["drifted"], 0);
    assert_eq!(report["no_target"], 0);
}

#[tokio::test]
#[ignore]
async fn compliance_report_with_machines() {
    let (app, _db) = test_app().await;

    // Create machines and mark them active — compliance report only counts active machines
    for name in ["comp-a", "comp-b"] {
        create_machine_http(&app, name).await;
        sqlx::query("UPDATE machines SET enrollment_status = 'active' WHERE hostname = $1")
            .bind(name)
            .execute(&_db.pool)
            .await
            .unwrap();
    }

    let (status, report): (_, Value) =
        send_json(&app, "GET", "/api/v1/reports/compliance", None, None).await;
    assert_eq!(status, 200);
    assert_eq!(report["total"], 2);
    assert_eq!(report["no_target"], 2);
}

#[tokio::test]
#[ignore]
async fn deployment_timeline_returns_ok() {
    let (app, _db) = test_app().await;

    // Default (30 days)
    let (status, _): (_, Vec<Value>) =
        send_json(&app, "GET", "/api/v1/reports/deployments", None, None).await;
    assert_eq!(status, 200);

    // Custom days parameter
    let (status, timeline): (_, Vec<Value>) = send_json(
        &app,
        "GET",
        "/api/v1/reports/deployments?days=7",
        None,
        None,
    )
    .await;
    assert_eq!(status, 200);
    assert!(timeline.len() <= 7);
}

#[tokio::test]
#[ignore]
async fn enrollment_timeline_returns_ok() {
    let (app, _db) = test_app().await;

    let (status, _): (_, Vec<Value>) =
        send_json(&app, "GET", "/api/v1/reports/enrollments", None, None).await;
    assert_eq!(status, 200);
}

// ---------------------------------------------------------------------------
// Auth-enabled tests
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn reports_require_user_identity() {
    let ctx = test_app_with_auth().await;
    assert_requires_user_identity(
        &ctx,
        &[
            "/api/v1/reports/compliance",
            "/api/v1/reports/deployments",
            "/api/v1/reports/enrollments",
        ],
    )
    .await;
}
