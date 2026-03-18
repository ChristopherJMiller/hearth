use crate::common::{send_json, send_status, test_app};
use hearth_common::api_types::Machine;
use serde_json::json;
use uuid::Uuid;

#[tokio::test]
#[ignore]
async fn create_and_get_machine() {
    let (app, _db) = test_app().await;

    let body = json!({
        "hostname": "test-workstation-01",
        "role": "developer",
        "tags": ["office-a"]
    });

    let (status, machine): (_, Machine) =
        send_json(&app, "POST", "/api/v1/machines", Some(body), None).await;
    assert_eq!(status, 201);
    assert_eq!(machine.hostname, "test-workstation-01");
    assert_eq!(machine.role.as_deref(), Some("developer"));

    // GET by id
    let uri = format!("/api/v1/machines/{}", machine.id);
    let (status, fetched): (_, Machine) = send_json(&app, "GET", &uri, None, None).await;
    assert_eq!(status, 200);
    assert_eq!(fetched.id, machine.id);
    assert_eq!(fetched.hostname, "test-workstation-01");
}

#[tokio::test]
#[ignore]
async fn list_machines() {
    let (app, _db) = test_app().await;

    // Create two machines
    for name in ["machine-a", "machine-b"] {
        let body = json!({ "hostname": name });
        let (status, _): (_, Machine) =
            send_json(&app, "POST", "/api/v1/machines", Some(body), None).await;
        assert_eq!(status, 201);
    }

    let (status, machines): (_, Vec<Machine>) =
        send_json(&app, "GET", "/api/v1/machines", None, None).await;
    assert_eq!(status, 200);
    assert!(machines.len() >= 2);
}

#[tokio::test]
#[ignore]
async fn update_machine() {
    let (app, _db) = test_app().await;

    let body = json!({ "hostname": "original-name" });
    let (_, machine): (_, Machine) =
        send_json(&app, "POST", "/api/v1/machines", Some(body), None).await;

    let uri = format!("/api/v1/machines/{}", machine.id);
    let update = json!({
        "hostname": "updated-name",
        "tags": ["floor-2", "engineering"]
    });
    let (status, updated): (_, Machine) = send_json(&app, "PUT", &uri, Some(update), None).await;
    assert_eq!(status, 200);
    assert_eq!(updated.hostname, "updated-name");
    assert_eq!(updated.tags, vec!["floor-2", "engineering"]);
}

#[tokio::test]
#[ignore]
async fn delete_machine() {
    let (app, _db) = test_app().await;

    let body = json!({ "hostname": "to-delete" });
    let (_, machine): (_, Machine) =
        send_json(&app, "POST", "/api/v1/machines", Some(body), None).await;

    let uri = format!("/api/v1/machines/{}", machine.id);
    let status = send_status(&app, "DELETE", &uri, None, None).await;
    assert_eq!(status, 204);

    // Verify it's gone
    let status = send_status(&app, "GET", &uri, None, None).await;
    assert_eq!(status, 404);
}

#[tokio::test]
#[ignore]
async fn get_nonexistent_machine_returns_404() {
    let (app, _db) = test_app().await;
    let uri = format!("/api/v1/machines/{}", Uuid::new_v4());
    let status = send_status(&app, "GET", &uri, None, None).await;
    assert_eq!(status, 404);
}

#[tokio::test]
#[ignore]
async fn delete_nonexistent_machine_returns_404() {
    let (app, _db) = test_app().await;
    let uri = format!("/api/v1/machines/{}", Uuid::new_v4());
    let status = send_status(&app, "DELETE", &uri, None, None).await;
    assert_eq!(status, 404);
}
