// hearth-office config — reads ~/.config/hearth/office.toml
//
// This file is written by the home-manager libreoffice module and provides
// the Nextcloud URL and WebDAV base URL for the extension's API calls.

use serde::Deserialize;
use std::path::PathBuf;

/// Top-level extension configuration from office.toml.
#[derive(Debug, Deserialize)]
pub struct OfficeConfig {
    pub nextcloud: NextcloudConfig,
}

/// Nextcloud server configuration.
#[derive(Debug, Deserialize)]
pub struct NextcloudConfig {
    /// Base URL of the Nextcloud instance (e.g., "https://cloud.hearth.example.com").
    pub url: String,

    /// WebDAV base URL for file access (e.g., "https://cloud.hearth.example.com/remote.php/dav/files/").
    pub webdav_url: String,
}

impl OfficeConfig {
    /// Load configuration from the standard path (~/.config/hearth/office.toml).
    pub fn load() -> Result<Self, ConfigError> {
        let path = config_path()?;
        let content = std::fs::read_to_string(&path)
            .map_err(|e| ConfigError::ReadFailed { path: path.clone(), source: e })?;
        let config: Self = toml::from_str(&content)
            .map_err(|e| ConfigError::ParseFailed { path, source: e })?;
        Ok(config)
    }

    /// Load from a specific path (for testing).
    pub fn load_from(path: &std::path::Path) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| ConfigError::ReadFailed { path: path.to_path_buf(), source: e })?;
        let config: Self = toml::from_str(&content)
            .map_err(|e| ConfigError::ParseFailed { path: path.to_path_buf(), source: e })?;
        Ok(config)
    }
}

fn config_path() -> Result<PathBuf, ConfigError> {
    let config_dir = dirs::config_dir()
        .ok_or(ConfigError::NoConfigDir)?;
    Ok(config_dir.join("hearth").join("office.toml"))
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("could not determine XDG config directory")]
    NoConfigDir,

    #[error("failed to read config file at {path}: {source}")]
    ReadFailed {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to parse config file at {path}: {source}")]
    ParseFailed {
        path: PathBuf,
        source: toml::de::Error,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn parse_minimal_config() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("office.toml");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(
            f,
            r#"
[nextcloud]
url = "https://cloud.example.com"
webdav_url = "https://cloud.example.com/remote.php/dav/files/"
"#
        )
        .unwrap();

        let config = OfficeConfig::load_from(&path).unwrap();
        assert_eq!(config.nextcloud.url, "https://cloud.example.com");
        assert!(config.nextcloud.webdav_url.ends_with("/dav/files/"));
    }

    #[test]
    fn missing_file_returns_error() {
        let path = std::path::Path::new("/nonexistent/office.toml");
        let result = OfficeConfig::load_from(path);
        assert!(result.is_err());
    }

    #[test]
    fn invalid_toml_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("office.toml");
        std::fs::write(&path, "not valid toml {{{}}}").unwrap();
        let result = OfficeConfig::load_from(&path);
        assert!(result.is_err());
    }
}
