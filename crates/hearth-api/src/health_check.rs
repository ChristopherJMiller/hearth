//! Health checking for deployment batches.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use tracing::debug;
use uuid::Uuid;

use crate::db::MachineUpdateStatusDb;
use crate::repo;

/// Health status of a batch of machines in a deployment.
#[derive(Debug, Clone)]
pub struct BatchHealth {
    pub total: usize,
    pub completed: usize,
    pub failed: usize,
    pub in_progress: usize,
    pub pending: usize,
}

impl BatchHealth {
    /// Fraction of machines that have failed.
    pub fn failure_rate(&self) -> f64 {
        if self.total == 0 {
            return 0.0;
        }
        self.failed as f64 / self.total as f64
    }

    /// Whether all machines have reached a terminal state.
    pub fn is_complete(&self) -> bool {
        self.pending == 0 && self.in_progress == 0
    }
}

/// Check the health of all machines in a deployment.
pub async fn check_deployment_health(
    pool: &PgPool,
    deployment_id: Uuid,
) -> Result<BatchHealth, sqlx::Error> {
    let machines = repo::get_deployment_machines(pool, deployment_id).await?;

    let mut health = BatchHealth {
        total: machines.len(),
        completed: 0,
        failed: 0,
        in_progress: 0,
        pending: 0,
    };

    for m in &machines {
        match m.status {
            MachineUpdateStatusDb::Completed => health.completed += 1,
            MachineUpdateStatusDb::Failed | MachineUpdateStatusDb::RolledBack => health.failed += 1,
            MachineUpdateStatusDb::Downloading | MachineUpdateStatusDb::Switching => {
                health.in_progress += 1
            }
            MachineUpdateStatusDb::Pending => health.pending += 1,
        }
    }

    debug!(
        deployment_id = %deployment_id,
        total = health.total,
        completed = health.completed,
        failed = health.failed,
        in_progress = health.in_progress,
        "batch health check"
    );

    Ok(health)
}

/// Check if machines have recent heartbeats (within the given window).
pub async fn check_heartbeat_recency(
    pool: &PgPool,
    deployment_id: Uuid,
    max_age: chrono::Duration,
) -> Result<(usize, usize), sqlx::Error> {
    let machines = repo::get_deployment_machines(pool, deployment_id).await?;
    let now = Utc::now();
    let mut recent = 0;
    let mut stale = 0;

    for dm in &machines {
        // Look up the machine's last heartbeat
        if let Some(machine) = repo::get_machine(pool, dm.machine_id).await? {
            let machine: hearth_common::api_types::Machine = machine.into();
            if let Some(last_hb) = machine.last_heartbeat {
                if is_recent(last_hb, now, max_age) {
                    recent += 1;
                } else {
                    stale += 1;
                }
            } else {
                stale += 1;
            }
        }
    }

    Ok((recent, stale))
}

fn is_recent(heartbeat: DateTime<Utc>, now: DateTime<Utc>, max_age: chrono::Duration) -> bool {
    now.signed_duration_since(heartbeat) < max_age
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_failure_rate_empty() {
        let h = BatchHealth {
            total: 0,
            completed: 0,
            failed: 0,
            in_progress: 0,
            pending: 0,
        };
        assert_eq!(h.failure_rate(), 0.0);
    }

    #[test]
    fn test_failure_rate_no_failures() {
        let h = BatchHealth {
            total: 5,
            completed: 5,
            failed: 0,
            in_progress: 0,
            pending: 0,
        };
        assert_eq!(h.failure_rate(), 0.0);
    }

    #[test]
    fn test_failure_rate_some_failures() {
        let h = BatchHealth {
            total: 10,
            completed: 7,
            failed: 3,
            in_progress: 0,
            pending: 0,
        };
        assert!((h.failure_rate() - 0.3).abs() < f64::EPSILON);
    }

    #[test]
    fn test_is_complete() {
        let done = BatchHealth {
            total: 5,
            completed: 4,
            failed: 1,
            in_progress: 0,
            pending: 0,
        };
        assert!(done.is_complete());

        let in_progress = BatchHealth {
            total: 5,
            completed: 3,
            failed: 0,
            in_progress: 2,
            pending: 0,
        };
        assert!(!in_progress.is_complete());

        let has_pending = BatchHealth {
            total: 5,
            completed: 3,
            failed: 0,
            in_progress: 0,
            pending: 2,
        };
        assert!(!has_pending.is_complete());
    }

    #[test]
    fn test_is_recent_within_window() {
        let now = Utc::now();
        let hb = now - chrono::Duration::minutes(3);
        assert!(is_recent(hb, now, chrono::Duration::minutes(5)));
    }

    #[test]
    fn test_is_recent_outside_window() {
        let now = Utc::now();
        let hb = now - chrono::Duration::minutes(10);
        assert!(!is_recent(hb, now, chrono::Duration::minutes(5)));
    }
}
