use crate::common::{send_json, send_status, test_app};
use hearth_common::api_types::{CatalogEntry, Machine, SoftwareRequest};
use serde_json::json;

#[tokio::test]
#[ignore]
async fn catalog_crud() {
    let (app, _db) = test_app().await;

    // Create
    let body = json!({
        "name": "Firefox",
        "description": "Web browser",
        "category": "Internet",
        "install_method": "flatpak",
        "flatpak_ref": "org.mozilla.firefox",
        "approval_required": false
    });
    let (status, entry): (_, CatalogEntry) =
        send_json(&app, "POST", "/api/v1/catalog", Some(body), None).await;
    assert_eq!(status, 201);
    assert_eq!(entry.name, "Firefox");
    assert!(!entry.approval_required);

    // Get
    let uri = format!("/api/v1/catalog/{}", entry.id);
    let (status, fetched): (_, CatalogEntry) = send_json(&app, "GET", &uri, None, None).await;
    assert_eq!(status, 200);
    assert_eq!(fetched.name, "Firefox");

    // Update
    let update = json!({ "name": "Firefox ESR", "approval_required": true });
    let (status, updated): (_, CatalogEntry) = send_json(&app, "PUT", &uri, Some(update), None).await;
    assert_eq!(status, 200);
    assert_eq!(updated.name, "Firefox ESR");
    assert!(updated.approval_required);

    // List
    let (status, list): (_, Vec<CatalogEntry>) =
        send_json(&app, "GET", "/api/v1/catalog", None, None).await;
    assert_eq!(status, 200);
    assert!(list.iter().any(|e| e.id == entry.id));

    // Delete
    let status = send_status(&app, "DELETE", &uri, None, None).await;
    assert_eq!(status, 204);

    // Verify deleted
    let status = send_status(&app, "GET", &uri, None, None).await;
    assert_eq!(status, 404);
}

#[tokio::test]
#[ignore]
async fn request_software() {
    let (app, _db) = test_app().await;

    // Create a catalog entry
    let cat_body = json!({
        "name": "VS Code",
        "install_method": "flatpak",
        "flatpak_ref": "com.visualstudio.code",
        "approval_required": true
    });
    let (_, entry): (_, CatalogEntry) =
        send_json(&app, "POST", "/api/v1/catalog", Some(cat_body), None).await;

    // Create a machine to request for
    let machine_body = json!({ "hostname": "dev-workstation" });
    let (_, machine): (_, Machine) =
        send_json(&app, "POST", "/api/v1/machines", Some(machine_body), None).await;

    // Request the software
    let req_uri = format!("/api/v1/catalog/{}/request", entry.id);
    let req_body = json!({
        "machine_id": machine.id,
        "username": "testuser"
    });
    let (status, request): (_, SoftwareRequest) =
        send_json(&app, "POST", &req_uri, Some(req_body), None).await;
    assert_eq!(status, 201);
    assert_eq!(request.catalog_entry_id, entry.id);
    assert_eq!(request.machine_id, machine.id);
    assert_eq!(request.username, "testuser");
}
