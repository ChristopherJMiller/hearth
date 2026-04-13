//! Configuration types for Hearth components, parsed from TOML files.

use serde::{Deserialize, Serialize};

/// Agent configuration, typically at /etc/hearth/agent.toml.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub server: ServerConnection,
    #[serde(default)]
    pub agent: AgentSettings,
    #[serde(default)]
    pub update: UpdateSettings,
    #[serde(default)]
    pub role_mapping: Option<RoleMapping>,
    pub home: Option<HomeConfig>,
    pub cache: Option<CacheConfig>,
    #[serde(default)]
    pub headscale: Option<HeadscaleAgentConfig>,
}

/// Headscale mesh VPN configuration for the agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeadscaleAgentConfig {
    /// When true, agent reports its Headscale IP in heartbeats.
    #[serde(default = "default_true")]
    pub report_ip: bool,
    /// Optional mesh URL for control plane communication over Headscale.
    pub mesh_server_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConnection {
    pub url: String,
    pub machine_id: Option<String>,
    /// Path to client certificate for mTLS (optional, used with Headscale).
    pub cert_path: Option<String>,
    pub key_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSettings {
    /// Poll interval in seconds.
    #[serde(default = "default_poll_interval")]
    pub poll_interval_secs: u64,
    /// Path to the Unix socket for IPC with the greeter.
    #[serde(default = "default_socket_path")]
    pub socket_path: String,
    /// Path to the offline event queue (SQLite).
    #[serde(default = "default_queue_path")]
    pub queue_path: String,
    /// Path to the machine auth token file.
    #[serde(default = "default_machine_token_path")]
    pub machine_token_path: String,
}

impl Default for AgentSettings {
    fn default() -> Self {
        Self {
            poll_interval_secs: default_poll_interval(),
            socket_path: default_socket_path(),
            queue_path: default_queue_path(),
            machine_token_path: default_machine_token_path(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateSettings {
    /// How to apply updates: "immediate", "maintenance_window", or "manual".
    #[serde(default = "default_apply_strategy")]
    pub apply_strategy: String,
    /// Reboot policy: "if_needed", "always", or "never".
    #[serde(default = "default_reboot_policy")]
    pub reboot_policy: String,
    /// Maintenance window start (HH:MM, 24h format).
    pub maintenance_window_start: Option<String>,
    /// Maintenance window end (HH:MM, 24h format).
    pub maintenance_window_end: Option<String>,
}

impl Default for UpdateSettings {
    fn default() -> Self {
        Self {
            apply_strategy: default_apply_strategy(),
            reboot_policy: default_reboot_policy(),
            maintenance_window_start: None,
            maintenance_window_end: None,
        }
    }
}

/// API server configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    pub listen: ListenConfig,
    pub database: DatabaseConfig,
    #[serde(default)]
    pub attic: Option<AtticConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListenConfig {
    #[serde(default = "default_listen_addr")]
    pub address: String,
    #[serde(default = "default_listen_port")]
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AtticConfig {
    pub server_url: String,
    pub token: Option<String>,
}

// --- Home-manager configuration ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HomeConfig {
    /// Flake reference for home-manager role profiles.
    pub flake_ref: String,
}

// --- Binary cache configuration ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    /// Attic binary cache URL for closure pulls.
    pub url: Option<String>,
}

// --- Role mapping ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleMapping {
    /// Priority-ordered list of group-to-role mappings. First match wins.
    pub mappings: Vec<RoleMappingEntry>,
    /// Default role if no mapping matches.
    #[serde(default = "default_role")]
    pub default_role: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleMappingEntry {
    pub group: String,
    pub role: String,
}

fn default_poll_interval() -> u64 {
    60
}
fn default_socket_path() -> String {
    "/run/hearth/agent.sock".to_string()
}
fn default_queue_path() -> String {
    "/var/lib/hearth/queue.db".to_string()
}
fn default_machine_token_path() -> String {
    "/var/lib/hearth/machine-token".to_string()
}
fn default_apply_strategy() -> String {
    "immediate".to_string()
}
fn default_reboot_policy() -> String {
    "if_needed".to_string()
}
fn default_listen_addr() -> String {
    "0.0.0.0".to_string()
}
fn default_listen_port() -> u16 {
    3000
}
fn default_max_connections() -> u32 {
    10
}
fn default_role() -> String {
    "default".to_string()
}

// --- Greeter configuration ---

/// Greeter configuration, typically at /etc/hearth/greeter.toml.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GreeterConfig {
    pub branding: BrandingConfig,
    pub agent: AgentConnectionConfig,
    pub session: SessionConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrandingConfig {
    #[serde(default = "default_org_name")]
    pub organization_name: String,
    pub logo_path: Option<String>,
    pub css_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConnectionConfig {
    #[serde(default = "default_socket_path")]
    pub socket_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    #[serde(default = "default_session_command")]
    pub command: String,
}

fn default_true() -> bool {
    true
}
fn default_org_name() -> String {
    "Your Organization".to_string()
}
fn default_session_command() -> String {
    "gnome-session".to_string()
}
