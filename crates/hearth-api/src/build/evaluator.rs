//! Wraps `nix-eval-jobs` to evaluate a flake and stream NDJSON results.

use serde::Deserialize;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tracing::{debug, error, info};

/// A single evaluation result from `nix-eval-jobs`.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct NixEvalResult {
    pub attr: String,
    #[serde(rename = "drvPath")]
    pub drv_path: String,
    #[allow(dead_code)]
    #[serde(default)]
    pub outputs: std::collections::HashMap<String, String>,
    #[allow(dead_code)]
    pub system: Option<String>,
    pub error: Option<String>,
}

/// Parse a single NDJSON line into a `NixEvalResult`.
///
/// Returns `None` for empty/whitespace lines. Returns `Err` for malformed JSON.
pub(crate) fn parse_eval_line(line: &str) -> Option<Result<NixEvalResult, serde_json::Error>> {
    if line.trim().is_empty() {
        return None;
    }
    Some(serde_json::from_str::<NixEvalResult>(line))
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

    collect_results(&mut lines, &mut results).await?;

    let status = child.wait().await.map_err(EvalError::SpawnFailed)?;

    if !status.success() {
        let code = status.code().unwrap_or(-1);
        return Err(EvalError::ExitFailed(
            code,
            format!("nix-eval-jobs exited with code {code}"),
        ));
    }

    info!(count = results.len(), "evaluation complete");
    Ok(results)
}

/// Evaluate a Nix expression file using `nix-eval-jobs --expr`.
///
/// This is used for per-machine closure builds where we generate an eval.nix
/// wrapper that calls `buildMachineConfig` for each machine. Unlike
/// `evaluate_flake`, this uses `--expr` instead of `--flake`.
pub async fn evaluate_expr(expr_path: &str) -> Result<Vec<NixEvalResult>, EvalError> {
    info!(expr_path, "starting nix-eval-jobs --expr");

    let expr = format!("import {expr_path}");

    let mut child = Command::new("nix-eval-jobs")
        .args(["--expr", &expr])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(EvalError::SpawnFailed)?;

    let stdout = child.stdout.take().expect("stdout not captured");
    let reader = BufReader::new(stdout);
    let mut lines = reader.lines();

    let mut results = Vec::new();

    collect_results(&mut lines, &mut results).await?;

    let status = child.wait().await.map_err(EvalError::SpawnFailed)?;

    if !status.success() {
        let code = status.code().unwrap_or(-1);
        return Err(EvalError::ExitFailed(
            code,
            format!("nix-eval-jobs --expr exited with code {code}"),
        ));
    }

    info!(count = results.len(), "expression evaluation complete");
    Ok(results)
}

/// Read NDJSON lines from a reader, parsing each into a `NixEvalResult`.
async fn collect_results<R: tokio::io::AsyncBufRead + Unpin>(
    lines: &mut tokio::io::Lines<R>,
    results: &mut Vec<NixEvalResult>,
) -> Result<(), EvalError> {
    while let Some(line) = lines
        .next_line()
        .await
        .map_err(|e| EvalError::ParseError(format!("failed to read stdout: {e}")))?
    {
        match parse_eval_line(&line) {
            None => continue,
            Some(Ok(result)) => {
                if let Some(err) = &result.error {
                    error!(attr = %result.attr, error = %err, "eval error for attribute");
                } else {
                    debug!(attr = %result.attr, drv = %result.drv_path, "evaluated");
                }
                results.push(result);
            }
            Some(Err(e)) => {
                error!(line = %line, error = %e, "failed to parse nix-eval-jobs output");
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_eval_result() {
        let line = r#"{"attr":"desk-001","drvPath":"/nix/store/abc-system.drv","system":"x86_64-linux"}"#;
        let result = parse_eval_line(line).unwrap().unwrap();
        assert_eq!(result.attr, "desk-001");
        assert_eq!(result.drv_path, "/nix/store/abc-system.drv");
        assert_eq!(result.system.as_deref(), Some("x86_64-linux"));
        assert!(result.error.is_none());
    }

    #[test]
    fn test_parse_eval_result_with_error() {
        let line = r#"{"attr":"broken","drvPath":"","error":"attribute 'broken' missing"}"#;
        let result = parse_eval_line(line).unwrap().unwrap();
        assert_eq!(result.attr, "broken");
        assert_eq!(
            result.error.as_deref(),
            Some("attribute 'broken' missing")
        );
    }

    #[test]
    fn test_parse_eval_result_with_outputs() {
        let line = r#"{"attr":"desk-001","drvPath":"/nix/store/abc.drv","outputs":{"out":"/nix/store/abc-out"}}"#;
        let result = parse_eval_line(line).unwrap().unwrap();
        assert_eq!(
            result.outputs.get("out").map(String::as_str),
            Some("/nix/store/abc-out")
        );
    }

    #[test]
    fn test_parse_empty_line_skipped() {
        assert!(parse_eval_line("").is_none());
        assert!(parse_eval_line("   ").is_none());
        assert!(parse_eval_line("\t\n").is_none());
    }

    #[test]
    fn test_parse_malformed_json() {
        let result = parse_eval_line("not json at all");
        assert!(result.is_some());
        assert!(result.unwrap().is_err());
    }

    #[test]
    fn test_parse_missing_required_fields() {
        // Missing "attr" and "drvPath"
        let result = parse_eval_line(r#"{"system":"x86_64-linux"}"#);
        assert!(result.is_some());
        assert!(result.unwrap().is_err());
    }

    #[tokio::test]
    async fn test_collect_results_from_stream() {
        let ndjson = concat!(
            r#"{"attr":"host-a","drvPath":"/nix/store/a.drv"}"#, "\n",
            "\n",
            r#"{"attr":"host-b","drvPath":"/nix/store/b.drv","error":"eval failed"}"#, "\n",
            "bad json\n",
        );
        let cursor = std::io::Cursor::new(ndjson.as_bytes().to_vec());
        let reader = BufReader::new(cursor);
        let mut lines = reader.lines();
        let mut results = Vec::new();

        collect_results(&mut lines, &mut results).await.unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].attr, "host-a");
        assert!(results[0].error.is_none());
        assert_eq!(results[1].attr, "host-b");
        assert!(results[1].error.is_some());
        // "bad json" was skipped (logged but not added)
    }
}
