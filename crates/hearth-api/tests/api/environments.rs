use crate::common::{
    create_machine_http, create_machine_http_authed, send_json, send_status, test_app,
    test_app_with_auth,
};
use hearth_common::api_types::UserEnvironment;
use serde_json::json;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Dev-mode (no auth) tests
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn upsert_and_get_environment() {
    let (app, _db) = test_app().await;
    let machine = create_machine_http(&app, "env-host").await;

    let uri = format!("/api/v1/machines/{}/environments/alice", machine.id);
    let body = json!({ "role": "developer", "status": "pending" });
    let (status, env): (_, UserEnvironment) = send_json(&app, "PUT", &uri, Some(body), None).await;
    assert_eq!(status, 200);
    assert_eq!(env.machine_id, machine.id);
    assert_eq!(env.username, "alice");
    assert_eq!(env.role, "developer");

    // Get the environment
    let (status, fetched): (_, UserEnvironment) = send_json(&app, "GET", &uri, None, None).await;
    assert_eq!(status, 200);
    assert_eq!(fetched.id, env.id);
    assert_eq!(fetched.username, "alice");
}

#[tokio::test]
#[ignore]
async fn list_environments() {
    let (app, _db) = test_app().await;
    let machine = create_machine_http(&app, "env-list-host").await;

    for user in ["bob", "charlie"] {
        let uri = format!("/api/v1/machines/{}/environments/{user}", machine.id);
        let body = json!({ "role": "developer" });
        let status = send_status(&app, "PUT", &uri, Some(body), None).await;
        assert_eq!(status, 200);
    }

    let list_uri = format!("/api/v1/machines/{}/environments", machine.id);
    let (status, envs): (_, Vec<UserEnvironment>) =
        send_json(&app, "GET", &list_uri, None, None).await;
    assert_eq!(status, 200);
    assert_eq!(envs.len(), 2);
}

#[tokio::test]
#[ignore]
async fn get_nonexistent_environment_returns_404() {
    let (app, _db) = test_app().await;
    let machine = create_machine_http(&app, "env-404-host").await;

    let uri = format!("/api/v1/machines/{}/environments/nonexistent", machine.id);
    let status = send_status(&app, "GET", &uri, None, None).await;
    assert_eq!(status, 404);
}

#[tokio::test]
#[ignore]
async fn upsert_updates_existing_environment() {
    let (app, _db) = test_app().await;
    let machine = create_machine_http(&app, "env-update-host").await;

    let uri = format!("/api/v1/machines/{}/environments/dave", machine.id);

    // Create
    let (_, env1): (_, UserEnvironment) = send_json(
        &app,
        "PUT",
        &uri,
        Some(json!({ "role": "developer" })),
        None,
    )
    .await;
    assert_eq!(env1.role, "developer");

    // Update role
    let (status, env2): (_, UserEnvironment) =
        send_json(&app, "PUT", &uri, Some(json!({ "role": "admin" })), None).await;
    assert_eq!(status, 200);
    assert_eq!(env2.role, "admin");
    assert_eq!(env2.id, env1.id); // same record
}

#[tokio::test]
#[ignore]
async fn record_login() {
    let (app, _db) = test_app().await;
    let machine = create_machine_http(&app, "login-host").await;

    // Create an environment first
    let env_uri = format!("/api/v1/machines/{}/environments/eve", machine.id);
    let (_, _env): (_, UserEnvironment) = send_json(
        &app,
        "PUT",
        &env_uri,
        Some(json!({ "role": "developer" })),
        None,
    )
    .await;

    // Record login
    let login_uri = format!("/api/v1/machines/{}/environments/eve/login", machine.id);
    let (status, updated): (_, UserEnvironment) =
        send_json(&app, "POST", &login_uri, None, None).await;
    assert_eq!(status, 200);
    assert_eq!(updated.username, "eve");
}

#[tokio::test]
#[ignore]
async fn record_login_nonexistent_returns_404() {
    let (app, _db) = test_app().await;
    let machine = create_machine_http(&app, "login-404-host").await;

    let login_uri = format!("/api/v1/machines/{}/environments/nobody/login", machine.id);
    let status = send_status(&app, "POST", &login_uri, None, None).await;
    assert_eq!(status, 404);
}

// ---------------------------------------------------------------------------
// Auth-enabled tests
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn environments_read_requires_user_identity() {
    let ctx = test_app_with_auth().await;

    let uri = format!("/api/v1/machines/{}/environments", Uuid::new_v4());

    // No token → 401
    let status = send_status(&ctx.router, "GET", &uri, None, None).await;
    assert_eq!(status, 401);

    // Valid user token → 200
    let token = ctx.mint_user_jwt("user-1", "user", &["hearth-users"]);
    let status = send_status(&ctx.router, "GET", &uri, None, Some(&token)).await;
    assert_eq!(status, 200);
}

#[tokio::test]
#[ignore]
async fn environments_write_requires_machine_identity() {
    let ctx = test_app_with_auth().await;
    let machine = create_machine_http_authed(&ctx, "auth-env-host").await;

    let uri = format!("/api/v1/machines/{}/environments/testuser", machine.id);
    let body = json!({ "role": "developer" });

    // User token cannot upsert (needs machine identity)
    let user_token = ctx.mint_user_jwt("user-1", "user", &["hearth-users"]);
    let status = send_status(
        &ctx.router,
        "PUT",
        &uri,
        Some(body.clone()),
        Some(&user_token),
    )
    .await;
    assert_eq!(status, 403);

    // Machine token can upsert
    let machine_token = ctx.mint_machine_jwt(machine.id);
    let (status, _): (_, UserEnvironment) =
        send_json(&ctx.router, "PUT", &uri, Some(body), Some(&machine_token)).await;
    assert_eq!(status, 200);
}
