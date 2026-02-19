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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatResponse {
    pub target_closure: Option<String>,
    pub pending_installs: Vec<PendingSoftwareInstall>,
    #[serde(default)]
    pub active_deployment_id: Option<Uuid>,
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrollmentResponse {
    pub machine_id: Uuid,
    pub status: EnrollmentStatus,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApproveEnrollmentRequest {
    pub role: String,
    pub admin: String,
    #[serde(default)]
    pub target_closure: Option<String>,
    #[serde(default)]
    pub cache_url: Option<String>,
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
