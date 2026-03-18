// Mirrors hearth-common api_types — keep in sync

export interface Machine {
  id: string;
  hostname: string;
  hardware_fingerprint: string | null;
  enrollment_status: EnrollmentStatus;
  current_closure: string | null;
  target_closure: string | null;
  rollback_closure: string | null;
  role: string | null;
  tags: string[];
  extra_config: Record<string, unknown> | null;
  last_heartbeat: string | null;
  created_at: string;
  updated_at: string;
}

export type EnrollmentStatus =
  | 'pending'
  | 'approved'
  | 'enrolled'
  | 'provisioning'
  | 'active'
  | 'decommissioned';

export interface Deployment {
  id: string;
  closure: string;
  module_library_ref: string;
  instance_data_hash: string;
  status: DeploymentStatus;
  target_filter: Record<string, unknown>;
  total_machines: number;
  succeeded: number;
  failed: number;
  canary_size: number;
  batch_size: number;
  failure_threshold: number;
  rollback_reason: string | null;
  created_at: string;
  updated_at: string;
}

export type DeploymentStatus =
  | 'pending'
  | 'canary'
  | 'rolling'
  | 'completed'
  | 'failed'
  | 'rolled_back';

export type MachineUpdateStatus =
  | 'pending'
  | 'downloading'
  | 'switching'
  | 'completed'
  | 'failed'
  | 'rolled_back';

export interface DeploymentMachineStatus {
  deployment_id: string;
  machine_id: string;
  status: MachineUpdateStatus;
  started_at: string | null;
  completed_at: string | null;
  error_message: string | null;
}

export type InstallMethod = 'nix_system' | 'nix_user' | 'flatpak' | 'home_manager';

export type SoftwareRequestStatus =
  | 'pending'
  | 'approved'
  | 'denied'
  | 'installing'
  | 'installed'
  | 'failed';

export interface CatalogEntry {
  id: string;
  name: string;
  description: string | null;
  category: string | null;
  install_method: InstallMethod;
  flatpak_ref: string | null;
  nix_attr: string | null;
  icon_url: string | null;
  approval_required: boolean;
  auto_approve_roles: string[];
  created_at: string;
}

export interface SoftwareRequest {
  id: string;
  catalog_entry_id: string;
  machine_id: string;
  username: string;
  status: SoftwareRequestStatus;
  requested_at: string;
  resolved_at: string | null;
  resolved_by: string | null;
}

export interface UserEnvironment {
  id: string;
  machine_id: string;
  username: string;
  role: string;
  current_closure: string | null;
  target_closure: string | null;
  status: 'pending' | 'building' | 'ready' | 'activating' | 'active' | 'failed';
  created_at: string;
  updated_at: string;
}

export interface AuditEvent {
  id: string;
  event_type: string;
  actor: string | null;
  machine_id: string | null;
  details: Record<string, unknown>;
  created_at: string;
}

export interface FleetStats {
  total_machines: number;
  active_machines: number;
  pending_enrollments: number;
  active_deployments: number;
  pending_requests: number;
}

// --- Phase 4: Remote Actions ---

export type ActionType = 'lock' | 'restart' | 'rebuild' | 'run_command';
export type ActionStatus = 'pending' | 'delivered' | 'running' | 'completed' | 'failed';

export interface PendingAction {
  id: string;
  machine_id: string;
  action_type: ActionType;
  payload: Record<string, unknown>;
  status: ActionStatus;
  created_by: string | null;
  created_at: string;
  delivered_at: string | null;
  completed_at: string | null;
  result: Record<string, unknown> | null;
}

export interface CreateActionRequest {
  action_type: ActionType;
  payload?: Record<string, unknown>;
}

// --- Phase 4: Reports ---

export interface ComplianceReport {
  total: number;
  compliant: number;
  drifted: number;
  no_target: number;
}

export interface DeploymentTimelineEntry {
  date: string;
  completed: number;
  failed: number;
  rolled_back: number;
}

export interface EnrollmentTimelineEntry {
  date: string;
  enrolled: number;
  pending: number;
}

// --- Phase 5B: Compliance Engine ---

export type DriftStatus = 'compliant' | 'drifted' | 'no_target';

export interface DriftedMachine {
  id: string;
  hostname: string;
  current_closure: string | null;
  target_closure: string | null;
  last_heartbeat: string | null;
  role: string | null;
  tags: string[];
  drift_status: DriftStatus;
}

export interface CompliancePolicy {
  id: string;
  name: string;
  description: string | null;
  nix_expression: string;
  severity: string;
  control_id: string | null;
  enabled: boolean;
  created_at: string;
  updated_at: string;
}

export interface CreateCompliancePolicyRequest {
  name: string;
  description?: string;
  nix_expression: string;
  severity?: string;
  control_id?: string;
  enabled?: boolean;
}

export interface PolicyResult {
  id: string;
  deployment_id: string;
  machine_id: string;
  policy_id: string;
  passed: boolean;
  message: string | null;
  evaluated_at: string;
}

export interface DeploymentComplianceSummary {
  deployment_id: string;
  total_checks: number;
  passed: number;
  failed: number;
}

export interface DeploymentSbom {
  id: string;
  deployment_id: string;
  machine_id: string;
  closure: string;
  sbom_path: string;
  format: string;
  generated_at: string;
}
