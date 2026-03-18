use crate::common::{
    create_machine_http, create_machine_http_authed, create_machine_with_action, send_json,
    send_status, test_app, test_app_with_auth,
};
use hearth_common::api_types::{ActionStatus, ActionType, PendingAction};
use serde_json::json;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Dev-mode (no auth) tests
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn create_and_list_actions() {
    let (app, _db) = test_app().await;

    let (machine, action) = create_machine_with_action(&app, "action-target", "lock").await;
    assert_eq!(action.machine_id, machine.id);
    assert_eq!(action.action_type, ActionType::Lock);

    // List actions for the machine
    let uri = format!("/api/v1/machines/{}/actions", machine.id);
    let (status, actions): (_, Vec<PendingAction>) = send_json(&app, "GET", &uri, None, None).await;
    assert_eq!(status, 200);
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].id, action.id);
}

#[tokio::test]
#[ignore]
async fn create_restart_action() {
    let (app, _db) = test_app().await;

    let (_machine, action) = create_machine_with_action(&app, "restart-target", "restart").await;
    assert_eq!(action.action_type, ActionType::Restart);
}

#[tokio::test]
#[ignore]
async fn report_action_result_success_and_failure() {
    let (app, _db) = test_app().await;

    // Create two actions on the same machine
    let machine = create_machine_http(&app, "result-target").await;
    let action_uri = format!("/api/v1/machines/{}/actions", machine.id);

    let (_, success_action): (_, PendingAction) = send_json(
        &app,
        "POST",
        &action_uri,
        Some(json!({ "action_type": "lock", "payload": {} })),
        None,
    )
    .await;

    let (_, failure_action): (_, PendingAction) = send_json(
        &app,
        "POST",
        &action_uri,
        Some(json!({ "action_type": "restart", "payload": {} })),
        None,
    )
    .await;

    // Report success
    let result_uri = format!("/api/v1/actions/{}/result", success_action.id);
    let (status, completed): (_, PendingAction) = send_json(
        &app,
        "POST",
        &result_uri,
        Some(json!({
            "action_id": success_action.id,
            "success": true,
            "result": { "message": "locked successfully" }
        })),
        None,
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(completed.status, ActionStatus::Completed);

    // Report failure
    let result_uri = format!("/api/v1/actions/{}/result", failure_action.id);
    let (status, failed): (_, PendingAction) = send_json(
        &app,
        "POST",
        &result_uri,
        Some(json!({
            "action_id": failure_action.id,
            "success": false,
            "result": { "error": "permission denied" }
        })),
        None,
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(failed.status, ActionStatus::Failed);
}

#[tokio::test]
#[ignore]
async fn report_result_nonexistent_action_returns_404() {
    let (app, _db) = test_app().await;

    let result_uri = format!("/api/v1/actions/{}/result", Uuid::new_v4());
    let result_body = json!({
        "action_id": Uuid::new_v4(),
        "success": true,
        "result": null
    });
    let status = send_status(&app, "POST", &result_uri, Some(result_body), None).await;
    assert_eq!(status, 404);
}

// ---------------------------------------------------------------------------
// Auth-enabled tests
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn create_action_requires_admin() {
    let ctx = test_app_with_auth().await;

    let machine = create_machine_http_authed(&ctx, "auth-test").await;
    let uri = format!("/api/v1/machines/{}/actions", machine.id);
    let body = json!({ "action_type": "lock", "payload": {} });

    // Viewer cannot create actions
    let viewer_token = ctx.mint_user_jwt("viewer-1", "viewer", &["hearth-viewers"]);
    let status = send_status(
        &ctx.router,
        "POST",
        &uri,
        Some(body.clone()),
        Some(&viewer_token),
    )
    .await;
    assert_eq!(status, 403);

    // Admin can create actions
    let admin_token = ctx.mint_user_jwt("admin-1", "admin", &["hearth-admins"]);
    let (status, _): (_, PendingAction) =
        send_json(&ctx.router, "POST", &uri, Some(body), Some(&admin_token)).await;
    assert_eq!(status, 200);
}

#[tokio::test]
#[ignore]
async fn list_actions_requires_user_identity() {
    let ctx = test_app_with_auth().await;

    let uri = format!("/api/v1/machines/{}/actions", Uuid::new_v4());

    // No token → 401
    let status = send_status(&ctx.router, "GET", &uri, None, None).await;
    assert_eq!(status, 401);

    // Valid token → 200
    let token = ctx.mint_user_jwt("user-1", "user", &["hearth-users"]);
    let status = send_status(&ctx.router, "GET", &uri, None, Some(&token)).await;
    assert_eq!(status, 200);
}

#[tokio::test]
#[ignore]
async fn report_result_requires_machine_identity() {
    let ctx = test_app_with_auth().await;

    let uri = format!("/api/v1/actions/{}/result", Uuid::new_v4());
    let body = json!({
        "action_id": Uuid::new_v4(),
        "success": true,
        "result": null
    });

    // User token should be rejected (needs machine token)
    let user_token = ctx.mint_user_jwt("user-1", "user", &["hearth-users"]);
    let status = send_status(
        &ctx.router,
        "POST",
        &uri,
        Some(body.clone()),
        Some(&user_token),
    )
    .await;
    assert_eq!(status, 403);

    // Machine token should be accepted (404 because action doesn't exist, not 401/403)
    let machine_token = ctx.mint_machine_jwt(Uuid::new_v4());
    let status = send_status(&ctx.router, "POST", &uri, Some(body), Some(&machine_token)).await;
    assert_eq!(status, 404);
}
