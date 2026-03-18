use crate::common::{
    assert_requires_user_identity, create_machine_http, create_machine_with_action, send_json,
    test_app, test_app_with_auth,
};
use hearth_common::api_types::AuditEvent;
use serde_json::Value;

// ---------------------------------------------------------------------------
// Dev-mode (no auth) tests
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn list_audit_events_empty() {
    let (app, _db) = test_app().await;

    let (status, events): (_, Vec<AuditEvent>) =
        send_json(&app, "GET", "/api/v1/audit", None, None).await;
    assert_eq!(status, 200);
    assert!(events.is_empty());
}

#[tokio::test]
#[ignore]
async fn audit_events_from_actions_with_filters() {
    let (app, _db) = test_app().await;

    // Create a machine and an action (which records an audit event)
    let (machine, _action) = create_machine_with_action(&app, "audit-host", "lock").await;

    // Verify audit event was created
    let (status, events): (_, Vec<AuditEvent>) =
        send_json(&app, "GET", "/api/v1/audit", None, None).await;
    assert_eq!(status, 200);
    assert!(
        events.iter().any(|e| e.event_type == "remote_action"),
        "expected a remote_action audit event"
    );

    // Filter by event_type
    let (status, filtered): (_, Vec<AuditEvent>) = send_json(
        &app,
        "GET",
        "/api/v1/audit?event_type=remote_action",
        None,
        None,
    )
    .await;
    assert_eq!(status, 200);
    assert!(
        !filtered.is_empty(),
        "expected at least one remote_action event"
    );
    assert!(filtered.iter().all(|e| e.event_type == "remote_action"));

    // Filter by nonexistent type → empty
    let (status, empty): (_, Vec<AuditEvent>) = send_json(
        &app,
        "GET",
        "/api/v1/audit?event_type=nonexistent",
        None,
        None,
    )
    .await;
    assert_eq!(status, 200);
    assert!(empty.is_empty());

    // Filter by machine_id
    let filter_uri = format!("/api/v1/audit?machine_id={}", machine.id);
    let (status, by_machine): (_, Vec<AuditEvent>) =
        send_json(&app, "GET", &filter_uri, None, None).await;
    assert_eq!(status, 200);
    assert!(
        !by_machine.is_empty(),
        "expected audit events for this machine"
    );
    assert!(by_machine.iter().all(|e| e.machine_id == Some(machine.id)));
}

#[tokio::test]
#[ignore]
async fn audit_events_limit() {
    let (app, _db) = test_app().await;

    let machine = create_machine_http(&app, "audit-limit-host").await;
    let action_uri = format!("/api/v1/machines/{}/actions", machine.id);

    // Create 3 actions to generate 3 audit events
    for _ in 0..3 {
        send_json::<hearth_common::api_types::PendingAction>(
            &app,
            "POST",
            &action_uri,
            Some(serde_json::json!({ "action_type": "lock", "payload": {} })),
            None,
        )
        .await;
    }

    // Limit to 2
    let (status, events): (_, Vec<AuditEvent>) =
        send_json(&app, "GET", "/api/v1/audit?limit=2", None, None).await;
    assert_eq!(status, 200);
    assert_eq!(events.len(), 2);
}

// ---------------------------------------------------------------------------
// Fleet stats
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn fleet_stats_empty() {
    let (app, _db) = test_app().await;

    let (status, stats): (_, Value) = send_json(&app, "GET", "/api/v1/stats", None, None).await;
    assert_eq!(status, 200);
    assert_eq!(stats["total_machines"], 0);
    assert_eq!(stats["active_machines"], 0);
    assert_eq!(stats["pending_enrollments"], 0);
}

#[tokio::test]
#[ignore]
async fn fleet_stats_with_machines() {
    let (app, _db) = test_app().await;

    for name in ["stats-a", "stats-b"] {
        create_machine_http(&app, name).await;
    }

    let (status, stats): (_, Value) = send_json(&app, "GET", "/api/v1/stats", None, None).await;
    assert_eq!(status, 200);
    assert_eq!(stats["total_machines"], 2);
}

// ---------------------------------------------------------------------------
// Auth-enabled tests
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn audit_and_stats_require_user_identity() {
    let ctx = test_app_with_auth().await;
    assert_requires_user_identity(&ctx, &["/api/v1/audit", "/api/v1/stats"]).await;
}
