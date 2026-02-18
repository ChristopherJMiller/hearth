//! Configuration loading for the hearth-agent.
//!
//! Reads an [`AgentConfig`] from a TOML file, supporting a CLI-provided path
//! or the default `/etc/hearth/agent.toml`.

use hearth_common::config::AgentConfig;
use std::path::{Path, PathBuf};

/// Default config file location on NixOS systems.
const DEFAULT_CONFIG_PATH: &str = "/etc/hearth/agent.toml";

/// Resolve the config file path from CLI arguments.
///
/// Looks for `--config <path>` in `args`. Falls back to [`DEFAULT_CONFIG_PATH`].
pub fn resolve_config_path(args: &[String]) -> PathBuf {
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if arg == "--config"
            && let Some(path) = iter.next()
        {
            return PathBuf::from(path);
        }
        // Also handle --config=<path>
        if let Some(path) = arg.strip_prefix("--config=") {
            return PathBuf::from(path);
        }
    }
    PathBuf::from(DEFAULT_CONFIG_PATH)
}

/// Load and parse the agent configuration from the given TOML file.
pub fn load_config(path: &Path) -> Result<AgentConfig, Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("failed to read config file {}: {e}", path.display()))?;
    let config: AgentConfig = toml::from_str(&content)
        .map_err(|e| format!("failed to parse config file {}: {e}", path.display()))?;
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_path_when_no_args() {
        let args: Vec<String> = vec!["hearth-agent".into()];
        assert_eq!(
            resolve_config_path(&args),
            PathBuf::from(DEFAULT_CONFIG_PATH)
        );
    }

    #[test]
    fn path_from_flag() {
        let args: Vec<String> = vec![
            "hearth-agent".into(),
            "--config".into(),
            "/tmp/test.toml".into(),
        ];
        assert_eq!(resolve_config_path(&args), PathBuf::from("/tmp/test.toml"));
    }

    #[test]
    fn path_from_equals_form() {
        let args: Vec<String> = vec!["hearth-agent".into(), "--config=/tmp/test.toml".into()];
        assert_eq!(resolve_config_path(&args), PathBuf::from("/tmp/test.toml"));
    }

    #[test]
    fn parse_minimal_config() {
        let toml = r#"
[server]
url = "https://hearth.example.com"
"#;
        let config: AgentConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.server.url, "https://hearth.example.com");
        assert_eq!(config.agent.poll_interval_secs, 60);
        assert_eq!(config.agent.socket_path, "/run/hearth/agent.sock");
    }
}
