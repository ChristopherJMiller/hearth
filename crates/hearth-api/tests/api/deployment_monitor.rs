//! Integration tests for the deployment monitor logic (rollout, health checks).
//!
//! These tests exercise the rollout controller and health-check functions against
//! a real PostgreSQL database with test fixtures.

use crate::common::{
    TestDb, add_machines_to_deployment, create_test_deployment, create_test_machine,
};
use hearth_api::db::{DeploymentStatusDb, MachineUpdateStatusDb};
use hearth_api::health_check;
use hearth_api::repo;
use hearth_api::rollout;

// ── Health check tests ──────────────────────────────────────

#[tokio::test]
#[ignore]
async fn health_check_empty_deployment() {
    let db = TestDb::new().await;
    let dep = create_test_deployment(&db.pool, "/nix/store/test-system", 1, 5, 0.1).await;

    let health = health_check::check_deployment_health(&db.pool, dep.id)
        .await
        .unwrap();
    assert_eq!(health.total, 0);
    assert_eq!(health.failure_rate(), 0.0);
    assert!(health.is_complete());
}

#[tokio::test]
#[ignore]
async fn health_check_mixed_statuses() {
    let db = TestDb::new().await;
    let dep = create_test_deployment(&db.pool, "/nix/store/test-system", 1, 5, 0.1).await;

    let m1 = create_test_machine(&db.pool, "host-1").await;
    let m2 = create_test_machine(&db.pool, "host-2").await;
    let m3 = create_test_machine(&db.pool, "host-3").await;

    // m1 completed, m2 failed, m3 downloading (in_progress)
    repo::upsert_deployment_machine(&db.pool, dep.id, m1, MachineUpdateStatusDb::Completed, None)
        .await
        .unwrap();
    repo::upsert_deployment_machine(
        &db.pool,
        dep.id,
        m2,
        MachineUpdateStatusDb::Failed,
        Some("oops"),
    )
    .await
    .unwrap();
    repo::upsert_deployment_machine(
        &db.pool,
        dep.id,
        m3,
        MachineUpdateStatusDb::Downloading,
        None,
    )
    .await
    .unwrap();

    let health = health_check::check_deployment_health(&db.pool, dep.id)
        .await
        .unwrap();
    assert_eq!(health.total, 3);
    assert_eq!(health.completed, 1);
    assert_eq!(health.failed, 1);
    assert_eq!(health.in_progress, 1);
    assert!(!health.is_complete());
    assert!((health.failure_rate() - 1.0 / 3.0).abs() < 0.01);
}

// ── Canary selection and validation ─────────────────────────

#[tokio::test]
#[ignore]
async fn select_canary_machines_respects_size() {
    let db = TestDb::new().await;
    let dep = create_test_deployment(&db.pool, "/nix/store/test", 2, 5, 0.1).await;

    let m1 = create_test_machine(&db.pool, "host-1").await;
    let m2 = create_test_machine(&db.pool, "host-2").await;
    let m3 = create_test_machine(&db.pool, "host-3").await;
    add_machines_to_deployment(&db.pool, dep.id, &[m1, m2, m3]).await;

    let canaries = rollout::select_canary_machines(&db.pool, dep.id, 2)
        .await
        .unwrap();
    assert_eq!(canaries.len(), 2);
}

#[tokio::test]
#[ignore]
async fn validate_canary_passes_when_healthy() {
    let db = TestDb::new().await;
    let dep = create_test_deployment(&db.pool, "/nix/store/test", 1, 5, 0.5).await;

    let m1 = create_test_machine(&db.pool, "canary-1").await;
    repo::upsert_deployment_machine(&db.pool, dep.id, m1, MachineUpdateStatusDb::Completed, None)
        .await
        .unwrap();

    let healthy = rollout::validate_canary(&db.pool, dep.id, 0.5)
        .await
        .unwrap();
    assert!(healthy);
}

#[tokio::test]
#[ignore]
async fn validate_canary_fails_above_threshold() {
    let db = TestDb::new().await;
    let dep = create_test_deployment(&db.pool, "/nix/store/test", 2, 5, 0.1).await;

    let m1 = create_test_machine(&db.pool, "canary-1").await;
    let m2 = create_test_machine(&db.pool, "canary-2").await;

    // 1 completed, 1 failed = 50% failure rate, threshold is 10%
    repo::upsert_deployment_machine(&db.pool, dep.id, m1, MachineUpdateStatusDb::Completed, None)
        .await
        .unwrap();
    repo::upsert_deployment_machine(
        &db.pool,
        dep.id,
        m2,
        MachineUpdateStatusDb::Failed,
        Some("error"),
    )
    .await
    .unwrap();

    let healthy = rollout::validate_canary(&db.pool, dep.id, 0.1)
        .await
        .unwrap();
    assert!(!healthy);
}

// ── Advance to rolling ──────────────────────────────────────

#[tokio::test]
#[ignore]
async fn advance_to_rolling_on_healthy_canary() {
    let db = TestDb::new().await;
    let dep = create_test_deployment(&db.pool, "/nix/store/test", 1, 5, 0.5).await;

    // Set deployment to Canary status
    repo::update_deployment_status(&db.pool, dep.id, DeploymentStatusDb::Canary)
        .await
        .unwrap();

    // Add a completed canary machine
    let m1 = create_test_machine(&db.pool, "canary-1").await;
    repo::upsert_deployment_machine(&db.pool, dep.id, m1, MachineUpdateStatusDb::Completed, None)
        .await
        .unwrap();

    // Should advance to Rolling
    rollout::advance_to_rolling(&db.pool, dep.id).await.unwrap();

    let updated = repo::get_deployment(&db.pool, dep.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated.status, DeploymentStatusDb::Rolling);
}

#[tokio::test]
#[ignore]
async fn advance_to_rolling_triggers_rollback_on_failure() {
    let db = TestDb::new().await;
    let dep = create_test_deployment(&db.pool, "/nix/store/test", 1, 5, 0.1).await;

    repo::update_deployment_status(&db.pool, dep.id, DeploymentStatusDb::Canary)
        .await
        .unwrap();

    // Canary machine failed
    let m1 = create_test_machine(&db.pool, "canary-1").await;
    repo::upsert_deployment_machine(
        &db.pool,
        dep.id,
        m1,
        MachineUpdateStatusDb::Failed,
        Some("error"),
    )
    .await
    .unwrap();

    // Should return ThresholdExceeded error
    let result = rollout::advance_to_rolling(&db.pool, dep.id).await;
    assert!(matches!(
        result,
        Err(rollout::RolloutError::ThresholdExceeded { .. })
    ));
}

// ── Get next batch ──────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn get_next_batch_returns_pending_machines() {
    let db = TestDb::new().await;
    let dep = create_test_deployment(&db.pool, "/nix/store/test", 1, 2, 0.1).await;

    let m1 = create_test_machine(&db.pool, "host-1").await;
    let m2 = create_test_machine(&db.pool, "host-2").await;
    let m3 = create_test_machine(&db.pool, "host-3").await;

    add_machines_to_deployment(&db.pool, dep.id, &[m1, m2, m3]).await;

    // Mark m1 as completed (not pending)
    repo::upsert_deployment_machine(&db.pool, dep.id, m1, MachineUpdateStatusDb::Completed, None)
        .await
        .unwrap();

    let batch = rollout::get_next_batch(&db.pool, dep.id, 2).await.unwrap();
    assert_eq!(batch.len(), 2);
    // Should only contain m2 and m3
    assert!(!batch.contains(&m1));
}

#[tokio::test]
#[ignore]
async fn get_next_batch_empty_when_all_done() {
    let db = TestDb::new().await;
    let dep = create_test_deployment(&db.pool, "/nix/store/test", 1, 5, 0.1).await;

    let m1 = create_test_machine(&db.pool, "host-1").await;
    repo::upsert_deployment_machine(&db.pool, dep.id, m1, MachineUpdateStatusDb::Completed, None)
        .await
        .unwrap();

    let batch = rollout::get_next_batch(&db.pool, dep.id, 5).await.unwrap();
    assert!(batch.is_empty());
}

// ── Trigger rollback ────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn trigger_rollback_marks_pending_as_rolled_back() {
    let db = TestDb::new().await;
    let dep = create_test_deployment(&db.pool, "/nix/store/test", 1, 5, 0.1).await;

    let m1 = create_test_machine(&db.pool, "host-1").await;
    let m2 = create_test_machine(&db.pool, "host-2").await;

    // m1 is pending, m2 is completed
    repo::upsert_deployment_machine(&db.pool, dep.id, m1, MachineUpdateStatusDb::Pending, None)
        .await
        .unwrap();
    repo::upsert_deployment_machine(&db.pool, dep.id, m2, MachineUpdateStatusDb::Completed, None)
        .await
        .unwrap();

    rollout::trigger_rollback(&db.pool, dep.id, "test rollback")
        .await
        .unwrap();

    let machines = repo::get_deployment_machines(&db.pool, dep.id)
        .await
        .unwrap();
    let m1_status = machines.iter().find(|m| m.machine_id == m1).unwrap();
    let m2_status = machines.iter().find(|m| m.machine_id == m2).unwrap();

    // m1 (was pending) should be rolled back
    assert_eq!(m1_status.status, MachineUpdateStatusDb::RolledBack);
    // m2 (was completed) should remain completed
    assert_eq!(m2_status.status, MachineUpdateStatusDb::Completed);
}
