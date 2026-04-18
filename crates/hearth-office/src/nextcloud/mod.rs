pub mod comments;
pub mod lock;
pub mod share;

use crate::config::OfficeConfig;
use configparser::ini::Ini;
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::OnceLock;

/// OCS API response metadata, shared across share and comments modules.
#[derive(Debug, Deserialize)]
pub struct OcsMeta {
    pub statuscode: u16,
    pub message: Option<String>,
}

/// Authenticated Nextcloud HTTP client.
pub struct NextcloudClient {
    pub(crate) http: reqwest::blocking::Client,
    pub(crate) base_url: String,
    pub(crate) webdav_url: String,
    pub(crate) username: String,
    password: String,
}

impl NextcloudClient {
    /// Create a new client using stored Nextcloud Desktop credentials.
    pub fn new(config: &OfficeConfig) -> Result<Self, AuthError> {
        let creds = read_nc_desktop_credentials()?;

        let http = reqwest::blocking::Client::builder()
            .user_agent("hearth-office/0.1.0")
            .default_headers({
                let mut h = reqwest::header::HeaderMap::new();
                h.insert("OCS-APIREQUEST", "true".parse().unwrap());
                h
            })
            .build()
            .map_err(AuthError::HttpClientBuild)?;

        Ok(Self {
            http,
            base_url: creds.server_url,
            webdav_url: config.nextcloud.webdav_url.clone(),
            username: creds.username,
            password: creds.password,
        })
    }

    pub fn ocs_url(&self, path: &str) -> String {
        format!("{}/ocs/v2.php{}", self.base_url, path)
    }

    pub fn webdav_file_url(&self, nc_path: &str) -> String {
        format!("{}{}{}", self.webdav_url, self.username, nc_path)
    }

    pub fn authed_get(&self, url: &str) -> reqwest::blocking::RequestBuilder {
        self.http.get(url).basic_auth(&self.username, Some(&self.password))
    }

    pub fn authed_post(&self, url: &str) -> reqwest::blocking::RequestBuilder {
        self.http.post(url).basic_auth(&self.username, Some(&self.password))
    }

    pub fn authed_propfind(&self, url: &str) -> reqwest::blocking::RequestBuilder {
        self.http
            .request(reqwest::Method::from_bytes(b"PROPFIND").unwrap(), url)
            .basic_auth(&self.username, Some(&self.password))
            .header("Depth", "0")
    }
}

/// Lazily-initialized global client + config, rebuilt only when needed.
static OFFICE_CONTEXT: OnceLock<Result<OfficeContext, String>> = OnceLock::new();

struct OfficeContext {
    config: OfficeConfig,
}

/// Resolve a document URL to a Nextcloud client + path, or None if external.
///
/// Lazily loads config (once per process lifetime). Creates a fresh client
/// per call since credentials may rotate over long sessions.
pub fn resolve_nc_context(
    document_url: &str,
) -> Result<Option<(NextcloudClient, String)>, OfficeError> {
    let config = load_config()?;

    let sync_dir = crate::util::default_sync_dir();
    let location = crate::util::resolve_file_location(
        document_url,
        sync_dir.as_deref().unwrap_or(std::path::Path::new("/nonexistent")),
        &config.nextcloud.webdav_url,
    );

    let nc_path = match location {
        crate::util::FileLocation::Synced { nc_path } => nc_path,
        crate::util::FileLocation::WebDav { nc_path } => nc_path,
        crate::util::FileLocation::External => return Ok(None),
    };

    let client = NextcloudClient::new(config)
        .map_err(OfficeError::Auth)?;

    Ok(Some((client, nc_path)))
}

fn load_config() -> Result<&'static OfficeConfig, OfficeError> {
    let ctx = OFFICE_CONTEXT.get_or_init(|| {
        OfficeConfig::load()
            .map(|config| OfficeContext { config })
            .map_err(|e| e.to_string())
    });
    match ctx {
        Ok(ctx) => Ok(&ctx.config),
        Err(e) => Err(OfficeError::Config(e.clone())),
    }
}

/// Unified error type for the extension's top-level operations.
#[derive(Debug, thiserror::Error)]
pub enum OfficeError {
    #[error("config error: {0}")]
    Config(String),

    #[error("auth error: {0}")]
    Auth(AuthError),

    #[error("share error: {0}")]
    Share(share::ShareError),

    #[error("comments error: {0}")]
    Comments(comments::CommentsError),

    #[error("lock error: {0}")]
    Lock(lock::LockError),

    #[error("file is not on Nextcloud")]
    NotOnNextcloud,
}

struct NcCredentials {
    server_url: String,
    username: String,
    password: String,
}

fn read_nc_desktop_credentials() -> Result<NcCredentials, AuthError> {
    let config_path = nc_config_path()?;

    let mut ini = Ini::new();
    ini.load(config_path.to_str().unwrap_or(""))
        .map_err(|e| AuthError::ConfigParse(e.to_string()))?;

    let server_url = ini
        .get("Accounts", r"0\url")
        .ok_or_else(|| AuthError::ConfigParse("missing 0\\url in [Accounts]".into()))?;

    let username = ini
        .get("Accounts", r"0\user")
        .or_else(|| ini.get("Accounts", r"0\webflow\user"))
        .ok_or_else(|| AuthError::ConfigParse("missing 0\\user in [Accounts]".into()))?;

    let password = ini
        .get("Accounts", r"0\app_password")
        .or_else(|| read_password_from_keyring(&server_url, &username))
        .ok_or(AuthError::NoPassword)?;

    Ok(NcCredentials { server_url, username, password })
}

fn read_password_from_keyring(server_url: &str, username: &str) -> Option<String> {
    let output = std::process::Command::new("secret-tool")
        .args(["lookup", "user", username, "server", server_url, "type", "nextcloud"])
        .output()
        .ok()?;

    if output.status.success() {
        let password = String::from_utf8(output.stdout).ok()?;
        let trimmed = password.trim().to_string();
        if trimmed.is_empty() { None } else { Some(trimmed) }
    } else {
        None
    }
}

fn nc_config_path() -> Result<PathBuf, AuthError> {
    let config_dir = dirs::config_dir().ok_or(AuthError::NoConfigDir)?;
    let path = config_dir.join("Nextcloud").join("nextcloud.cfg");
    if !path.exists() {
        return Err(AuthError::NoNcConfig(path));
    }
    Ok(path)
}

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("could not determine XDG config directory")]
    NoConfigDir,

    #[error("Nextcloud Desktop config not found at {0}")]
    NoNcConfig(PathBuf),

    #[error("failed to parse Nextcloud config: {0}")]
    ConfigParse(String),

    #[error("no Nextcloud password found in config or keyring")]
    NoPassword,

    #[error("failed to build HTTP client: {0}")]
    HttpClientBuild(reqwest::Error),
}
