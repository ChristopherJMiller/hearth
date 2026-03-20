//! Database model types with `sqlx::FromRow` derives, and conversions to API types.

use chrono::{DateTime, Utc};
use hearth_common::api_types;
use uuid::Uuid;

// --- PostgreSQL enum mappings ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "enrollment_status", rename_all = "snake_case")]
pub enum EnrollmentStatusDb {
    Pending,
    Approved,
    Enrolled,
    Provisioning,
    Active,
    Decommissioned,
}

impl From<EnrollmentStatusDb> for api_types::EnrollmentStatus {
    fn from(s: EnrollmentStatusDb) -> Self {
        match s {
            EnrollmentStatusDb::Pending => api_types::EnrollmentStatus::Pending,
            EnrollmentStatusDb::Approved => api_types::EnrollmentStatus::Approved,
            EnrollmentStatusDb::Enrolled => api_types::EnrollmentStatus::Enrolled,
            EnrollmentStatusDb::Provisioning => api_types::EnrollmentStatus::Provisioning,
            EnrollmentStatusDb::Active => api_types::EnrollmentStatus::Active,
            EnrollmentStatusDb::Decommissioned => api_types::EnrollmentStatus::Decommissioned,
        }
    }
}

impl From<api_types::EnrollmentStatus> for EnrollmentStatusDb {
    fn from(s: api_types::EnrollmentStatus) -> Self {
        match s {
            api_types::EnrollmentStatus::Pending => EnrollmentStatusDb::Pending,
            api_types::EnrollmentStatus::Approved => EnrollmentStatusDb::Approved,
            api_types::EnrollmentStatus::Enrolled => EnrollmentStatusDb::Enrolled,
            api_types::EnrollmentStatus::Provisioning => EnrollmentStatusDb::Provisioning,
            api_types::EnrollmentStatus::Active => EnrollmentStatusDb::Active,
            api_types::EnrollmentStatus::Decommissioned => EnrollmentStatusDb::Decommissioned,
        }
    }
}

// --- Machine row ---

#[derive(Debug, sqlx::FromRow)]
pub struct MachineRow {
    pub id: Uuid,
    pub hostname: String,
    pub hardware_fingerprint: Option<String>,
    pub enrollment_status: EnrollmentStatusDb,
    pub current_closure: Option<String>,
    pub target_closure: Option<String>,
    pub rollback_closure: Option<String>,
    pub role: Option<String>,
    pub tags: Vec<String>,
    pub extra_config: Option<serde_json::Value>,
    pub last_heartbeat: Option<DateTime<Utc>>,
    pub enrolled_by: Option<String>,
    pub machine_token_hash: Option<String>,
    pub hardware_report: Option<serde_json::Value>,
    pub serial_number: Option<String>,
    pub hardware_config: Option<String>,
    pub hardware_profile: Option<String>,
    pub instance_data_hash: Option<String>,
    pub module_library_ref: Option<String>,
    pub headscale_ip: Option<String>,
    pub headscale_node_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<MachineRow> for api_types::Machine {
    fn from(row: MachineRow) -> Self {
        api_types::Machine {
            id: row.id,
            hostname: row.hostname,
            hardware_fingerprint: row.hardware_fingerprint,
            enrollment_status: row.enrollment_status.into(),
            current_closure: row.current_closure,
            target_closure: row.target_closure,
            rollback_closure: row.rollback_closure,
            role: row.role,
            tags: row.tags,
            extra_config: row.extra_config,
            last_heartbeat: row.last_heartbeat,
            enrolled_by: row.enrolled_by,
            machine_token_hash: row.machine_token_hash,
            hardware_report: row.hardware_report,
            serial_number: row.serial_number,
            hardware_config: row.hardware_config,
            hardware_profile: row.hardware_profile,
            instance_data_hash: row.instance_data_hash,
            module_library_ref: row.module_library_ref,
            headscale_ip: row.headscale_ip,
            headscale_node_id: row.headscale_node_id,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

// --- User row ---

#[derive(Debug, sqlx::FromRow)]
pub struct UserRow {
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

impl From<UserRow> for api_types::User {
    fn from(row: UserRow) -> Self {
        api_types::User {
            id: row.id,
            username: row.username,
            display_name: row.display_name,
            email: row.email,
            kanidm_uuid: row.kanidm_uuid,
            groups: row.groups,
            last_seen: row.last_seen,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

// --- Target state partial row ---

#[derive(Debug, sqlx::FromRow)]
pub struct TargetStateRow {
    pub target_closure: Option<String>,
}

impl From<TargetStateRow> for api_types::TargetState {
    fn from(row: TargetStateRow) -> Self {
        api_types::TargetState {
            target_closure: row.target_closure,
            module_library_ref: None, // Phase 1: not yet tracked
        }
    }
}

// --- Heartbeat result row ---

#[derive(Debug, sqlx::FromRow)]
pub struct HeartbeatResultRow {
    pub target_closure: Option<String>,
}

// --- User environment enums ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "user_env_status", rename_all = "snake_case")]
pub enum UserEnvStatusDb {
    Pending,
    Building,
    Ready,
    Activating,
    Active,
    Failed,
}

impl From<UserEnvStatusDb> for api_types::UserEnvStatus {
    fn from(s: UserEnvStatusDb) -> Self {
        match s {
            UserEnvStatusDb::Pending => api_types::UserEnvStatus::Pending,
            UserEnvStatusDb::Building => api_types::UserEnvStatus::Building,
            UserEnvStatusDb::Ready => api_types::UserEnvStatus::Ready,
            UserEnvStatusDb::Activating => api_types::UserEnvStatus::Activating,
            UserEnvStatusDb::Active => api_types::UserEnvStatus::Active,
            UserEnvStatusDb::Failed => api_types::UserEnvStatus::Failed,
        }
    }
}

impl From<api_types::UserEnvStatus> for UserEnvStatusDb {
    fn from(s: api_types::UserEnvStatus) -> Self {
        match s {
            api_types::UserEnvStatus::Pending => UserEnvStatusDb::Pending,
            api_types::UserEnvStatus::Building => UserEnvStatusDb::Building,
            api_types::UserEnvStatus::Ready => UserEnvStatusDb::Ready,
            api_types::UserEnvStatus::Activating => UserEnvStatusDb::Activating,
            api_types::UserEnvStatus::Active => UserEnvStatusDb::Active,
            api_types::UserEnvStatus::Failed => UserEnvStatusDb::Failed,
        }
    }
}

// --- User environment row ---

#[derive(Debug, sqlx::FromRow)]
pub struct UserEnvironmentRow {
    pub id: Uuid,
    pub machine_id: Uuid,
    pub username: String,
    pub role: String,
    pub current_closure: Option<String>,
    pub target_closure: Option<String>,
    pub status: UserEnvStatusDb,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<UserEnvironmentRow> for api_types::UserEnvironment {
    fn from(row: UserEnvironmentRow) -> Self {
        api_types::UserEnvironment {
            id: row.id,
            machine_id: row.machine_id,
            username: row.username,
            role: row.role,
            current_closure: row.current_closure,
            target_closure: row.target_closure,
            status: row.status.into(),
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

// --- Software catalog enums ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "install_method", rename_all = "snake_case")]
pub enum InstallMethodDb {
    NixSystem,
    NixUser,
    Flatpak,
    HomeManager,
}

impl From<InstallMethodDb> for api_types::InstallMethod {
    fn from(m: InstallMethodDb) -> Self {
        match m {
            InstallMethodDb::NixSystem => api_types::InstallMethod::NixSystem,
            InstallMethodDb::NixUser => api_types::InstallMethod::NixUser,
            InstallMethodDb::Flatpak => api_types::InstallMethod::Flatpak,
            InstallMethodDb::HomeManager => api_types::InstallMethod::HomeManager,
        }
    }
}

impl From<api_types::InstallMethod> for InstallMethodDb {
    fn from(m: api_types::InstallMethod) -> Self {
        match m {
            api_types::InstallMethod::NixSystem => InstallMethodDb::NixSystem,
            api_types::InstallMethod::NixUser => InstallMethodDb::NixUser,
            api_types::InstallMethod::Flatpak => InstallMethodDb::Flatpak,
            api_types::InstallMethod::HomeManager => InstallMethodDb::HomeManager,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "software_request_status", rename_all = "snake_case")]
pub enum SoftwareRequestStatusDb {
    Pending,
    Approved,
    Denied,
    Installing,
    Installed,
    Failed,
}

impl From<SoftwareRequestStatusDb> for api_types::SoftwareRequestStatus {
    fn from(s: SoftwareRequestStatusDb) -> Self {
        match s {
            SoftwareRequestStatusDb::Pending => api_types::SoftwareRequestStatus::Pending,
            SoftwareRequestStatusDb::Approved => api_types::SoftwareRequestStatus::Approved,
            SoftwareRequestStatusDb::Denied => api_types::SoftwareRequestStatus::Denied,
            SoftwareRequestStatusDb::Installing => api_types::SoftwareRequestStatus::Installing,
            SoftwareRequestStatusDb::Installed => api_types::SoftwareRequestStatus::Installed,
            SoftwareRequestStatusDb::Failed => api_types::SoftwareRequestStatus::Failed,
        }
    }
}

impl From<api_types::SoftwareRequestStatus> for SoftwareRequestStatusDb {
    fn from(s: api_types::SoftwareRequestStatus) -> Self {
        match s {
            api_types::SoftwareRequestStatus::Pending => SoftwareRequestStatusDb::Pending,
            api_types::SoftwareRequestStatus::Approved => SoftwareRequestStatusDb::Approved,
            api_types::SoftwareRequestStatus::Denied => SoftwareRequestStatusDb::Denied,
            api_types::SoftwareRequestStatus::Installing => SoftwareRequestStatusDb::Installing,
            api_types::SoftwareRequestStatus::Installed => SoftwareRequestStatusDb::Installed,
            api_types::SoftwareRequestStatus::Failed => SoftwareRequestStatusDb::Failed,
        }
    }
}

// --- Catalog entry row ---

#[derive(Debug, sqlx::FromRow)]
pub struct CatalogEntryRow {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub install_method: InstallMethodDb,
    pub flatpak_ref: Option<String>,
    pub nix_attr: Option<String>,
    pub icon_url: Option<String>,
    pub approval_required: bool,
    pub auto_approve_roles: Vec<String>,
    pub created_at: DateTime<Utc>,
}

impl From<CatalogEntryRow> for api_types::CatalogEntry {
    fn from(row: CatalogEntryRow) -> Self {
        api_types::CatalogEntry {
            id: row.id,
            name: row.name,
            description: row.description,
            category: row.category,
            install_method: row.install_method.into(),
            flatpak_ref: row.flatpak_ref,
            nix_attr: row.nix_attr,
            icon_url: row.icon_url,
            approval_required: row.approval_required,
            auto_approve_roles: row.auto_approve_roles,
            created_at: row.created_at,
        }
    }
}

// --- Software request row ---

#[derive(Debug, sqlx::FromRow)]
pub struct SoftwareRequestRow {
    pub id: Uuid,
    pub catalog_entry_id: Uuid,
    pub machine_id: Uuid,
    pub username: String,
    pub status: SoftwareRequestStatusDb,
    pub requested_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub resolved_by: Option<String>,
}

impl From<SoftwareRequestRow> for api_types::SoftwareRequest {
    fn from(row: SoftwareRequestRow) -> Self {
        api_types::SoftwareRequest {
            id: row.id,
            catalog_entry_id: row.catalog_entry_id,
            machine_id: row.machine_id,
            username: row.username,
            status: row.status.into(),
            requested_at: row.requested_at,
            resolved_at: row.resolved_at,
            resolved_by: row.resolved_by,
        }
    }
}

// --- Deployment status enum ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "deployment_status", rename_all = "snake_case")]
pub enum DeploymentStatusDb {
    Pending,
    Canary,
    Rolling,
    Completed,
    Failed,
    RolledBack,
}

impl From<DeploymentStatusDb> for api_types::DeploymentStatus {
    fn from(s: DeploymentStatusDb) -> Self {
        match s {
            DeploymentStatusDb::Pending => api_types::DeploymentStatus::Pending,
            DeploymentStatusDb::Canary => api_types::DeploymentStatus::Canary,
            DeploymentStatusDb::Rolling => api_types::DeploymentStatus::Rolling,
            DeploymentStatusDb::Completed => api_types::DeploymentStatus::Completed,
            DeploymentStatusDb::Failed => api_types::DeploymentStatus::Failed,
            DeploymentStatusDb::RolledBack => api_types::DeploymentStatus::RolledBack,
        }
    }
}

impl From<api_types::DeploymentStatus> for DeploymentStatusDb {
    fn from(s: api_types::DeploymentStatus) -> Self {
        match s {
            api_types::DeploymentStatus::Pending => DeploymentStatusDb::Pending,
            api_types::DeploymentStatus::Canary => DeploymentStatusDb::Canary,
            api_types::DeploymentStatus::Rolling => DeploymentStatusDb::Rolling,
            api_types::DeploymentStatus::Completed => DeploymentStatusDb::Completed,
            api_types::DeploymentStatus::Failed => DeploymentStatusDb::Failed,
            api_types::DeploymentStatus::RolledBack => DeploymentStatusDb::RolledBack,
        }
    }
}

// --- Machine update status enum ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "machine_update_status", rename_all = "snake_case")]
pub enum MachineUpdateStatusDb {
    Pending,
    Downloading,
    Switching,
    Completed,
    Failed,
    RolledBack,
}

impl From<MachineUpdateStatusDb> for api_types::MachineUpdateStatus {
    fn from(s: MachineUpdateStatusDb) -> Self {
        match s {
            MachineUpdateStatusDb::Pending => api_types::MachineUpdateStatus::Pending,
            MachineUpdateStatusDb::Downloading => api_types::MachineUpdateStatus::Downloading,
            MachineUpdateStatusDb::Switching => api_types::MachineUpdateStatus::Switching,
            MachineUpdateStatusDb::Completed => api_types::MachineUpdateStatus::Completed,
            MachineUpdateStatusDb::Failed => api_types::MachineUpdateStatus::Failed,
            MachineUpdateStatusDb::RolledBack => api_types::MachineUpdateStatus::RolledBack,
        }
    }
}

impl From<api_types::MachineUpdateStatus> for MachineUpdateStatusDb {
    fn from(s: api_types::MachineUpdateStatus) -> Self {
        match s {
            api_types::MachineUpdateStatus::Pending => MachineUpdateStatusDb::Pending,
            api_types::MachineUpdateStatus::Downloading => MachineUpdateStatusDb::Downloading,
            api_types::MachineUpdateStatus::Switching => MachineUpdateStatusDb::Switching,
            api_types::MachineUpdateStatus::Completed => MachineUpdateStatusDb::Completed,
            api_types::MachineUpdateStatus::Failed => MachineUpdateStatusDb::Failed,
            api_types::MachineUpdateStatus::RolledBack => MachineUpdateStatusDb::RolledBack,
        }
    }
}

// --- Deployment row ---

#[derive(Debug, sqlx::FromRow)]
pub struct DeploymentRow {
    pub id: Uuid,
    pub closure: String,
    pub module_library_ref: String,
    pub instance_data_hash: String,
    pub status: DeploymentStatusDb,
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

impl From<DeploymentRow> for api_types::Deployment {
    fn from(row: DeploymentRow) -> Self {
        api_types::Deployment {
            id: row.id,
            closure: row.closure,
            module_library_ref: row.module_library_ref,
            instance_data_hash: row.instance_data_hash,
            status: row.status.into(),
            target_filter: row.target_filter,
            total_machines: row.total_machines,
            succeeded: row.succeeded,
            failed: row.failed,
            canary_size: row.canary_size,
            batch_size: row.batch_size,
            failure_threshold: row.failure_threshold,
            rollback_reason: row.rollback_reason,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

// --- Deployment machine row ---

#[derive(Debug, sqlx::FromRow)]
pub struct DeploymentMachineRow {
    pub deployment_id: Uuid,
    pub machine_id: Uuid,
    pub status: MachineUpdateStatusDb,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub error_message: Option<String>,
}

impl From<DeploymentMachineRow> for api_types::DeploymentMachineStatus {
    fn from(row: DeploymentMachineRow) -> Self {
        api_types::DeploymentMachineStatus {
            deployment_id: row.deployment_id,
            machine_id: row.machine_id,
            status: row.status.into(),
            started_at: row.started_at,
            completed_at: row.completed_at,
            error_message: row.error_message,
        }
    }
}

// --- Audit event row ---

#[derive(Debug, sqlx::FromRow)]
pub struct AuditEventRow {
    pub id: Uuid,
    pub event_type: String,
    pub actor: Option<String>,
    pub machine_id: Option<Uuid>,
    pub details: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

impl From<AuditEventRow> for api_types::AuditEvent {
    fn from(row: AuditEventRow) -> Self {
        api_types::AuditEvent {
            id: row.id,
            event_type: row.event_type,
            actor: row.actor,
            machine_id: row.machine_id,
            details: row.details,
            created_at: row.created_at,
        }
    }
}

// --- Active deployment ID row ---

#[derive(Debug, sqlx::FromRow)]
pub struct ActiveDeploymentRow {
    pub deployment_id: Uuid,
}

// --- Deployment closure row ---

#[derive(Debug, sqlx::FromRow)]
pub struct DeploymentClosureRow {
    pub closure: String,
}

// --- Remote action enums ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "action_type", rename_all = "snake_case")]
pub enum ActionTypeDb {
    Lock,
    Restart,
    Rebuild,
    RunCommand,
}

impl From<ActionTypeDb> for api_types::ActionType {
    fn from(a: ActionTypeDb) -> Self {
        match a {
            ActionTypeDb::Lock => api_types::ActionType::Lock,
            ActionTypeDb::Restart => api_types::ActionType::Restart,
            ActionTypeDb::Rebuild => api_types::ActionType::Rebuild,
            ActionTypeDb::RunCommand => api_types::ActionType::RunCommand,
        }
    }
}

impl From<api_types::ActionType> for ActionTypeDb {
    fn from(a: api_types::ActionType) -> Self {
        match a {
            api_types::ActionType::Lock => ActionTypeDb::Lock,
            api_types::ActionType::Restart => ActionTypeDb::Restart,
            api_types::ActionType::Rebuild => ActionTypeDb::Rebuild,
            api_types::ActionType::RunCommand => ActionTypeDb::RunCommand,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "action_status", rename_all = "snake_case")]
pub enum ActionStatusDb {
    Pending,
    Delivered,
    Running,
    Completed,
    Failed,
}

impl From<ActionStatusDb> for api_types::ActionStatus {
    fn from(s: ActionStatusDb) -> Self {
        match s {
            ActionStatusDb::Pending => api_types::ActionStatus::Pending,
            ActionStatusDb::Delivered => api_types::ActionStatus::Delivered,
            ActionStatusDb::Running => api_types::ActionStatus::Running,
            ActionStatusDb::Completed => api_types::ActionStatus::Completed,
            ActionStatusDb::Failed => api_types::ActionStatus::Failed,
        }
    }
}

// --- Pending action row ---

#[derive(Debug, sqlx::FromRow)]
pub struct PendingActionRow {
    pub id: Uuid,
    pub machine_id: Uuid,
    pub action_type: ActionTypeDb,
    pub payload: serde_json::Value,
    pub status: ActionStatusDb,
    pub created_by: Option<String>,
    pub created_at: DateTime<Utc>,
    pub delivered_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub result: Option<serde_json::Value>,
}

impl From<PendingActionRow> for api_types::PendingAction {
    fn from(row: PendingActionRow) -> Self {
        api_types::PendingAction {
            id: row.id,
            machine_id: row.machine_id,
            action_type: row.action_type.into(),
            payload: row.payload,
            status: row.status.into(),
            created_by: row.created_by,
            created_at: row.created_at,
        }
    }
}

// --- Pending user env row (for heartbeat response) ---

#[derive(Debug, sqlx::FromRow)]
pub struct PendingUserEnvRow {
    pub username: String,
    pub target_closure: String,
}

// --- Build job status enum ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "build_job_status", rename_all = "snake_case")]
pub enum BuildJobStatusDb {
    Pending,
    Claimed,
    Evaluating,
    Building,
    Pushing,
    Deploying,
    Completed,
    Failed,
}

impl From<BuildJobStatusDb> for api_types::BuildJobStatus {
    fn from(s: BuildJobStatusDb) -> Self {
        match s {
            BuildJobStatusDb::Pending => api_types::BuildJobStatus::Pending,
            BuildJobStatusDb::Claimed => api_types::BuildJobStatus::Claimed,
            BuildJobStatusDb::Evaluating => api_types::BuildJobStatus::Evaluating,
            BuildJobStatusDb::Building => api_types::BuildJobStatus::Building,
            BuildJobStatusDb::Pushing => api_types::BuildJobStatus::Pushing,
            BuildJobStatusDb::Deploying => api_types::BuildJobStatus::Deploying,
            BuildJobStatusDb::Completed => api_types::BuildJobStatus::Completed,
            BuildJobStatusDb::Failed => api_types::BuildJobStatus::Failed,
        }
    }
}

impl From<api_types::BuildJobStatus> for BuildJobStatusDb {
    fn from(s: api_types::BuildJobStatus) -> Self {
        match s {
            api_types::BuildJobStatus::Pending => BuildJobStatusDb::Pending,
            api_types::BuildJobStatus::Claimed => BuildJobStatusDb::Claimed,
            api_types::BuildJobStatus::Evaluating => BuildJobStatusDb::Evaluating,
            api_types::BuildJobStatus::Building => BuildJobStatusDb::Building,
            api_types::BuildJobStatus::Pushing => BuildJobStatusDb::Pushing,
            api_types::BuildJobStatus::Deploying => BuildJobStatusDb::Deploying,
            api_types::BuildJobStatus::Completed => BuildJobStatusDb::Completed,
            api_types::BuildJobStatus::Failed => BuildJobStatusDb::Failed,
        }
    }
}

// --- Build job row ---

#[derive(Debug, sqlx::FromRow)]
pub struct BuildJobRow {
    pub id: Uuid,
    pub status: BuildJobStatusDb,
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

impl From<BuildJobRow> for api_types::BuildJob {
    fn from(row: BuildJobRow) -> Self {
        api_types::BuildJob {
            id: row.id,
            status: row.status.into(),
            flake_ref: row.flake_ref,
            target_filter: row.target_filter,
            canary_size: row.canary_size,
            batch_size: row.batch_size,
            failure_threshold: row.failure_threshold,
            worker_id: row.worker_id,
            claimed_at: row.claimed_at,
            deployment_id: row.deployment_id,
            closure: row.closure,
            closures_built: row.closures_built,
            closures_pushed: row.closures_pushed,
            total_machines: row.total_machines,
            error_message: row.error_message,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

// --- Compliance policy row ---

#[derive(Debug, sqlx::FromRow)]
pub struct CompliancePolicyRow {
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

impl From<CompliancePolicyRow> for api_types::CompliancePolicy {
    fn from(row: CompliancePolicyRow) -> Self {
        api_types::CompliancePolicy {
            id: row.id,
            name: row.name,
            description: row.description,
            nix_expression: row.nix_expression,
            severity: row.severity,
            control_id: row.control_id,
            enabled: row.enabled,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

// --- Policy result row ---

#[derive(Debug, sqlx::FromRow)]
pub struct PolicyResultRow {
    pub id: Uuid,
    pub deployment_id: Uuid,
    pub machine_id: Uuid,
    pub policy_id: Uuid,
    pub passed: bool,
    pub message: Option<String>,
    pub evaluated_at: DateTime<Utc>,
}

impl From<PolicyResultRow> for api_types::PolicyResult {
    fn from(row: PolicyResultRow) -> Self {
        api_types::PolicyResult {
            id: row.id,
            deployment_id: row.deployment_id,
            machine_id: row.machine_id,
            policy_id: row.policy_id,
            passed: row.passed,
            message: row.message,
            evaluated_at: row.evaluated_at,
        }
    }
}

// --- Deployment SBOM row ---

#[derive(Debug, sqlx::FromRow)]
pub struct DeploymentSbomRow {
    pub id: Uuid,
    pub deployment_id: Uuid,
    pub machine_id: Uuid,
    pub closure: String,
    pub sbom_path: String,
    pub format: String,
    pub generated_at: DateTime<Utc>,
}

impl From<DeploymentSbomRow> for api_types::DeploymentSbom {
    fn from(row: DeploymentSbomRow) -> Self {
        api_types::DeploymentSbom {
            id: row.id,
            deployment_id: row.deployment_id,
            machine_id: row.machine_id,
            closure: row.closure,
            sbom_path: row.sbom_path,
            format: row.format,
            generated_at: row.generated_at,
        }
    }
}

// --- Drifted machine row ---

#[derive(Debug, sqlx::FromRow)]
pub struct DriftedMachineRow {
    pub id: Uuid,
    pub hostname: String,
    pub current_closure: Option<String>,
    pub target_closure: Option<String>,
    pub last_heartbeat: Option<DateTime<Utc>>,
    pub role: Option<String>,
    pub tags: Vec<String>,
}

// --- Pending install row (joined request + catalog) ---

#[derive(Debug, sqlx::FromRow)]
pub struct PendingInstallRow {
    // From software_requests
    pub request_id: Uuid,
    pub username: String,
    // From software_catalog (prefixed in query)
    pub catalog_id: Uuid,
    pub catalog_name: String,
    pub catalog_description: Option<String>,
    pub catalog_category: Option<String>,
    pub catalog_install_method: InstallMethodDb,
    pub catalog_flatpak_ref: Option<String>,
    pub catalog_nix_attr: Option<String>,
    pub catalog_icon_url: Option<String>,
    pub catalog_approval_required: bool,
    pub catalog_auto_approve_roles: Vec<String>,
    pub catalog_created_at: DateTime<Utc>,
}

impl From<PendingInstallRow> for api_types::PendingSoftwareInstall {
    fn from(row: PendingInstallRow) -> Self {
        api_types::PendingSoftwareInstall {
            request_id: row.request_id,
            username: row.username,
            catalog_entry: api_types::CatalogEntry {
                id: row.catalog_id,
                name: row.catalog_name,
                description: row.catalog_description,
                category: row.catalog_category,
                install_method: row.catalog_install_method.into(),
                flatpak_ref: row.catalog_flatpak_ref,
                nix_attr: row.catalog_nix_attr,
                icon_url: row.catalog_icon_url,
                approval_required: row.catalog_approval_required,
                auto_approve_roles: row.catalog_auto_approve_roles,
                created_at: row.catalog_created_at,
            },
        }
    }
}
