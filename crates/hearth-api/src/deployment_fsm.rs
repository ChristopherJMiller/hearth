//! Deployment state machine: valid transitions and state advancement.

use crate::db::DeploymentStatusDb;

/// Valid state transitions for a deployment.
///
/// ```text
/// pending → canary → rolling → completed
///         ↘        ↘        ↘
///           rolled_back (from any active state)
///         ↘        ↘        ↘
///              failed (from any active state)
/// ```
pub fn is_valid_transition(from: DeploymentStatusDb, to: DeploymentStatusDb) -> bool {
    matches!(
        (from, to),
        // Normal progression
        (DeploymentStatusDb::Pending, DeploymentStatusDb::Canary)
            | (DeploymentStatusDb::Canary, DeploymentStatusDb::Rolling)
            | (DeploymentStatusDb::Rolling, DeploymentStatusDb::Completed)
            // Skip canary (direct to rolling)
            | (DeploymentStatusDb::Pending, DeploymentStatusDb::Rolling)
            // Rollback from any active state
            | (DeploymentStatusDb::Pending, DeploymentStatusDb::RolledBack)
            | (DeploymentStatusDb::Canary, DeploymentStatusDb::RolledBack)
            | (DeploymentStatusDb::Rolling, DeploymentStatusDb::RolledBack)
            // Failure from any active state
            | (DeploymentStatusDb::Pending, DeploymentStatusDb::Failed)
            | (DeploymentStatusDb::Canary, DeploymentStatusDb::Failed)
            | (DeploymentStatusDb::Rolling, DeploymentStatusDb::Failed)
    )
}

/// Determine the next state for a deployment that should advance.
///
/// Returns `None` if the deployment is already in a terminal state.
#[allow(dead_code)]
pub fn next_state(current: DeploymentStatusDb) -> Option<DeploymentStatusDb> {
    match current {
        DeploymentStatusDb::Pending => Some(DeploymentStatusDb::Canary),
        DeploymentStatusDb::Canary => Some(DeploymentStatusDb::Rolling),
        DeploymentStatusDb::Rolling => Some(DeploymentStatusDb::Completed),
        // Terminal states
        DeploymentStatusDb::Completed
        | DeploymentStatusDb::Failed
        | DeploymentStatusDb::RolledBack => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_forward_transitions() {
        assert!(is_valid_transition(
            DeploymentStatusDb::Pending,
            DeploymentStatusDb::Canary
        ));
        assert!(is_valid_transition(
            DeploymentStatusDb::Canary,
            DeploymentStatusDb::Rolling
        ));
        assert!(is_valid_transition(
            DeploymentStatusDb::Rolling,
            DeploymentStatusDb::Completed
        ));
    }

    #[test]
    fn valid_rollback_transitions() {
        assert!(is_valid_transition(
            DeploymentStatusDb::Canary,
            DeploymentStatusDb::RolledBack
        ));
        assert!(is_valid_transition(
            DeploymentStatusDb::Rolling,
            DeploymentStatusDb::RolledBack
        ));
    }

    #[test]
    fn invalid_backward_transitions() {
        assert!(!is_valid_transition(
            DeploymentStatusDb::Rolling,
            DeploymentStatusDb::Canary
        ));
        assert!(!is_valid_transition(
            DeploymentStatusDb::Completed,
            DeploymentStatusDb::Rolling
        ));
        assert!(!is_valid_transition(
            DeploymentStatusDb::RolledBack,
            DeploymentStatusDb::Pending
        ));
    }

    #[test]
    fn next_state_progression() {
        assert_eq!(
            next_state(DeploymentStatusDb::Pending),
            Some(DeploymentStatusDb::Canary)
        );
        assert_eq!(
            next_state(DeploymentStatusDb::Canary),
            Some(DeploymentStatusDb::Rolling)
        );
        assert_eq!(
            next_state(DeploymentStatusDb::Rolling),
            Some(DeploymentStatusDb::Completed)
        );
        assert_eq!(next_state(DeploymentStatusDb::Completed), None);
        assert_eq!(next_state(DeploymentStatusDb::RolledBack), None);
    }
}
