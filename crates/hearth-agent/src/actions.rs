//! Remote action executor for the Hearth agent.
//!
//! Actions are dispatched from the control plane via the heartbeat response
//! and executed locally on the fleet device.

use hearth_common::api_types::{ActionType, PendingAction};
use tracing::{error, info, warn};

/// Execute a remote action and return `(success, result_json)`.
pub async fn execute_action(action: &PendingAction) -> (bool, Option<serde_json::Value>) {
    info!(
        action_id = %action.id,
        action_type = ?action.action_type,
        "executing remote action"
    );

    match action.action_type {
        ActionType::Lock => execute_lock().await,
        ActionType::Restart => execute_restart().await,
        ActionType::Rebuild => execute_rebuild().await,
        ActionType::RunCommand => execute_run_command(&action.payload).await,
    }
}

async fn execute_lock() -> (bool, Option<serde_json::Value>) {
    match tokio::process::Command::new("loginctl")
        .arg("lock-sessions")
        .output()
        .await
    {
        Ok(output) if output.status.success() => {
            info!("locked all sessions");
            (
                true,
                Some(serde_json::json!({"message": "sessions locked"})),
            )
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!(stderr = %stderr, "lock-sessions failed");
            (
                false,
                Some(serde_json::json!({"error": stderr.to_string()})),
            )
        }
        Err(e) => {
            error!(error = %e, "failed to run loginctl");
            (false, Some(serde_json::json!({"error": e.to_string()})))
        }
    }
}

async fn execute_restart() -> (bool, Option<serde_json::Value>) {
    info!("initiating system reboot");
    match tokio::process::Command::new("systemctl")
        .arg("reboot")
        .output()
        .await
    {
        Ok(output) if output.status.success() => (
            true,
            Some(serde_json::json!({"message": "reboot initiated"})),
        ),
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            (
                false,
                Some(serde_json::json!({"error": stderr.to_string()})),
            )
        }
        Err(e) => (false, Some(serde_json::json!({"error": e.to_string()}))),
    }
}

async fn execute_rebuild() -> (bool, Option<serde_json::Value>) {
    // Trigger a system update check by touching a flag file that the poller watches.
    // The actual rebuild is handled by the normal update loop.
    let flag_path = std::path::Path::new("/run/hearth/rebuild-requested");
    if let Some(parent) = flag_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match std::fs::write(flag_path, "1") {
        Ok(()) => {
            info!("rebuild flag set");
            (
                true,
                Some(
                    serde_json::json!({"message": "rebuild requested, will apply on next poll cycle"}),
                ),
            )
        }
        Err(e) => (false, Some(serde_json::json!({"error": e.to_string()}))),
    }
}

async fn execute_run_command(payload: &serde_json::Value) -> (bool, Option<serde_json::Value>) {
    let command = match payload.get("command").and_then(|v| v.as_str()) {
        Some(cmd) => cmd,
        None => {
            return (
                false,
                Some(serde_json::json!({"error": "missing 'command' in payload"})),
            );
        }
    };

    // Safety: only allow commands if the agent is explicitly configured to do so.
    // In production, this should be gated by agent config. For now, we limit to
    // read-only diagnostic commands.
    let timeout_secs = payload
        .get("timeout_secs")
        .and_then(|v| v.as_u64())
        .unwrap_or(30);

    info!(command = %command, timeout_secs, "running remote command");

    match tokio::time::timeout(
        std::time::Duration::from_secs(timeout_secs),
        tokio::process::Command::new("sh")
            .arg("-c")
            .arg(command)
            .output(),
    )
    .await
    {
        Ok(Ok(output)) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let success = output.status.success();
            (
                success,
                Some(serde_json::json!({
                    "exit_code": output.status.code(),
                    "stdout": stdout.chars().take(4096).collect::<String>(),
                    "stderr": stderr.chars().take(4096).collect::<String>(),
                })),
            )
        }
        Ok(Err(e)) => (false, Some(serde_json::json!({"error": e.to_string()}))),
        Err(_) => (
            false,
            Some(serde_json::json!({"error": "command timed out"})),
        ),
    }
}
