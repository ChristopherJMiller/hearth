//! REST API request and response types shared between hearth-api and hearth-agent.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// --- Machine types ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Machine {
    pub id: Uuid,
    pub hostname: String,
    pub hardware_fingerprint: Option<String>,
    pub enrollment_status: EnrollmentStatus,
    pub current_closure: Option<String>,
    pub target_closure: Option<String>,
    pub rollback_closure: Option<String>,
    pub role: Option<String>,
    pub tags: Vec<String>,
    pub extra_config: Option<serde_json::Value>,
    pub last_heartbeat: Option<DateTime<Utc>>,
    #[serde(default)]
    pub enrolled_by: Option<String>,
    #[serde(default, skip_serializing)]
    pub machine_token_hash: Option<String>,
    /// Full hardware detection report (CPU, RAM, disk, NIC, etc.) stored as JSON.
    #[serde(default)]
    pub hardware_report: Option<serde_json::Value>,
    /// Device serial number for asset tracking.
    #[serde(default)]
    pub serial_number: Option<String>,
    /// Generated NixOS hardware-configuration.nix content from the device.
    /// Captured during enrollment via `nixos-generate-config --show-hardware-config`.
    #[serde(default)]
    pub hardware_config: Option<String>,
    /// Legacy hardware profile name (from migration 010). Retained for backwards
    /// compatibility but `hardware_config` is preferred for new enrollments.
    #[serde(default)]
    pub hardware_profile: Option<String>,
    /// Hash of the instance data JSON used for the current build.
    #[serde(default)]
    pub instance_data_hash: Option<String>,
    /// Git commit ref of the module library used for the current build.
    #[serde(default)]
    pub module_library_ref: Option<String>,
    /// Headscale mesh VPN IP address (100.x.y.z).
    #[serde(default)]
    pub headscale_ip: Option<String>,
    /// Headscale node identifier for API correlation.
    #[serde(default)]
    pub headscale_node_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnrollmentStatus {
    Pending,
    Approved,
    Enrolled,
    Provisioning,
    Active,
    Decommissioned,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateMachineRequest {
    pub hostname: String,
    pub hardware_fingerprint: Option<String>,
    pub role: Option<String>,
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateMachineRequest {
    pub hostname: Option<String>,
    pub role: Option<String>,
    pub tags: Option<Vec<String>>,
    pub target_closure: Option<String>,
    pub extra_config: Option<serde_json::Value>,
}

// --- Heartbeat types ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatRequest {
    pub machine_id: Uuid,
    pub current_closure: Option<String>,
    pub os_version: Option<String>,
    pub uptime_seconds: Option<u64>,
    #[serde(default)]
    pub update_in_progress: Option<bool>,
    #[serde(default)]
    pub update_error: Option<String>,
    /// Headscale mesh VPN IP address reported by the agent.
    #[serde(default)]
    pub headscale_ip: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatResponse {
    pub target_closure: Option<String>,
    pub pending_installs: Vec<PendingSoftwareInstall>,
    #[serde(default)]
    pub active_deployment_id: Option<Uuid>,
    #[serde(default)]
    pub cache_url: Option<String>,
    #[serde(default)]
    pub cache_token: Option<String>,
    /// Refreshed machine token — agent should persist this and use for future requests.
    #[serde(default)]
    pub machine_token: Option<String>,
    /// Remote actions pending execution on this machine.
    #[serde(default)]
    pub pending_actions: Vec<PendingAction>,
    /// Per-user environment closures ready for activation.
    #[serde(default)]
    pub pending_user_envs: Vec<PendingUserEnv>,
    /// Available platform services (populated from server config).
    #[serde(default)]
    pub services: Vec<ServiceInfo>,
}

// --- Target state ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetState {
    pub target_closure: Option<String>,
    pub module_library_ref: Option<String>,
}

// --- User environment types ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserEnvironment {
    pub id: Uuid,
    pub machine_id: Uuid,
    pub username: String,
    pub role: String,
    pub current_closure: Option<String>,
    pub target_closure: Option<String>,
    pub status: UserEnvStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UserEnvStatus {
    Pending,
    Building,
    Ready,
    Activating,
    Active,
    Failed,
}

// --- Deployment types ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Deployment {
    pub id: Uuid,
    pub closure: String,
    pub module_library_ref: String,
    pub instance_data_hash: String,
    pub status: DeploymentStatus,
    pub target_filter: serde_json::Value,
    pub total_machines: i32,
    pub succeeded: i32,
    pub failed: i32,
    pub canary_size: i32,
    pub batch_size: i32,
    pub failure_threshold: f64,
    pub rollback_reason: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeploymentStatus {
    Pending,
    Canary,
    Rolling,
    Completed,
    Failed,
    RolledBack,
}

// --- Deployment machine tracking ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MachineUpdateStatus {
    Pending,
    Downloading,
    Switching,
    Completed,
    Failed,
    RolledBack,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentMachineStatus {
    pub deployment_id: Uuid,
    pub machine_id: Uuid,
    pub status: MachineUpdateStatus,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateDeploymentRequest {
    pub closure: String,
    #[serde(default)]
    pub module_library_ref: Option<String>,
    #[serde(default)]
    pub instance_data_hash: Option<String>,
    #[serde(default)]
    pub target_filter: Option<serde_json::Value>,
    #[serde(default = "default_canary_size")]
    pub canary_size: i32,
    #[serde(default = "default_batch_size")]
    pub batch_size: i32,
    #[serde(default = "default_failure_threshold")]
    pub failure_threshold: f64,
}

fn default_canary_size() -> i32 {
    1
}
fn default_batch_size() -> i32 {
    5
}
fn default_failure_threshold() -> f64 {
    0.1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateDeploymentStatusRequest {
    pub status: DeploymentStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateMachineUpdateStatusRequest {
    pub status: MachineUpdateStatus,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerBuildRequest {
    pub flake_ref: String,
    #[serde(default)]
    pub target_filter: Option<serde_json::Value>,
    #[serde(default)]
    pub canary_size: Option<i32>,
    #[serde(default)]
    pub batch_size: Option<i32>,
    #[serde(default)]
    pub failure_threshold: Option<f64>,
}

// --- Fleet stats ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FleetStats {
    pub total_machines: i64,
    pub active_machines: i64,
    pub pending_enrollments: i64,
    pub active_deployments: i64,
    pub pending_requests: i64,
}

// --- Software catalog types ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogEntry {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub install_method: InstallMethod,
    pub flatpak_ref: Option<String>,
    pub nix_attr: Option<String>,
    pub icon_url: Option<String>,
    pub approval_required: bool,
    pub auto_approve_roles: Vec<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallMethod {
    NixSystem,
    NixUser,
    Flatpak,
    HomeManager,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoftwareRequest {
    pub id: Uuid,
    pub catalog_entry_id: Uuid,
    pub machine_id: Uuid,
    pub username: String,
    pub status: SoftwareRequestStatus,
    pub requested_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub resolved_by: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SoftwareRequestStatus {
    Pending,
    Approved,
    Denied,
    Installing,
    Installed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingSoftwareInstall {
    pub request_id: Uuid,
    pub username: String,
    pub catalog_entry: CatalogEntry,
}

// --- Software catalog request/response types ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCatalogEntryRequest {
    pub name: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub install_method: InstallMethod,
    pub flatpak_ref: Option<String>,
    pub nix_attr: Option<String>,
    pub icon_url: Option<String>,
    #[serde(default = "default_true")]
    pub approval_required: bool,
    #[serde(default)]
    pub auto_approve_roles: Vec<String>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateCatalogEntryRequest {
    pub name: Option<String>,
    pub description: Option<Option<String>>,
    pub category: Option<Option<String>>,
    pub install_method: Option<InstallMethod>,
    pub flatpak_ref: Option<Option<String>>,
    pub nix_attr: Option<Option<String>>,
    pub icon_url: Option<Option<String>>,
    pub approval_required: Option<bool>,
    pub auto_approve_roles: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoftwareRequestBody {
    pub machine_id: Uuid,
    pub username: String,
    #[serde(default)]
    pub user_role: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolveRequestBody {
    pub admin: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallResultReport {
    pub request_id: Uuid,
    pub success: bool,
    pub error_message: Option<String>,
}

// --- Enrollment request/response types ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrollmentRequest {
    pub hostname: String,
    pub hardware_fingerprint: Option<String>,
    pub os_version: Option<String>,
    pub role_hint: Option<String>,
    /// Full hardware detection report (CPU, RAM, disk, NIC, etc.).
    #[serde(default)]
    pub hardware_report: Option<serde_json::Value>,
    /// Device serial number for asset tracking.
    #[serde(default)]
    pub serial_number: Option<String>,
    /// Generated NixOS hardware-configuration.nix content from the device.
    /// Captured during enrollment via `nixos-generate-config --show-hardware-config`.
    #[serde(default)]
    pub hardware_config: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrollmentResponse {
    pub machine_id: Uuid,
    pub status: EnrollmentStatus,
    pub message: String,
    #[serde(default)]
    pub enrolled_by: Option<String>,
    /// Machine auth token, present only in the approval response after admin approves.
    #[serde(default)]
    pub machine_token: Option<String>,
    /// Target NixOS system closure to install during provisioning.
    #[serde(default)]
    pub target_closure: Option<String>,
    /// Attic cache URL for pulling the closure.
    #[serde(default)]
    pub cache_url: Option<String>,
    /// Attic cache pull token (short-lived JWT).
    #[serde(default)]
    pub cache_token: Option<String>,
    /// Disko config name for disk partitioning (e.g., "standard", "luks-lvm").
    /// The enrollment TUI resolves this to /etc/hearth/disko-configs/{name}.nix.
    #[serde(default)]
    pub disko_config: Option<String>,
    /// Headscale pre-auth key for mesh VPN join (single-use, short-lived).
    #[serde(default)]
    pub headscale_preauth_key: Option<String>,
    /// Headscale coordination server URL.
    #[serde(default)]
    pub headscale_url: Option<String>,
    /// Build job status for this machine (if a build has been queued).
    #[serde(default)]
    pub build_status: Option<String>,
    /// Build error message (set when the build job has failed).
    #[serde(default)]
    pub build_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApproveEnrollmentRequest {
    pub role: String,
    pub admin: String,
    #[serde(default)]
    pub target_closure: Option<String>,
    #[serde(default)]
    pub cache_url: Option<String>,
    /// Disko config name for disk partitioning during enrollment.
    /// Defaults to "standard" if not specified.
    #[serde(default)]
    pub disko_config: Option<String>,
}

// --- Cache token types ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheTokenResponse {
    pub cache_url: String,
    pub cache_token: String,
    /// Seconds until the token expires.
    pub expires_in: u64,
}

// --- User environment request types ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpsertUserEnvRequest {
    pub role: String,
    pub status: Option<UserEnvStatus>,
}

// --- Audit types ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    pub id: Uuid,
    pub event_type: String,
    pub actor: Option<String>,
    pub machine_id: Option<Uuid>,
    pub details: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

// --- User / identity types ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: Uuid,
    pub username: String,
    pub display_name: Option<String>,
    pub email: Option<String>,
    pub kanidm_uuid: Option<String>,
    pub groups: Vec<String>,
    pub last_seen: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A person in the company directory, enriched with derived contact info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectoryPerson {
    pub username: String,
    pub display_name: Option<String>,
    pub email: Option<String>,
    pub groups: Vec<String>,
    /// Derived Matrix ID, e.g. `@alice:hearth.local`.
    pub matrix_id: Option<String>,
    /// Derived Nextcloud profile URL.
    pub nextcloud_url: Option<String>,
    pub last_seen: Option<DateTime<Utc>>,
}

/// Claims extracted from a validated JWT (Kanidm OIDC or machine token).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthClaims {
    pub sub: String,
    #[serde(default)]
    pub preferred_username: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub groups: Vec<String>,
}

impl AuthClaims {
    /// The display username: `preferred_username` if available, otherwise `sub`.
    pub fn username(&self) -> &str {
        self.preferred_username.as_deref().unwrap_or(&self.sub)
    }
}

/// Authenticated identity attached to a request by the auth middleware.
#[derive(Debug, Clone)]
pub enum AuthIdentity {
    /// A human user authenticated via Kanidm OIDC.
    User(AuthClaims),
    /// A machine (agent) authenticated via a machine token.
    Machine { machine_id: Uuid },
}

// --- Remote action types ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionType {
    Lock,
    Restart,
    Rebuild,
    RunCommand,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionStatus {
    Pending,
    Delivered,
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingAction {
    pub id: Uuid,
    pub machine_id: Uuid,
    pub action_type: ActionType,
    pub payload: serde_json::Value,
    pub status: ActionStatus,
    pub created_by: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateActionRequest {
    pub action_type: ActionType,
    #[serde(default)]
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionResultReport {
    pub action_id: Uuid,
    pub success: bool,
    pub result: Option<serde_json::Value>,
}

// --- Per-user environment types ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingUserEnv {
    pub username: String,
    pub target_closure: String,
    pub cache_url: Option<String>,
}

// --- Per-user config types ---

/// Per-user environment configuration (machine-independent).
/// Role templates are initial seeds; user_configs is the source of truth.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserConfig {
    pub id: Uuid,
    pub username: String,
    pub base_role: String,
    pub overrides: serde_json::Value,
    pub config_hash: Option<String>,
    pub latest_closure: Option<String>,
    pub build_status: UserEnvBuildStatus,
    pub build_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum UserEnvBuildStatus {
    Pending,
    Building,
    Built,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpsertUserConfigRequest {
    pub base_role: Option<String>,
    pub overrides: Option<serde_json::Value>,
}

/// Self-service config update — restricted fields only.
///
/// Users can modify their own environment via `/api/v1/me/config`.
/// Admin-only fields (base_role, extra_packages) are excluded.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateMyConfigRequest {
    pub git_user_name: Option<String>,
    pub git_user_email: Option<String>,
    pub editor: Option<String>,
    pub shell_aliases: Option<std::collections::HashMap<String, String>>,
    pub session_variables: Option<std::collections::HashMap<String, String>>,
    pub desktop: Option<DesktopPreferences>,
}

/// User-customizable desktop preferences.
///
/// These are the curated set of GNOME dconf keys that users are allowed to
/// personalize. The agent reads these from dconf on the device and syncs
/// them back to the control plane; the build pipeline applies them as
/// overrides on top of the role defaults.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DesktopPreferences {
    /// Pinned apps on the dash/taskbar (e.g. `["firefox.desktop", "org.gnome.Nautilus.desktop"]`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub favorite_apps: Option<Vec<String>>,
    /// Wallpaper image URI (e.g. `file:///usr/share/backgrounds/gnome/blobs-l.svg`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wallpaper_uri: Option<String>,
    /// Solid background color when no wallpaper is set (e.g. `#1e1e2e`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wallpaper_color: Option<String>,
    /// Whether to use dark mode (`prefer-dark`) or light mode.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dark_mode: Option<bool>,
}

/// Request body for the machine-scoped desktop preferences sync endpoint.
///
/// Used by the agent to sync observed dconf values back on behalf of a user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncDesktopPrefsRequest {
    pub desktop: DesktopPreferences,
}

/// Response from the agent's env-closure lookup endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserEnvClosureResponse {
    /// Pre-built per-user closure, if available.
    pub closure: Option<String>,
    /// Binary cache URL to pull the closure from.
    pub cache_url: Option<String>,
    /// Role template to fall back to if no pre-built closure exists.
    pub fallback_role: String,
    /// Build pipeline status for this user's closure.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build_status: Option<UserEnvBuildStatus>,
}

/// Request body for reporting a broken closure from the agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportClosureFailureRequest {
    /// The closure store path that failed.
    pub closure: String,
    /// Error message describing the failure.
    pub error: String,
}

/// Response after reporting a closure failure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportClosureFailureResponse {
    /// Whether a rebuild was enqueued.
    pub rebuild_queued: bool,
}

/// A per-user environment build job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserEnvBuildJob {
    pub id: Uuid,
    pub username: String,
    pub config_hash: String,
    pub status: BuildJobStatus,
    pub worker_id: Option<String>,
    pub claimed_at: Option<DateTime<Utc>>,
    pub closure: Option<String>,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// --- Compliance report types ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceReport {
    pub total: i64,
    pub compliant: i64,
    pub drifted: i64,
    pub no_target: i64,
}

/// Per-machine drift status detail.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftedMachine {
    pub id: Uuid,
    pub hostname: String,
    pub current_closure: Option<String>,
    pub target_closure: Option<String>,
    pub last_heartbeat: Option<DateTime<Utc>>,
    pub role: Option<String>,
    pub tags: Vec<String>,
    pub drift_status: DriftStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DriftStatus {
    Compliant,
    Drifted,
    NoTarget,
}

// --- Compliance policy types ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompliancePolicy {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub nix_expression: String,
    pub severity: String,
    pub control_id: Option<String>,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCompliancePolicyRequest {
    pub name: String,
    pub description: Option<String>,
    pub nix_expression: String,
    #[serde(default = "default_severity")]
    pub severity: String,
    pub control_id: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_severity() -> String {
    "medium".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateCompliancePolicyRequest {
    pub name: Option<String>,
    pub description: Option<Option<String>>,
    pub nix_expression: Option<String>,
    pub severity: Option<String>,
    pub control_id: Option<Option<String>>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyResult {
    pub id: Uuid,
    pub deployment_id: Uuid,
    pub machine_id: Uuid,
    pub policy_id: Uuid,
    pub passed: bool,
    pub message: Option<String>,
    pub evaluated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentComplianceSummary {
    pub deployment_id: Uuid,
    pub total_checks: i64,
    pub passed: i64,
    pub failed: i64,
}

// --- SBOM types ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentSbom {
    pub id: Uuid,
    pub deployment_id: Uuid,
    pub machine_id: Uuid,
    pub closure: String,
    pub sbom_path: String,
    pub format: String,
    pub generated_at: DateTime<Utc>,
}

// --- Service discovery types ---

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceInfo {
    /// Machine-readable identifier (e.g., "chat", "cloud", "identity").
    pub id: String,
    /// Human-readable display name.
    pub name: String,
    /// Category for grouping in the UI.
    pub category: ServiceCategory,
    /// URL users should visit to access the service.
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Icon identifier (maps to a known icon set in the frontend).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceCategory {
    Communication,
    Storage,
    Identity,
    Infrastructure,
}

// --- Build job types ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuildJobStatus {
    Pending,
    Claimed,
    Evaluating,
    Building,
    Pushing,
    Deploying,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildJob {
    pub id: Uuid,
    pub status: BuildJobStatus,
    pub flake_ref: String,
    pub target_filter: Option<serde_json::Value>,
    pub canary_size: i32,
    pub batch_size: i32,
    pub failure_threshold: f64,
    pub worker_id: Option<String>,
    pub claimed_at: Option<DateTime<Utc>>,
    pub deployment_id: Option<Uuid>,
    pub closure: Option<String>,
    pub closures_built: Option<i32>,
    pub closures_pushed: Option<i32>,
    pub total_machines: Option<i32>,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateBuildJobRequest {
    pub flake_ref: String,
    #[serde(default)]
    pub target_filter: Option<serde_json::Value>,
    #[serde(default)]
    pub canary_size: Option<i32>,
    #[serde(default)]
    pub batch_size: Option<i32>,
    #[serde(default)]
    pub failure_threshold: Option<f64>,
}
