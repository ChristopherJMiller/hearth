use crate::common::{send_json, send_status, test_app};
use hearth_common::api_types::{HeartbeatResponse, Machine};
use serde_json::json;
use uuid::Uuid;

#[tokio::test]
#[ignore]
async fn heartbeat_updates_last_seen() {
    let (app, _db) = test_app().await;

    // Create a machine first
    let body = json!({ "hostname": "heartbeat-test" });
    let (_, machine): (_, Machine) =
        send_json(&app, "POST", "/api/v1/machines", Some(body)).await;

    // Send heartbeat
    let hb = json!({
        "machine_id": machine.id,
        "current_closure": "/nix/store/abc123-nixos-system-24.05",
        "os_version": "NixOS 24.05",
        "uptime_seconds": 3600
    });
    let (status, resp): (_, HeartbeatResponse) =
        send_json(&app, "POST", "/api/v1/heartbeat", Some(hb)).await;
    assert_eq!(status, 200);

    // Verify machine's last_heartbeat is now set
    let uri = format!("/api/v1/machines/{}", machine.id);
    let (_, updated): (_, Machine) = send_json(&app, "GET", &uri, None).await;
    assert!(updated.last_heartbeat.is_some());

    // pending_installs should be an empty list for a fresh machine
    assert!(resp.pending_installs.is_empty());
}

#[tokio::test]
#[ignore]
async fn heartbeat_returns_target_closure() {
    let (app, _db) = test_app().await;

    let body = json!({ "hostname": "target-test" });
    let (_, machine): (_, Machine) =
        send_json(&app, "POST", "/api/v1/machines", Some(body)).await;

    // Set a target closure on the machine
    let uri = format!("/api/v1/machines/{}", machine.id);
    let update = json!({
        "target_closure": "/nix/store/xyz789-nixos-system-24.05"
    });
    let (_, _): (_, Machine) = send_json(&app, "PUT", &uri, Some(update)).await;

    // Heartbeat should return the target closure
    let hb = json!({ "machine_id": machine.id });
    let (_, resp): (_, HeartbeatResponse) =
        send_json(&app, "POST", "/api/v1/heartbeat", Some(hb)).await;
    assert_eq!(
        resp.target_closure.as_deref(),
        Some("/nix/store/xyz789-nixos-system-24.05")
    );
}

#[tokio::test]
#[ignore]
async fn heartbeat_unknown_machine_returns_404() {
    let (app, _db) = test_app().await;

    let hb = json!({ "machine_id": Uuid::new_v4() });
    let status = send_status(&app, "POST", "/api/v1/heartbeat", Some(hb)).await;
    assert_eq!(status, 404);
}
