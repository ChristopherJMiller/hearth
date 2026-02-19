//! Repository layer: database queries for machines, heartbeats, etc.

use chrono::{DateTime, Utc};
use hearth_common::api_types::{
    CreateCatalogEntryRequest, CreateDeploymentRequest, CreateMachineRequest, FleetStats,
    HeartbeatRequest, HeartbeatResponse, TargetState, UpdateCatalogEntryRequest,
    UpdateMachineRequest,
};
use sqlx::PgPool;
use uuid::Uuid;

use crate::db::{
    ActiveDeploymentRow, AuditEventRow, CatalogEntryRow, DeploymentClosureRow,
    DeploymentMachineRow, DeploymentRow, DeploymentStatusDb, HeartbeatResultRow, InstallMethodDb,
    MachineRow, MachineUpdateStatusDb, PendingInstallRow, RoleClosureRow, SoftwareRequestRow,
    SoftwareRequestStatusDb, TargetStateRow, UserEnvStatusDb, UserEnvironmentRow,
};

pub async fn list_machines(pool: &PgPool) -> Result<Vec<MachineRow>, sqlx::Error> {
    sqlx::query_as::<_, MachineRow>(
        "SELECT id, hostname, hardware_fingerprint, enrollment_status,
                current_closure, target_closure, rollback_closure,
                role, tags, extra_config, last_heartbeat,
                created_at, updated_at
         FROM machines
         ORDER BY created_at DESC",
    )
    .fetch_all(pool)
    .await
}

pub async fn get_machine(pool: &PgPool, id: Uuid) -> Result<Option<MachineRow>, sqlx::Error> {
    sqlx::query_as::<_, MachineRow>(
        "SELECT id, hostname, hardware_fingerprint, enrollment_status,
                current_closure, target_closure, rollback_closure,
                role, tags, extra_config, last_heartbeat,
                created_at, updated_at
         FROM machines
         WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn create_machine(
    pool: &PgPool,
    req: &CreateMachineRequest,
) -> Result<MachineRow, sqlx::Error> {
    let tags = req.tags.clone().unwrap_or_default();

    sqlx::query_as::<_, MachineRow>(
        "INSERT INTO machines (hostname, hardware_fingerprint, role, tags)
         VALUES ($1, $2, $3, $4)
         RETURNING id, hostname, hardware_fingerprint, enrollment_status,
                   current_closure, target_closure, rollback_closure,
                   role, tags, extra_config, last_heartbeat,
                   created_at, updated_at",
    )
    .bind(&req.hostname)
    .bind(&req.hardware_fingerprint)
    .bind(&req.role)
    .bind(&tags)
    .fetch_one(pool)
    .await
}

pub async fn update_machine(
    pool: &PgPool,
    id: Uuid,
    req: &UpdateMachineRequest,
) -> Result<Option<MachineRow>, sqlx::Error> {
    // Use COALESCE-style conditional updates: only update fields that are Some.
    sqlx::query_as::<_, MachineRow>(
        "UPDATE machines SET
            hostname = COALESCE($2, hostname),
            role = COALESCE($3, role),
            tags = COALESCE($4, tags),
            target_closure = COALESCE($5, target_closure),
            extra_config = COALESCE($6, extra_config),
            updated_at = now()
         WHERE id = $1
         RETURNING id, hostname, hardware_fingerprint, enrollment_status,
                   current_closure, target_closure, rollback_closure,
                   role, tags, extra_config, last_heartbeat,
                   created_at, updated_at",
    )
    .bind(id)
    .bind(&req.hostname)
    .bind(&req.role)
    .bind(&req.tags)
    .bind(&req.target_closure)
    .bind(&req.extra_config)
    .fetch_optional(pool)
    .await
}

pub async fn delete_machine(pool: &PgPool, id: Uuid) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("DELETE FROM machines WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;

    Ok(result.rows_affected() > 0)
}

pub async fn get_target_state(pool: &PgPool, id: Uuid) -> Result<Option<TargetState>, sqlx::Error> {
    let row =
        sqlx::query_as::<_, TargetStateRow>("SELECT target_closure FROM machines WHERE id = $1")
            .bind(id)
            .fetch_optional(pool)
            .await?;

    Ok(row.map(|r| r.into()))
}

pub async fn record_heartbeat(
    pool: &PgPool,
    req: &HeartbeatRequest,
) -> Result<Option<HeartbeatResponse>, sqlx::Error> {
    let row = sqlx::query_as::<_, HeartbeatResultRow>(
        "UPDATE machines SET
            last_heartbeat = now(),
            current_closure = COALESCE($2, current_closure),
            updated_at = now()
         WHERE id = $1
         RETURNING target_closure",
    )
    .bind(req.machine_id)
    .bind(&req.current_closure)
    .fetch_optional(pool)
    .await?;

    match row {
        Some(r) => {
            let installs = get_pending_installs(pool, req.machine_id).await?;

            // Check for active deployment assignment
            let active_deployment = get_active_deployment_for_machine(pool, req.machine_id).await?;
            let active_deployment_id = active_deployment.map(|d| d.deployment_id);

            // If this machine has an active deployment and its current closure matches
            // the deployment closure, mark it as completed
            if let (Some(dep_id), Some(current_closure)) =
                (active_deployment_id, &req.current_closure)
            {
                let dep_closure = get_deployment_closure(pool, dep_id).await?;
                if let Some(dc) = dep_closure
                    && dc.closure == *current_closure
                {
                    // Machine has arrived at the deployment closure -- mark completed
                    let _ = upsert_deployment_machine(
                        pool,
                        dep_id,
                        req.machine_id,
                        MachineUpdateStatusDb::Completed,
                        None,
                    )
                    .await?;
                    increment_deployment_counter(pool, dep_id, true).await?;
                }
            }

            Ok(Some(HeartbeatResponse {
                target_closure: r.target_closure,
                pending_installs: installs.into_iter().map(Into::into).collect(),
                active_deployment_id,
            }))
        }
        None => Ok(None),
    }
}

// --- Software catalog queries ---

pub async fn list_catalog(pool: &PgPool) -> Result<Vec<CatalogEntryRow>, sqlx::Error> {
    sqlx::query_as::<_, CatalogEntryRow>(
        "SELECT id, name, description, category, install_method,
                flatpak_ref, nix_attr, icon_url, approval_required,
                auto_approve_roles, created_at
         FROM software_catalog
         ORDER BY name",
    )
    .fetch_all(pool)
    .await
}

pub async fn get_catalog_entry(
    pool: &PgPool,
    id: Uuid,
) -> Result<Option<CatalogEntryRow>, sqlx::Error> {
    sqlx::query_as::<_, CatalogEntryRow>(
        "SELECT id, name, description, category, install_method,
                flatpak_ref, nix_attr, icon_url, approval_required,
                auto_approve_roles, created_at
         FROM software_catalog
         WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn create_catalog_entry(
    pool: &PgPool,
    req: &CreateCatalogEntryRequest,
) -> Result<CatalogEntryRow, sqlx::Error> {
    sqlx::query_as::<_, CatalogEntryRow>(
        "INSERT INTO software_catalog
            (name, description, category, install_method, flatpak_ref,
             nix_attr, icon_url, approval_required, auto_approve_roles)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
         RETURNING id, name, description, category, install_method,
                   flatpak_ref, nix_attr, icon_url, approval_required,
                   auto_approve_roles, created_at",
    )
    .bind(&req.name)
    .bind(&req.description)
    .bind(&req.category)
    .bind(InstallMethodDb::from(req.install_method))
    .bind(&req.flatpak_ref)
    .bind(&req.nix_attr)
    .bind(&req.icon_url)
    .bind(req.approval_required)
    .bind(&req.auto_approve_roles)
    .fetch_one(pool)
    .await
}

pub async fn update_catalog_entry(
    pool: &PgPool,
    id: Uuid,
    req: &UpdateCatalogEntryRequest,
) -> Result<Option<CatalogEntryRow>, sqlx::Error> {
    let install_method = req.install_method.map(InstallMethodDb::from);

    sqlx::query_as::<_, CatalogEntryRow>(
        "UPDATE software_catalog SET
            name = COALESCE($2, name),
            description = COALESCE($3, description),
            category = COALESCE($4, category),
            install_method = COALESCE($5, install_method),
            flatpak_ref = COALESCE($6, flatpak_ref),
            nix_attr = COALESCE($7, nix_attr),
            icon_url = COALESCE($8, icon_url),
            approval_required = COALESCE($9, approval_required),
            auto_approve_roles = COALESCE($10, auto_approve_roles)
         WHERE id = $1
         RETURNING id, name, description, category, install_method,
                   flatpak_ref, nix_attr, icon_url, approval_required,
                   auto_approve_roles, created_at",
    )
    .bind(id)
    .bind(&req.name)
    .bind(&req.description)
    .bind(&req.category)
    .bind(install_method)
    .bind(&req.flatpak_ref)
    .bind(&req.nix_attr)
    .bind(&req.icon_url)
    .bind(req.approval_required)
    .bind(&req.auto_approve_roles)
    .fetch_optional(pool)
    .await
}

pub async fn delete_catalog_entry(pool: &PgPool, id: Uuid) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("DELETE FROM software_catalog WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;

    Ok(result.rows_affected() > 0)
}

// --- Software request workflow ---

pub async fn create_software_request(
    pool: &PgPool,
    catalog_entry_id: Uuid,
    machine_id: Uuid,
    username: &str,
    user_role: Option<&str>,
) -> Result<SoftwareRequestRow, sqlx::Error> {
    // Fetch the catalog entry to check approval rules
    let entry = get_catalog_entry(pool, catalog_entry_id).await?;
    let entry = entry.ok_or_else(|| sqlx::Error::RowNotFound)?;

    // Determine initial status
    let status = if !entry.approval_required {
        SoftwareRequestStatusDb::Approved
    } else if let Some(role) = user_role {
        if entry.auto_approve_roles.contains(&role.to_string()) {
            SoftwareRequestStatusDb::Approved
        } else {
            SoftwareRequestStatusDb::Pending
        }
    } else {
        SoftwareRequestStatusDb::Pending
    };

    sqlx::query_as::<_, SoftwareRequestRow>(
        "INSERT INTO software_requests
            (catalog_entry_id, machine_id, username, status)
         VALUES ($1, $2, $3, $4)
         RETURNING id, catalog_entry_id, machine_id, username,
                   status, requested_at, resolved_at, resolved_by",
    )
    .bind(catalog_entry_id)
    .bind(machine_id)
    .bind(username)
    .bind(status)
    .fetch_one(pool)
    .await
}

pub async fn list_requests(
    pool: &PgPool,
    status_filter: Option<SoftwareRequestStatusDb>,
) -> Result<Vec<SoftwareRequestRow>, sqlx::Error> {
    match status_filter {
        Some(status) => {
            sqlx::query_as::<_, SoftwareRequestRow>(
                "SELECT id, catalog_entry_id, machine_id, username,
                        status, requested_at, resolved_at, resolved_by
                 FROM software_requests
                 WHERE status = $1
                 ORDER BY requested_at DESC",
            )
            .bind(status)
            .fetch_all(pool)
            .await
        }
        None => {
            sqlx::query_as::<_, SoftwareRequestRow>(
                "SELECT id, catalog_entry_id, machine_id, username,
                        status, requested_at, resolved_at, resolved_by
                 FROM software_requests
                 ORDER BY requested_at DESC",
            )
            .fetch_all(pool)
            .await
        }
    }
}

pub async fn approve_request(
    pool: &PgPool,
    id: Uuid,
    admin: &str,
) -> Result<Option<SoftwareRequestRow>, sqlx::Error> {
    sqlx::query_as::<_, SoftwareRequestRow>(
        "UPDATE software_requests
         SET status = 'approved', resolved_at = now(), resolved_by = $2
         WHERE id = $1 AND status = 'pending'
         RETURNING id, catalog_entry_id, machine_id, username,
                   status, requested_at, resolved_at, resolved_by",
    )
    .bind(id)
    .bind(admin)
    .fetch_optional(pool)
    .await
}

pub async fn deny_request(
    pool: &PgPool,
    id: Uuid,
    admin: &str,
) -> Result<Option<SoftwareRequestRow>, sqlx::Error> {
    sqlx::query_as::<_, SoftwareRequestRow>(
        "UPDATE software_requests
         SET status = 'denied', resolved_at = now(), resolved_by = $2
         WHERE id = $1 AND status = 'pending'
         RETURNING id, catalog_entry_id, machine_id, username,
                   status, requested_at, resolved_at, resolved_by",
    )
    .bind(id)
    .bind(admin)
    .fetch_optional(pool)
    .await
}

// --- Install lifecycle ---

pub async fn claim_install(
    pool: &PgPool,
    id: Uuid,
) -> Result<Option<SoftwareRequestRow>, sqlx::Error> {
    sqlx::query_as::<_, SoftwareRequestRow>(
        "UPDATE software_requests
         SET status = 'installing'
         WHERE id = $1 AND status = 'approved'
         RETURNING id, catalog_entry_id, machine_id, username,
                   status, requested_at, resolved_at, resolved_by",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn report_install_result(
    pool: &PgPool,
    id: Uuid,
    success: bool,
) -> Result<Option<SoftwareRequestRow>, sqlx::Error> {
    let new_status = if success { "installed" } else { "failed" };

    sqlx::query_as::<_, SoftwareRequestRow>(
        "UPDATE software_requests
         SET status = $2::software_request_status, resolved_at = now()
         WHERE id = $1 AND status = 'installing'
         RETURNING id, catalog_entry_id, machine_id, username,
                   status, requested_at, resolved_at, resolved_by",
    )
    .bind(id)
    .bind(new_status)
    .fetch_optional(pool)
    .await
}

pub async fn get_pending_installs(
    pool: &PgPool,
    machine_id: Uuid,
) -> Result<Vec<PendingInstallRow>, sqlx::Error> {
    sqlx::query_as::<_, PendingInstallRow>(
        "SELECT sr.id AS request_id,
                sr.username,
                sc.id AS catalog_id,
                sc.name AS catalog_name,
                sc.description AS catalog_description,
                sc.category AS catalog_category,
                sc.install_method AS catalog_install_method,
                sc.flatpak_ref AS catalog_flatpak_ref,
                sc.nix_attr AS catalog_nix_attr,
                sc.icon_url AS catalog_icon_url,
                sc.approval_required AS catalog_approval_required,
                sc.auto_approve_roles AS catalog_auto_approve_roles,
                sc.created_at AS catalog_created_at
         FROM software_requests sr
         JOIN software_catalog sc ON sr.catalog_entry_id = sc.id
         WHERE sr.machine_id = $1 AND sr.status = 'approved'",
    )
    .bind(machine_id)
    .fetch_all(pool)
    .await
}

// --- User environment queries ---

pub async fn list_user_envs(
    pool: &PgPool,
    machine_id: Uuid,
) -> Result<Vec<UserEnvironmentRow>, sqlx::Error> {
    sqlx::query_as::<_, UserEnvironmentRow>(
        "SELECT id, machine_id, username, role, current_closure, target_closure,
                status, created_at, updated_at
         FROM user_environments
         WHERE machine_id = $1
         ORDER BY username",
    )
    .bind(machine_id)
    .fetch_all(pool)
    .await
}

pub async fn get_user_env(
    pool: &PgPool,
    machine_id: Uuid,
    username: &str,
) -> Result<Option<UserEnvironmentRow>, sqlx::Error> {
    sqlx::query_as::<_, UserEnvironmentRow>(
        "SELECT id, machine_id, username, role, current_closure, target_closure,
                status, created_at, updated_at
         FROM user_environments
         WHERE machine_id = $1 AND username = $2",
    )
    .bind(machine_id)
    .bind(username)
    .fetch_optional(pool)
    .await
}

pub async fn upsert_user_env(
    pool: &PgPool,
    machine_id: Uuid,
    username: &str,
    role: &str,
    status: Option<UserEnvStatusDb>,
) -> Result<UserEnvironmentRow, sqlx::Error> {
    let status = status.unwrap_or(UserEnvStatusDb::Pending);
    sqlx::query_as::<_, UserEnvironmentRow>(
        "INSERT INTO user_environments (machine_id, username, role, status)
         VALUES ($1, $2, $3, $4)
         ON CONFLICT (machine_id, username)
         DO UPDATE SET role = $3, status = $4, updated_at = now()
         RETURNING id, machine_id, username, role, current_closure, target_closure,
                   status, created_at, updated_at",
    )
    .bind(machine_id)
    .bind(username)
    .bind(role)
    .bind(status)
    .fetch_one(pool)
    .await
}

#[allow(dead_code)]
pub async fn update_user_env_status(
    pool: &PgPool,
    id: Uuid,
    new_status: UserEnvStatusDb,
) -> Result<Option<UserEnvironmentRow>, sqlx::Error> {
    sqlx::query_as::<_, UserEnvironmentRow>(
        "UPDATE user_environments SET status = $2, updated_at = now()
         WHERE id = $1
         RETURNING id, machine_id, username, role, current_closure, target_closure,
                   status, created_at, updated_at",
    )
    .bind(id)
    .bind(new_status)
    .fetch_optional(pool)
    .await
}

pub async fn record_user_login(
    pool: &PgPool,
    machine_id: Uuid,
    username: &str,
) -> Result<Option<UserEnvironmentRow>, sqlx::Error> {
    sqlx::query_as::<_, UserEnvironmentRow>(
        "UPDATE user_environments SET updated_at = now()
         WHERE machine_id = $1 AND username = $2
         RETURNING id, machine_id, username, role, current_closure, target_closure,
                   status, created_at, updated_at",
    )
    .bind(machine_id)
    .bind(username)
    .fetch_optional(pool)
    .await
}

// --- Enrollment queries ---

pub async fn enroll_machine(
    pool: &PgPool,
    hostname: &str,
    hardware_fingerprint: Option<&str>,
) -> Result<MachineRow, sqlx::Error> {
    sqlx::query_as::<_, MachineRow>(
        "INSERT INTO machines (hostname, hardware_fingerprint, enrollment_status)
         VALUES ($1, $2, 'pending')
         RETURNING id, hostname, hardware_fingerprint, enrollment_status,
                   current_closure, target_closure, rollback_closure,
                   role, tags, extra_config, last_heartbeat,
                   created_at, updated_at",
    )
    .bind(hostname)
    .bind(hardware_fingerprint)
    .fetch_one(pool)
    .await
}

pub async fn approve_enrollment(
    pool: &PgPool,
    id: Uuid,
    role: &str,
    target_closure: Option<&str>,
    extra_config: Option<&serde_json::Value>,
) -> Result<Option<MachineRow>, sqlx::Error> {
    sqlx::query_as::<_, MachineRow>(
        "UPDATE machines SET
            enrollment_status = 'approved',
            role = $2,
            target_closure = COALESCE($3, target_closure),
            extra_config = COALESCE($4, extra_config),
            updated_at = now()
         WHERE id = $1 AND enrollment_status = 'pending'
         RETURNING id, hostname, hardware_fingerprint, enrollment_status,
                   current_closure, target_closure, rollback_closure,
                   role, tags, extra_config, last_heartbeat,
                   created_at, updated_at",
    )
    .bind(id)
    .bind(role)
    .bind(target_closure)
    .bind(extra_config)
    .fetch_optional(pool)
    .await
}

// --- Role closure queries ---

pub async fn list_role_closures(pool: &PgPool) -> Result<Vec<RoleClosureRow>, sqlx::Error> {
    sqlx::query_as::<_, RoleClosureRow>(
        "SELECT role, closure, built_at, updated_at
         FROM role_closures
         ORDER BY role",
    )
    .fetch_all(pool)
    .await
}

pub async fn get_role_closure(
    pool: &PgPool,
    role: &str,
) -> Result<Option<RoleClosureRow>, sqlx::Error> {
    sqlx::query_as::<_, RoleClosureRow>(
        "SELECT role, closure, built_at, updated_at
         FROM role_closures
         WHERE role = $1",
    )
    .bind(role)
    .fetch_optional(pool)
    .await
}

pub async fn upsert_role_closure(
    pool: &PgPool,
    role: &str,
    closure: &str,
) -> Result<RoleClosureRow, sqlx::Error> {
    sqlx::query_as::<_, RoleClosureRow>(
        "INSERT INTO role_closures (role, closure)
         VALUES ($1, $2)
         ON CONFLICT (role)
         DO UPDATE SET closure = $2, updated_at = now()
         RETURNING role, closure, built_at, updated_at",
    )
    .bind(role)
    .bind(closure)
    .fetch_one(pool)
    .await
}

pub async fn delete_role_closure(pool: &PgPool, role: &str) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("DELETE FROM role_closures WHERE role = $1")
        .bind(role)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

// --- Deployment queries ---

const DEPLOYMENT_COLUMNS: &str = "id, closure, module_library_ref, instance_data_hash, status,
    target_filter, total_machines, succeeded, failed, canary_size, batch_size,
    failure_threshold, rollback_reason, created_at, updated_at";

pub async fn create_deployment(
    pool: &PgPool,
    req: &CreateDeploymentRequest,
) -> Result<DeploymentRow, sqlx::Error> {
    let target_filter = req.target_filter.clone().unwrap_or(serde_json::json!({}));
    let module_library_ref = req.module_library_ref.clone().unwrap_or_default();
    let instance_data_hash = req.instance_data_hash.clone().unwrap_or_default();

    sqlx::query_as::<_, DeploymentRow>(&format!(
        "INSERT INTO deployments
                (closure, module_library_ref, instance_data_hash, target_filter,
                 canary_size, batch_size, failure_threshold)
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             RETURNING {DEPLOYMENT_COLUMNS}"
    ))
    .bind(&req.closure)
    .bind(&module_library_ref)
    .bind(&instance_data_hash)
    .bind(&target_filter)
    .bind(req.canary_size)
    .bind(req.batch_size)
    .bind(req.failure_threshold)
    .fetch_one(pool)
    .await
}

pub async fn list_deployments(
    pool: &PgPool,
    status: Option<DeploymentStatusDb>,
) -> Result<Vec<DeploymentRow>, sqlx::Error> {
    match status {
        Some(s) => {
            sqlx::query_as::<_, DeploymentRow>(&format!(
                "SELECT {DEPLOYMENT_COLUMNS}
                     FROM deployments
                     WHERE status = $1
                     ORDER BY created_at DESC"
            ))
            .bind(s)
            .fetch_all(pool)
            .await
        }
        None => {
            sqlx::query_as::<_, DeploymentRow>(&format!(
                "SELECT {DEPLOYMENT_COLUMNS}
                     FROM deployments
                     ORDER BY created_at DESC"
            ))
            .fetch_all(pool)
            .await
        }
    }
}

pub async fn get_deployment(pool: &PgPool, id: Uuid) -> Result<Option<DeploymentRow>, sqlx::Error> {
    sqlx::query_as::<_, DeploymentRow>(&format!(
        "SELECT {DEPLOYMENT_COLUMNS}
             FROM deployments
             WHERE id = $1"
    ))
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn update_deployment_status(
    pool: &PgPool,
    id: Uuid,
    status: DeploymentStatusDb,
) -> Result<Option<DeploymentRow>, sqlx::Error> {
    sqlx::query_as::<_, DeploymentRow>(&format!(
        "UPDATE deployments SET status = $2, updated_at = now()
             WHERE id = $1
             RETURNING {DEPLOYMENT_COLUMNS}"
    ))
    .bind(id)
    .bind(status)
    .fetch_optional(pool)
    .await
}

pub async fn rollback_deployment(
    pool: &PgPool,
    id: Uuid,
    reason: &str,
) -> Result<Option<DeploymentRow>, sqlx::Error> {
    sqlx::query_as::<_, DeploymentRow>(&format!(
        "UPDATE deployments
             SET status = 'rolled_back'::deployment_status,
                 rollback_reason = $2,
                 updated_at = now()
             WHERE id = $1
             RETURNING {DEPLOYMENT_COLUMNS}"
    ))
    .bind(id)
    .bind(reason)
    .fetch_optional(pool)
    .await
}

pub async fn get_deployment_machines(
    pool: &PgPool,
    deployment_id: Uuid,
) -> Result<Vec<DeploymentMachineRow>, sqlx::Error> {
    sqlx::query_as::<_, DeploymentMachineRow>(
        "SELECT deployment_id, machine_id, status, started_at, completed_at, error_message
         FROM deployment_machines
         WHERE deployment_id = $1
         ORDER BY machine_id",
    )
    .bind(deployment_id)
    .fetch_all(pool)
    .await
}

pub async fn upsert_deployment_machine(
    pool: &PgPool,
    deployment_id: Uuid,
    machine_id: Uuid,
    status: MachineUpdateStatusDb,
    error_message: Option<&str>,
) -> Result<DeploymentMachineRow, sqlx::Error> {
    sqlx::query_as::<_, DeploymentMachineRow>(
        "INSERT INTO deployment_machines (deployment_id, machine_id, status, started_at, error_message)
         VALUES ($1, $2, $3, now(), $4)
         ON CONFLICT (deployment_id, machine_id)
         DO UPDATE SET status = $3,
                       error_message = COALESCE($4, deployment_machines.error_message),
                       completed_at = CASE
                           WHEN $3 IN ('completed'::machine_update_status, 'failed'::machine_update_status, 'rolled_back'::machine_update_status)
                           THEN now()
                           ELSE deployment_machines.completed_at
                       END
         RETURNING deployment_id, machine_id, status, started_at, completed_at, error_message",
    )
    .bind(deployment_id)
    .bind(machine_id)
    .bind(status)
    .bind(error_message)
    .fetch_one(pool)
    .await
}

pub async fn increment_deployment_counter(
    pool: &PgPool,
    deployment_id: Uuid,
    success: bool,
) -> Result<(), sqlx::Error> {
    if success {
        sqlx::query(
            "UPDATE deployments SET succeeded = succeeded + 1, updated_at = now()
             WHERE id = $1",
        )
        .bind(deployment_id)
        .execute(pool)
        .await?;
    } else {
        sqlx::query(
            "UPDATE deployments SET failed = failed + 1, updated_at = now()
             WHERE id = $1",
        )
        .bind(deployment_id)
        .execute(pool)
        .await?;
    }
    Ok(())
}

pub async fn get_active_deployment_for_machine(
    pool: &PgPool,
    machine_id: Uuid,
) -> Result<Option<ActiveDeploymentRow>, sqlx::Error> {
    sqlx::query_as::<_, ActiveDeploymentRow>(
        "SELECT dm.deployment_id
         FROM deployment_machines dm
         JOIN deployments d ON d.id = dm.deployment_id
         WHERE dm.machine_id = $1
           AND dm.status NOT IN ('completed'::machine_update_status, 'failed'::machine_update_status, 'rolled_back'::machine_update_status)
           AND d.status NOT IN ('completed'::deployment_status, 'failed'::deployment_status, 'rolled_back'::deployment_status)
         ORDER BY d.created_at DESC
         LIMIT 1",
    )
    .bind(machine_id)
    .fetch_optional(pool)
    .await
}

pub async fn get_deployment_closure(
    pool: &PgPool,
    deployment_id: Uuid,
) -> Result<Option<DeploymentClosureRow>, sqlx::Error> {
    sqlx::query_as::<_, DeploymentClosureRow>("SELECT closure FROM deployments WHERE id = $1")
        .bind(deployment_id)
        .fetch_optional(pool)
        .await
}

// --- Audit queries ---

pub async fn list_audit_events(
    pool: &PgPool,
    event_type: Option<&str>,
    machine_id: Option<Uuid>,
    actor: Option<&str>,
    since: Option<DateTime<Utc>>,
    limit: i64,
) -> Result<Vec<AuditEventRow>, sqlx::Error> {
    // Build the query dynamically based on which filters are provided.
    // We always add all bind parameters in the same order, using a WHERE TRUE
    // clause to simplify conditional ANDs.
    sqlx::query_as::<_, AuditEventRow>(
        "SELECT id, event_type, actor, machine_id, details, created_at
         FROM audit_events
         WHERE ($1::text IS NULL OR event_type = $1)
           AND ($2::uuid IS NULL OR machine_id = $2)
           AND ($3::text IS NULL OR actor = $3)
           AND ($4::timestamptz IS NULL OR created_at >= $4)
         ORDER BY created_at DESC
         LIMIT $5",
    )
    .bind(event_type)
    .bind(machine_id)
    .bind(actor)
    .bind(since)
    .bind(limit)
    .fetch_all(pool)
    .await
}

// --- Stats queries ---

#[derive(Debug, sqlx::FromRow)]
struct StatsRow {
    total_machines: Option<i64>,
    active_machines: Option<i64>,
    pending_enrollments: Option<i64>,
    active_deployments: Option<i64>,
    pending_requests: Option<i64>,
}

pub async fn get_fleet_stats(pool: &PgPool) -> Result<FleetStats, sqlx::Error> {
    let row = sqlx::query_as::<_, StatsRow>(
        "SELECT
            (SELECT COUNT(*) FROM machines) AS total_machines,
            (SELECT COUNT(*) FROM machines WHERE enrollment_status = 'active') AS active_machines,
            (SELECT COUNT(*) FROM machines WHERE enrollment_status = 'pending') AS pending_enrollments,
            (SELECT COUNT(*) FROM deployments WHERE status IN ('pending'::deployment_status, 'canary'::deployment_status, 'rolling'::deployment_status)) AS active_deployments,
            (SELECT COUNT(*) FROM software_requests WHERE status = 'pending') AS pending_requests",
    )
    .fetch_one(pool)
    .await?;

    Ok(FleetStats {
        total_machines: row.total_machines.unwrap_or(0),
        active_machines: row.active_machines.unwrap_or(0),
        pending_enrollments: row.pending_enrollments.unwrap_or(0),
        active_deployments: row.active_deployments.unwrap_or(0),
        pending_requests: row.pending_requests.unwrap_or(0),
    })
}
