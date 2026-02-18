//! System update logic.
//!
//! In Phase 1 this is a stub that compares closure paths and logs what
//! *would* happen. Phase 2 will shell out to `nix copy` and
//! `nixos-rebuild switch`.

use tracing::{debug, info, warn};

/// Errors that can occur during an update attempt.
#[derive(Debug, thiserror::Error)]
#[allow(dead_code)]
pub enum UpdateError {
    #[error("update command failed: {0}")]
    CommandFailed(String),
    #[error("invalid store path: {0}")]
    InvalidStorePath(String),
}

/// Compare the current system closure with the target and, if they differ,
/// apply the update.
///
/// Returns `Ok(true)` if an update was applied (or would be applied in this
/// stub), `Ok(false)` if no update was needed.
pub async fn check_and_apply_update(
    current_closure: Option<&str>,
    target_closure: &str,
) -> Result<bool, UpdateError> {
    // If we already have the target closure, nothing to do.
    if let Some(current) = current_closure {
        if current == target_closure {
            debug!(
                closure = target_closure,
                "system is already at the target closure"
            );
            return Ok(false);
        }

        info!(
            from = current,
            to = target_closure,
            "update available: would switch closure"
        );
    } else {
        info!(
            to = target_closure,
            "update available: no current closure recorded, would switch"
        );
    }

    // --- Phase 2 will do the real work here: ---
    // 1. nix copy --from <cache> <target_closure>
    // 2. nix-env --profile /nix/var/nix/profiles/system --set <target_closure>
    // 3. <target_closure>/bin/switch-to-configuration switch
    //
    // For now, just log and pretend it succeeded.
    warn!(
        target_closure,
        "STUB: update not actually applied (Phase 1)"
    );

    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn no_update_when_same() {
        let result =
            check_and_apply_update(Some("/nix/store/aaaa-system"), "/nix/store/aaaa-system")
                .await
                .unwrap();
        assert!(!result);
    }

    #[tokio::test]
    async fn update_when_different() {
        let result =
            check_and_apply_update(Some("/nix/store/aaaa-system"), "/nix/store/bbbb-system")
                .await
                .unwrap();
        assert!(result);
    }

    #[tokio::test]
    async fn update_when_no_current() {
        let result = check_and_apply_update(None, "/nix/store/bbbb-system")
            .await
            .unwrap();
        assert!(result);
    }
}
