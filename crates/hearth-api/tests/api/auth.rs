use crate::common::{send_status, test_app_with_auth};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// No token → 401
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore] // requires PostgreSQL
async fn no_token_returns_401() {
    let ctx = test_app_with_auth().await;

    // GET /api/v1/machines requires UserIdentity
    let status = send_status(&ctx.router, "GET", "/api/v1/machines", None, None).await;
    assert_eq!(status, 401);
}

// ---------------------------------------------------------------------------
// Valid user token → 200
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn valid_user_token_grants_access() {
    let ctx = test_app_with_auth().await;

    let token = ctx.mint_user_jwt("user-1", "alice", &["hearth-users"]);
    let status =
        send_status(&ctx.router, "GET", "/api/v1/machines", None, Some(&token)).await;
    assert_eq!(status, 200);
}

// ---------------------------------------------------------------------------
// Expired token → 401
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn expired_token_returns_401() {
    let ctx = test_app_with_auth().await;

    // exp = 0 → long expired
    let token = ctx.mint_user_jwt_with_exp("user-1", "alice", &["hearth-users"], 0);
    let status =
        send_status(&ctx.router, "GET", "/api/v1/machines", None, Some(&token)).await;
    assert_eq!(status, 401);
}

// ---------------------------------------------------------------------------
// Malformed token → 401
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn malformed_token_returns_401() {
    let ctx = test_app_with_auth().await;

    let status = send_status(
        &ctx.router,
        "GET",
        "/api/v1/machines",
        None,
        Some("this-is-not-a-jwt"),
    )
    .await;
    assert_eq!(status, 401);
}

// ---------------------------------------------------------------------------
// RBAC: viewer (hearth-users) cannot hit operator endpoint → 403
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn viewer_cannot_hit_operator_endpoint() {
    let ctx = test_app_with_auth().await;

    // POST /api/v1/catalog requires OperatorIdentity
    let token = ctx.mint_user_jwt("user-1", "alice", &["hearth-users"]);
    let body = serde_json::json!({
        "name": "test-app",
        "category": "Utilities",
        "install_method": "flatpak",
        "flatpak_ref": "com.example.Test",
    });
    let status = send_status(
        &ctx.router,
        "POST",
        "/api/v1/catalog",
        Some(body),
        Some(&token),
    )
    .await;
    assert_eq!(status, 403);
}

// ---------------------------------------------------------------------------
// RBAC: operator can hit operator endpoint → 2xx
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn operator_can_hit_operator_endpoint() {
    let ctx = test_app_with_auth().await;

    let token = ctx.mint_user_jwt("op-1", "bob", &["hearth-operators"]);
    let body = serde_json::json!({
        "name": "test-app",
        "category": "Utilities",
        "install_method": "flatpak",
        "flatpak_ref": "com.example.Test",
    });
    let status = send_status(
        &ctx.router,
        "POST",
        "/api/v1/catalog",
        Some(body),
        Some(&token),
    )
    .await;
    // 201 Created on success
    assert!(
        status.is_success(),
        "expected 2xx for operator, got {status}"
    );
}

// ---------------------------------------------------------------------------
// RBAC: operator cannot hit admin endpoint → 403
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn operator_cannot_hit_admin_endpoint() {
    let ctx = test_app_with_auth().await;

    // POST /api/v1/machines requires AdminIdentity
    let token = ctx.mint_user_jwt("op-1", "bob", &["hearth-operators"]);
    let body = serde_json::json!({
        "hostname": "test-machine",
    });
    let status = send_status(
        &ctx.router,
        "POST",
        "/api/v1/machines",
        Some(body),
        Some(&token),
    )
    .await;
    assert_eq!(status, 403);
}

// ---------------------------------------------------------------------------
// RBAC: admin can hit admin endpoint → 2xx
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn admin_can_hit_admin_endpoint() {
    let ctx = test_app_with_auth().await;

    let token = ctx.mint_user_jwt("admin-1", "carol", &["hearth-admins"]);
    let body = serde_json::json!({
        "hostname": "test-machine",
    });
    let status = send_status(
        &ctx.router,
        "POST",
        "/api/v1/machines",
        Some(body),
        Some(&token),
    )
    .await;
    assert!(status.is_success(), "expected 2xx for admin, got {status}");
}

// ---------------------------------------------------------------------------
// Machine token: valid → heartbeat succeeds
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn valid_machine_token_hits_heartbeat() {
    let ctx = test_app_with_auth().await;

    // First create a machine so the heartbeat has something to record against.
    // Use a user admin token to create it.
    let admin_token = ctx.mint_user_jwt("admin-1", "carol", &["hearth-admins"]);
    let machine_body = serde_json::json!({"hostname": "heartbeat-test"});
    let (status, machine): (_, serde_json::Value) = crate::common::send_json(
        &ctx.router,
        "POST",
        "/api/v1/machines",
        Some(machine_body),
        Some(&admin_token),
    )
    .await;
    assert!(status.is_success(), "failed to create machine: {status}");
    let machine_id: Uuid = machine["id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();

    let machine_token = ctx.mint_machine_jwt(machine_id);
    let heartbeat_body = serde_json::json!({
        "machine_id": machine_id.to_string(),
    });
    let status = send_status(
        &ctx.router,
        "POST",
        "/api/v1/heartbeat",
        Some(heartbeat_body),
        Some(&machine_token),
    )
    .await;
    assert!(
        status.is_success(),
        "expected 2xx for machine heartbeat, got {status}"
    );
}

// ---------------------------------------------------------------------------
// User token cannot hit machine-only endpoint → 401
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn user_token_cannot_hit_machine_endpoint() {
    let ctx = test_app_with_auth().await;

    // POST /api/v1/heartbeat requires MachineIdentity
    let user_token = ctx.mint_user_jwt("user-1", "alice", &["hearth-users"]);
    let body = serde_json::json!({
        "machine_id": Uuid::new_v4().to_string(),
    });
    let status = send_status(
        &ctx.router,
        "POST",
        "/api/v1/heartbeat",
        Some(body),
        Some(&user_token),
    )
    .await;
    assert_eq!(status, 401);
}

// ---------------------------------------------------------------------------
// Machine token signed with wrong secret → 401
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn invalid_machine_secret_returns_401() {
    let ctx = test_app_with_auth().await;

    let wrong_secret = b"wrong-secret-key-not-matching-config";
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let machine_id = Uuid::new_v4();
    let claims = serde_json::json!({
        "sub": format!("machine:{machine_id}"),
        "machine_id": machine_id.to_string(),
        "exp": now + 3600,
        "iat": now,
    });
    let key = jsonwebtoken::EncodingKey::from_secret(wrong_secret.as_slice());
    let bad_token =
        jsonwebtoken::encode(&jsonwebtoken::Header::default(), &claims, &key).unwrap();

    let body = serde_json::json!({
        "machine_id": machine_id.to_string(),
    });
    let status = send_status(
        &ctx.router,
        "POST",
        "/api/v1/heartbeat",
        Some(body),
        Some(&bad_token),
    )
    .await;
    assert_eq!(status, 401);
}

// ---------------------------------------------------------------------------
// JWT with unknown kid → 401
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn wrong_kid_returns_401() {
    let ctx = test_app_with_auth().await;

    // Sign with the correct key but use a different kid
    let claims = serde_json::json!({
        "sub": "user-1",
        "preferred_username": "alice",
        "groups": ["hearth-users"],
        "exp": jsonwebtoken::get_current_timestamp() + 3600,
        "iat": jsonwebtoken::get_current_timestamp(),
    });
    let mut header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256);
    header.kid = Some("unknown-key-id".to_string());
    let bad_token = jsonwebtoken::encode(&header, &claims, &ctx.encoding_key).unwrap();

    let status = send_status(
        &ctx.router,
        "GET",
        "/api/v1/machines",
        None,
        Some(&bad_token),
    )
    .await;
    assert_eq!(status, 401);
}

// ---------------------------------------------------------------------------
// Admin also has operator access (hearth-admins implies operator)
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn admin_can_hit_operator_endpoint() {
    let ctx = test_app_with_auth().await;

    let token = ctx.mint_user_jwt("admin-1", "carol", &["hearth-admins"]);
    let body = serde_json::json!({
        "name": "admin-created-app",
        "category": "Utilities",
        "install_method": "flatpak",
        "flatpak_ref": "com.example.AdminTest",
    });
    let status = send_status(
        &ctx.router,
        "POST",
        "/api/v1/catalog",
        Some(body),
        Some(&token),
    )
    .await;
    assert!(
        status.is_success(),
        "expected 2xx for admin hitting operator endpoint, got {status}"
    );
}
