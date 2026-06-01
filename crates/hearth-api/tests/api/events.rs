//! Integration tests for the SSE push fast-path (RFC-001).
//!
//! Auth-boundary cases hit the HTTP endpoint; the cross-replica fan-out
//! test exercises the repo → pg_notify → LISTEN → bus chain by spawning
//! the listener and asserting the in-process bus receives the event.

use std::sync::Arc;
use std::time::Duration;

use hearth_api::events::{EventBus, MachineEvent};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::common::{create_machine_http_authed, send_status, test_app_with_auth};

// ---------------------------------------------------------------------------
// GET /api/v1/machines/{id}/events
//
// Auth contract (crates/hearth-api/src/routes/machine_events.rs):
//   - MachineIdentity extractor → 401 without a machine JWT.
//   - machine_id in path must equal machine_id in token → 403 on
//     mismatch (otherwise any agent could tap any other agent's push
//     stream).
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn events_endpoint_rejects_missing_token() {
    let ctx = test_app_with_auth().await;
    let machine_id = Uuid::new_v4();
    let uri = format!("/api/v1/machines/{machine_id}/events");

    let status = send_status(&ctx.router, "GET", &uri, None, None).await;
    assert_eq!(status, 401, "expected 401 without any token, got {status}");
}

#[tokio::test]
#[ignore]
async fn events_endpoint_rejects_user_token() {
    let ctx = test_app_with_auth().await;
    let machine_id = Uuid::new_v4();
    let uri = format!("/api/v1/machines/{machine_id}/events");

    // A perfectly valid user OIDC JWT must not be accepted on a
    // machine-scoped endpoint. The `MachineIdentity` extractor in
    // `crates/hearth-api/src/auth.rs` distinguishes the two failure
    // modes: it returns 403 ("this endpoint requires machine
    // identity") for a valid *user* JWT and 401 for any other token
    // shape. We assert the more specific 403 here.
    let user_token = ctx.mint_user_jwt("user-1", "user", &["hearth-users"]);
    let status = send_status(&ctx.router, "GET", &uri, None, Some(&user_token)).await;
    assert_eq!(
        status, 403,
        "user JWT must yield 403 (wrong token type) on machine-scoped endpoint, got {status}"
    );
}

#[tokio::test]
#[ignore]
async fn events_endpoint_rejects_mismatched_machine_token() {
    let ctx = test_app_with_auth().await;
    let machine = create_machine_http_authed(&ctx, "events-mismatch-host").await;

    // Mint a token for a *different* machine id, then try to subscribe
    // to `machine`'s push stream. The handler must refuse — otherwise a
    // compromised device could tap arbitrary fleet hosts' update
    // notifications.
    let other_machine_id = Uuid::new_v4();
    let bad_token = ctx.mint_machine_jwt(other_machine_id);

    let uri = format!("/api/v1/machines/{}/events", machine.id);
    let status = send_status(&ctx.router, "GET", &uri, None, Some(&bad_token)).await;
    assert_eq!(
        status, 403,
        "expected 403 on machine_id/path mismatch, got {status}"
    );
}

#[tokio::test]
#[ignore]
async fn events_endpoint_accepts_matching_machine_token() {
    let ctx = test_app_with_auth().await;
    let machine = create_machine_http_authed(&ctx, "events-match-host").await;
    let token = ctx.mint_machine_jwt(machine.id);

    let uri = format!("/api/v1/machines/{}/events", machine.id);

    // Use a low-level oneshot so we can read just the response head
    // without draining the (long-lived) SSE body. Status + content-type
    // are enough to prove the subscription was accepted.
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;
    let req = Request::builder()
        .method("GET")
        .uri(&uri)
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    let resp = ctx.router.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), 200, "expected 200 on matching machine token");
    let content_type = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        content_type.starts_with("text/event-stream"),
        "expected text/event-stream content-type, got {content_type}"
    );
}

// ---------------------------------------------------------------------------
// Cross-replica fan-out: repo → pg_notify → LISTEN → bus
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn complete_user_env_build_emits_machine_event() {
    let ctx = test_app_with_auth().await;
    let pool = ctx.db.pool.clone();

    // Use a fresh standalone bus so the test runs without racing the
    // bus the router was built with (which would also be valid, but a
    // dedicated bus keeps the assertion crisp).
    let bus = EventBus::new();
    let cancel = CancellationToken::new();
    let listener_handle = {
        let pool = pool.clone();
        let bus = bus.clone();
        let cancel = cancel.clone();
        tokio::spawn(async move {
            hearth_api::events::run_listener(pool, bus, cancel).await;
        })
    };

    // Give the listener a moment to LISTEN before we publish.
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Seed: machine + user_environments row that pins the machine to
    // this user (so the fan-out can find it).
    let machine = create_machine_http_authed(&ctx, "events-fanout-host").await;
    let username = "events-fanout-user";
    sqlx::query(
        "INSERT INTO user_environments (machine_id, username, role, status)
         VALUES ($1, $2, 'default', 'pending')",
    )
    .bind(machine.id)
    .bind(username)
    .execute(&pool)
    .await
    .unwrap();

    let mut rx = bus.subscribe(machine.id);

    // Insert a build job, then complete it via the repo path under
    // test. complete_user_env_build is what the build worker calls;
    // see crates/hearth-build-worker/src/main.rs.
    let job_id: Uuid = sqlx::query_scalar(
        "INSERT INTO user_env_build_jobs (username, config_hash, status)
         VALUES ($1, 'hash-events-fanout', 'building')
         RETURNING id",
    )
    .bind(username)
    .fetch_one(&pool)
    .await
    .unwrap();

    // user_configs must exist for the fan-out UPDATE to land.
    sqlx::query(
        "INSERT INTO user_configs (username, base_role, config_hash, build_status)
         VALUES ($1, 'default', 'hash-events-fanout', 'building')",
    )
    .bind(username)
    .execute(&pool)
    .await
    .unwrap();

    hearth_api::repo::complete_user_env_build(
        &pool,
        job_id,
        "/nix/store/test-events-fanout-closure",
    )
    .await
    .unwrap();

    let event = tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await
        .expect("expected to receive a state_changed event within 5s")
        .expect("broadcast send should not fail");
    assert!(
        matches!(event, MachineEvent::StateChanged),
        "expected StateChanged, got {event:?}"
    );

    cancel.cancel();
    let _ = tokio::time::timeout(Duration::from_secs(2), listener_handle).await;
}

#[tokio::test]
#[ignore]
async fn update_machine_target_closure_emits_event() {
    let ctx = test_app_with_auth().await;
    let pool = ctx.db.pool.clone();

    let bus = EventBus::new();
    let cancel = CancellationToken::new();
    let listener_handle = {
        let pool = pool.clone();
        let bus = bus.clone();
        let cancel = cancel.clone();
        tokio::spawn(async move {
            hearth_api::events::run_listener(pool, bus, cancel).await;
        })
    };
    tokio::time::sleep(Duration::from_millis(200)).await;

    let machine = create_machine_http_authed(&ctx, "events-update-host").await;
    let mut rx = bus.subscribe(machine.id);

    // The PUT /machines/{id} path that the deployment monitor uses to
    // assign a new target_closure must trigger the push fan-out.
    let admin_token = ctx.mint_user_jwt("admin-fixture", "admin-fixture", &["hearth-admins"]);
    let uri = format!("/api/v1/machines/{}", machine.id);
    let body = serde_json::json!({ "target_closure": "/nix/store/events-update-target" });
    let status = send_status(&ctx.router, "PUT", &uri, Some(body), Some(&admin_token)).await;
    assert_eq!(status, 200, "expected 200 from admin PUT, got {status}");

    let event = tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await
        .expect("expected state_changed event within 5s")
        .expect("broadcast send should not fail");
    assert!(matches!(event, MachineEvent::StateChanged));

    cancel.cancel();
    let _ = tokio::time::timeout(Duration::from_secs(2), listener_handle).await;
}

#[tokio::test]
async fn event_bus_subscribe_publish_unit() {
    // Pure unit test that doesn't need a database — exercises the
    // EventBus API surface the tests above rely on.
    let bus = EventBus::new();
    let machine_id = Uuid::new_v4();

    let mut rx = bus.subscribe(machine_id);
    assert!(rx.try_recv().is_err(), "no events yet");

    let delivered = bus.publish(machine_id, MachineEvent::StateChanged);
    assert_eq!(delivered, 1);

    let event = rx.try_recv().unwrap();
    assert!(matches!(event, MachineEvent::StateChanged));
}

/// Suppress the unused-import lints when the database-gated tests are
/// filtered out of a `cargo test` run.
#[allow(dead_code)]
fn _imports_referenced() {
    let _: Arc<EventBus> = EventBus::new();
}
