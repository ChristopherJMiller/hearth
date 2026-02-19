//! Wraps `nix-eval-jobs` to evaluate a flake and stream NDJSON results.

use serde::Deserialize;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tracing::{debug, error, info};

/// A single evaluation result from `nix-eval-jobs`.
#[derive(Debug, Clone, Deserialize)]
pub struct NixEvalResult {
    pub attr: String,
    #[serde(rename = "drvPath")]
    pub drv_path: String,
    #[serde(default)]
    pub outputs: std::collections::HashMap<String, String>,
    pub system: Option<String>,
    pub error: Option<String>,
}

/// Errors from the evaluation process.
#[derive(Debug, thiserror::Error)]
pub enum EvalError {
    #[error("nix-eval-jobs failed to start: {0}")]
    SpawnFailed(#[source] std::io::Error),
    #[error("nix-eval-jobs exited with code {0}: {1}")]
    ExitFailed(i32, String),
    #[error("failed to parse eval result: {0}")]
    ParseError(String),
}

/// Evaluate a flake reference using `nix-eval-jobs`.
///
/// Returns a list of evaluation results (derivations). Entries with errors
/// are included but have the `error` field set.
pub async fn evaluate_flake(flake_ref: &str) -> Result<Vec<NixEvalResult>, EvalError> {
    info!(flake_ref, "starting nix-eval-jobs");

    let mut child = Command::new("nix-eval-jobs")
        .args(["--flake", flake_ref])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(EvalError::SpawnFailed)?;

    let stdout = child.stdout.take().expect("stdout not captured");
    let reader = BufReader::new(stdout);
    let mut lines = reader.lines();

    let mut results = Vec::new();

    while let Some(line) = lines
        .next_line()
        .await
        .map_err(|e| EvalError::ParseError(format!("failed to read stdout: {e}")))?
    {
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<NixEvalResult>(&line) {
            Ok(result) => {
                if let Some(err) = &result.error {
                    error!(attr = %result.attr, error = %err, "eval error for attribute");
                } else {
                    debug!(attr = %result.attr, drv = %result.drv_path, "evaluated");
                }
                results.push(result);
            }
            Err(e) => {
                error!(line = %line, error = %e, "failed to parse nix-eval-jobs output");
            }
        }
    }

    let status = child.wait().await.map_err(EvalError::SpawnFailed)?;

    if !status.success() {
        let code = status.code().unwrap_or(-1);
        // Read any remaining stderr
        return Err(EvalError::ExitFailed(
            code,
            format!("nix-eval-jobs exited with code {code}"),
        ));
    }

    info!(count = results.len(), "evaluation complete");
    Ok(results)
}
