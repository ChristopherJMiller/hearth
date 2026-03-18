use crate::common::{send_json, send_status, test_app};
use hearth_common::api_types::{Deployment, DeploymentStatus};
use serde_json::json;
use uuid::Uuid;

#[tokio::test]
#[ignore]
async fn create_and_get_deployment() {
    let (app, _db) = test_app().await;

    let body = json!({
        "closure": "/nix/store/abc123-nixos-system-24.05",
        "target_filter": { "role": "developer" },
        "canary_size": 2,
        "batch_size": 5,
        "failure_threshold": 0.1
    });
    let (status, deployment): (_, Deployment) =
        send_json(&app, "POST", "/api/v1/deployments", Some(body)).await;
    assert_eq!(status, 201);
    assert_eq!(deployment.closure, "/nix/store/abc123-nixos-system-24.05");
    assert_eq!(deployment.status, DeploymentStatus::Pending);
    assert_eq!(deployment.canary_size, 2);

    // Get by id
    let uri = format!("/api/v1/deployments/{}", deployment.id);
    let (status, fetched): (_, Deployment) = send_json(&app, "GET", &uri, None).await;
    assert_eq!(status, 200);
    assert_eq!(fetched.id, deployment.id);
}

#[tokio::test]
#[ignore]
async fn list_deployments() {
    let (app, _db) = test_app().await;

    let body = json!({ "closure": "/nix/store/deploy1-nixos" });
    let (status, _): (_, Deployment) =
        send_json(&app, "POST", "/api/v1/deployments", Some(body)).await;
    assert_eq!(status, 201);

    let (status, list): (_, Vec<Deployment>) =
        send_json(&app, "GET", "/api/v1/deployments", None).await;
    assert_eq!(status, 200);
    assert!(!list.is_empty());
}

#[tokio::test]
#[ignore]
async fn deployment_status_transitions() {
    let (app, _db) = test_app().await;

    let body = json!({ "closure": "/nix/store/fsm-test-nixos" });
    let (_, deployment): (_, Deployment) =
        send_json(&app, "POST", "/api/v1/deployments", Some(body)).await;
    assert_eq!(deployment.status, DeploymentStatus::Pending);

    let status_uri = format!("/api/v1/deployments/{}/status", deployment.id);

    // pending → canary (valid)
    let update = json!({ "status": "canary" });
    let (status, updated): (_, Deployment) =
        send_json(&app, "PUT", &status_uri, Some(update)).await;
    assert_eq!(status, 200);
    assert_eq!(updated.status, DeploymentStatus::Canary);

    // canary → rolling (valid)
    let update = json!({ "status": "rolling" });
    let (status, updated): (_, Deployment) =
        send_json(&app, "PUT", &status_uri, Some(update)).await;
    assert_eq!(status, 200);
    assert_eq!(updated.status, DeploymentStatus::Rolling);

    // rolling → completed (valid)
    let update = json!({ "status": "completed" });
    let (status, updated): (_, Deployment) =
        send_json(&app, "PUT", &status_uri, Some(update)).await;
    assert_eq!(status, 200);
    assert_eq!(updated.status, DeploymentStatus::Completed);
}

#[tokio::test]
#[ignore]
async fn invalid_deployment_transition_rejected() {
    let (app, _db) = test_app().await;

    let body = json!({ "closure": "/nix/store/invalid-fsm" });
    let (_, deployment): (_, Deployment) =
        send_json(&app, "POST", "/api/v1/deployments", Some(body)).await;

    let status_uri = format!("/api/v1/deployments/{}/status", deployment.id);

    // pending → completed (invalid — must go through canary/rolling)
    let update = json!({ "status": "completed" });
    let status = send_status(&app, "PUT", &status_uri, Some(update)).await;
    assert_eq!(status, 400);
}

#[tokio::test]
#[ignore]
async fn rollback_deployment() {
    let (app, _db) = test_app().await;

    let body = json!({ "closure": "/nix/store/rollback-test" });
    let (_, deployment): (_, Deployment) =
        send_json(&app, "POST", "/api/v1/deployments", Some(body)).await;

    // Advance to canary
    let status_uri = format!("/api/v1/deployments/{}/status", deployment.id);
    let update = json!({ "status": "canary" });
    let (_, _): (_, Deployment) = send_json(&app, "PUT", &status_uri, Some(update)).await;

    // Rollback from canary
    let rollback_uri = format!("/api/v1/deployments/{}/rollback", deployment.id);
    let (status, rolled_back): (_, Deployment) =
        send_json(&app, "POST", &rollback_uri, None).await;
    assert_eq!(status, 200);
    assert_eq!(rolled_back.status, DeploymentStatus::RolledBack);
}

#[tokio::test]
#[ignore]
async fn get_nonexistent_deployment_returns_404() {
    let (app, _db) = test_app().await;
    let uri = format!("/api/v1/deployments/{}", Uuid::new_v4());
    let status = send_status(&app, "GET", &uri, None).await;
    assert_eq!(status, 404);
}
