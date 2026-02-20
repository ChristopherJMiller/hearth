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
    ActionTypeDb, ActiveDeploymentRow, AuditEventRow, BuildJobRow, BuildJobStatusDb,
    CatalogEntryRow, DeploymentClosureRow, DeploymentMachineRow, DeploymentRow, DeploymentStatusDb,
    HeartbeatResultRow, InstallMethodDb, MachineRow, MachineUpdateStatusDb, PendingActionRow,
    PendingInstallRow, PendingUserEnvRow, RoleClosureRow, SoftwareRequestRow,
    SoftwareRequestStatusDb, TargetStateRow, UserEnvStatusDb, UserEnvironmentRow, UserRow,
};
use crate::routes::reports::{ComplianceReport, DeploymentTimelineEntry, EnrollmentTimelineEntry};

const MACHINE_COLUMNS: &str = "id, hostname, hardware_fingerprint, enrollment_status,
    current_closure, target_closure, rollback_closure,
    role, tags, extra_config, last_heartbeat,
    enrolled_by, machine_token_hash,
    hardware_report, serial_number, hardware_config, hardware_profile,
    instance_data_hash, module_library_ref,
    created_at, updated_at";

pub async fn list_machines(pool: &PgPool) -> Result<Vec<MachineRow>, sqlx::Error> {
    sqlx::query_as::<_, MachineRow>(&format!(
        "SELECT {MACHINE_COLUMNS} FROM machines ORDER BY created_at DESC"
    ))
    .fetch_all(pool)
    .await
}

pub async fn get_machine(pool: &PgPool, id: Uuid) -> Result<Option<MachineRow>, sqlx::Error> {
    sqlx::query_as::<_, MachineRow>(&format!(
        "SELECT {MACHINE_COLUMNS} FROM machines WHERE id = $1"
    ))
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn create_machine(
    pool: &PgPool,
    req: &CreateMachineRequest,
) -> Result<MachineRow, sqlx::Error> {
    let tags = req.tags.clone().unwrap_or_default();

    sqlx::query_as::<_, MachineRow>(&format!(
        "INSERT INTO machines (hostname, hardware_fingerprint, role, tags)
         VALUES ($1, $2, $3, $4)
         RETURNING {MACHINE_COLUMNS}"
    ))
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
    sqlx::query_as::<_, MachineRow>(&format!(
        "UPDATE machines SET
            hostname = COALESCE($2, hostname),
            role = COALESCE($3, role),
            tags = COALESCE($4, tags),
            target_closure = COALESCE($5, target_closure),
            extra_config = COALESCE($6, extra_config),
            updated_at = now()
         WHERE id = $1
         RETURNING {MACHINE_COLUMNS}"
    ))
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

            // Fetch pending remote actions
            let actions = get_pending_actions_for_machine(pool, req.machine_id).await?;
            let action_ids: Vec<Uuid> = actions.iter().map(|a| a.id).collect();
            let pending_actions: Vec<hearth_common::api_types::PendingAction> =
                actions.into_iter().map(Into::into).collect();

            // Mark fetched actions as delivered
            if !action_ids.is_empty() {
                deliver_actions(pool, &action_ids).await?;
            }

            // Fetch pending user environment activations
            let user_envs = get_pending_user_envs(pool, req.machine_id).await?;
            let pending_user_envs: Vec<hearth_common::api_types::PendingUserEnv> = user_envs
                .into_iter()
                .map(|e| hearth_common::api_types::PendingUserEnv {
                    username: e.username,
                    target_closure: e.target_closure,
                    cache_url: None,
                })
                .collect();

            Ok(Some(HeartbeatResponse {
                target_closure: r.target_closure,
                pending_installs: installs.into_iter().map(Into::into).collect(),
                active_deployment_id,
                cache_url: None,
                cache_token: None,
                machine_token: None,
                pending_actions,
                pending_user_envs,
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
    enrolled_by: Option<&str>,
    hardware_report: Option<&serde_json::Value>,
    serial_number: Option<&str>,
    hardware_config: Option<&str>,
) -> Result<MachineRow, sqlx::Error> {
    sqlx::query_as::<_, MachineRow>(&format!(
        "INSERT INTO machines (hostname, hardware_fingerprint, enrollment_status, enrolled_by,
             hardware_report, serial_number, hardware_config)
         VALUES ($1, $2, 'pending', $3, $4, $5, $6)
         RETURNING {MACHINE_COLUMNS}"
    ))
    .bind(hostname)
    .bind(hardware_fingerprint)
    .bind(enrolled_by)
    .bind(hardware_report)
    .bind(serial_number)
    .bind(hardware_config)
    .fetch_one(pool)
    .await
}

pub async fn approve_enrollment(
    pool: &PgPool,
    id: Uuid,
    role: &str,
    target_closure: Option<&str>,
    extra_config: Option<&serde_json::Value>,
    machine_token_hash: Option<&str>,
) -> Result<Option<MachineRow>, sqlx::Error> {
    sqlx::query_as::<_, MachineRow>(&format!(
        "UPDATE machines SET
            enrollment_status = 'approved',
            role = $2,
            target_closure = COALESCE($3, target_closure),
            extra_config = COALESCE($4, extra_config),
            machine_token_hash = $5,
            updated_at = now()
         WHERE id = $1 AND enrollment_status = 'pending'
         RETURNING {MACHINE_COLUMNS}"
    ))
    .bind(id)
    .bind(role)
    .bind(target_closure)
    .bind(extra_config)
    .bind(machine_token_hash)
    .fetch_optional(pool)
    .await
}

pub async fn set_machine_token_hash(
    pool: &PgPool,
    id: Uuid,
    hash: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE machines SET machine_token_hash = $2, updated_at = now() WHERE id = $1")
        .bind(id)
        .bind(hash)
        .execute(pool)
        .await?;
    Ok(())
}

// --- User queries ---

pub async fn upsert_user(
    pool: &PgPool,
    username: &str,
    display_name: Option<&str>,
    email: Option<&str>,
    kanidm_uuid: Option<&str>,
    groups: &[String],
) -> Result<UserRow, sqlx::Error> {
    sqlx::query_as::<_, UserRow>(
        "INSERT INTO users (username, display_name, email, kanidm_uuid, groups, last_seen)
         VALUES ($1, $2, $3, $4, $5, now())
         ON CONFLICT (username)
         DO UPDATE SET
            display_name = COALESCE($2, users.display_name),
            email = COALESCE($3, users.email),
            kanidm_uuid = COALESCE($4, users.kanidm_uuid),
            groups = $5,
            last_seen = now(),
            updated_at = now()
         RETURNING id, username, display_name, email, kanidm_uuid, groups,
                   last_seen, created_at, updated_at",
    )
    .bind(username)
    .bind(display_name)
    .bind(email)
    .bind(kanidm_uuid)
    .bind(groups)
    .fetch_one(pool)
    .await
}

pub async fn get_user_by_username(
    pool: &PgPool,
    username: &str,
) -> Result<Option<UserRow>, sqlx::Error> {
    sqlx::query_as::<_, UserRow>(
        "SELECT id, username, display_name, email, kanidm_uuid, groups,
                last_seen, created_at, updated_at
         FROM users
         WHERE username = $1",
    )
    .bind(username)
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

// --- Build job queries ---

const BUILD_JOB_COLUMNS: &str = "id, status, flake_ref, target_filter, canary_size, batch_size,
    failure_threshold, worker_id, claimed_at, deployment_id, closure, closures_built,
    closures_pushed, total_machines, error_message, created_at, updated_at";

pub async fn enqueue_build_job(
    pool: &PgPool,
    flake_ref: &str,
    target_filter: Option<&serde_json::Value>,
    canary_size: i32,
    batch_size: i32,
    failure_threshold: f64,
) -> Result<BuildJobRow, sqlx::Error> {
    sqlx::query_as::<_, BuildJobRow>(&format!(
        "INSERT INTO build_jobs (flake_ref, target_filter, canary_size, batch_size, failure_threshold)
         VALUES ($1, $2, $3, $4, $5)
         RETURNING {BUILD_JOB_COLUMNS}"
    ))
    .bind(flake_ref)
    .bind(target_filter)
    .bind(canary_size)
    .bind(batch_size)
    .bind(failure_threshold)
    .fetch_one(pool)
    .await
}

/// Claim the next pending build job using `FOR UPDATE SKIP LOCKED`.
/// Returns `None` if no jobs are available.
pub async fn claim_build_job(
    pool: &PgPool,
    worker_id: &str,
) -> Result<Option<BuildJobRow>, sqlx::Error> {
    sqlx::query_as::<_, BuildJobRow>(&format!(
        "UPDATE build_jobs SET
            status = 'claimed'::build_job_status,
            worker_id = $1,
            claimed_at = now(),
            updated_at = now()
         WHERE id = (
            SELECT id FROM build_jobs
            WHERE status = 'pending'::build_job_status
            ORDER BY created_at ASC
            FOR UPDATE SKIP LOCKED
            LIMIT 1
         )
         RETURNING {BUILD_JOB_COLUMNS}"
    ))
    .bind(worker_id)
    .fetch_optional(pool)
    .await
}

pub async fn update_build_job_status(
    pool: &PgPool,
    id: Uuid,
    status: BuildJobStatusDb,
) -> Result<Option<BuildJobRow>, sqlx::Error> {
    sqlx::query_as::<_, BuildJobRow>(&format!(
        "UPDATE build_jobs SET status = $2, updated_at = now()
         WHERE id = $1
         RETURNING {BUILD_JOB_COLUMNS}"
    ))
    .bind(id)
    .bind(status)
    .fetch_optional(pool)
    .await
}

pub async fn complete_build_job(
    pool: &PgPool,
    id: Uuid,
    deployment_id: Uuid,
    closure: &str,
    closures_built: i32,
    closures_pushed: i32,
    total_machines: i32,
) -> Result<Option<BuildJobRow>, sqlx::Error> {
    sqlx::query_as::<_, BuildJobRow>(&format!(
        "UPDATE build_jobs SET
            status = 'completed'::build_job_status,
            deployment_id = $2,
            closure = $3,
            closures_built = $4,
            closures_pushed = $5,
            total_machines = $6,
            updated_at = now()
         WHERE id = $1
         RETURNING {BUILD_JOB_COLUMNS}"
    ))
    .bind(id)
    .bind(deployment_id)
    .bind(closure)
    .bind(closures_built)
    .bind(closures_pushed)
    .bind(total_machines)
    .fetch_optional(pool)
    .await
}

pub async fn fail_build_job(
    pool: &PgPool,
    id: Uuid,
    error_message: &str,
) -> Result<Option<BuildJobRow>, sqlx::Error> {
    sqlx::query_as::<_, BuildJobRow>(&format!(
        "UPDATE build_jobs SET
            status = 'failed'::build_job_status,
            error_message = $2,
            updated_at = now()
         WHERE id = $1
         RETURNING {BUILD_JOB_COLUMNS}"
    ))
    .bind(id)
    .bind(error_message)
    .fetch_optional(pool)
    .await
}

pub async fn get_build_job(pool: &PgPool, id: Uuid) -> Result<Option<BuildJobRow>, sqlx::Error> {
    sqlx::query_as::<_, BuildJobRow>(&format!(
        "SELECT {BUILD_JOB_COLUMNS} FROM build_jobs WHERE id = $1"
    ))
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn list_build_jobs(
    pool: &PgPool,
    status: Option<BuildJobStatusDb>,
) -> Result<Vec<BuildJobRow>, sqlx::Error> {
    match status {
        Some(s) => sqlx::query_as::<_, BuildJobRow>(&format!(
            "SELECT {BUILD_JOB_COLUMNS} FROM build_jobs WHERE status = $1 ORDER BY created_at DESC"
        ))
        .bind(s)
        .fetch_all(pool)
        .await,
        None => {
            sqlx::query_as::<_, BuildJobRow>(&format!(
                "SELECT {BUILD_JOB_COLUMNS} FROM build_jobs ORDER BY created_at DESC"
            ))
            .fetch_all(pool)
            .await
        }
    }
}

// --- Remote action queries ---

const ACTION_COLUMNS: &str = "id, machine_id, action_type, payload, status,
    created_by, created_at, delivered_at, completed_at, result";

pub async fn create_action(
    pool: &PgPool,
    machine_id: Uuid,
    action_type: hearth_common::api_types::ActionType,
    payload: &serde_json::Value,
    created_by: &str,
) -> Result<PendingActionRow, sqlx::Error> {
    let action_type_db = ActionTypeDb::from(action_type);
    sqlx::query_as::<_, PendingActionRow>(&format!(
        "INSERT INTO pending_actions (machine_id, action_type, payload, created_by)
         VALUES ($1, $2, $3, $4)
         RETURNING {ACTION_COLUMNS}"
    ))
    .bind(machine_id)
    .bind(action_type_db)
    .bind(payload)
    .bind(created_by)
    .fetch_one(pool)
    .await
}

pub async fn get_pending_actions_for_machine(
    pool: &PgPool,
    machine_id: Uuid,
) -> Result<Vec<PendingActionRow>, sqlx::Error> {
    sqlx::query_as::<_, PendingActionRow>(&format!(
        "SELECT {ACTION_COLUMNS}
         FROM pending_actions
         WHERE machine_id = $1 AND status = 'pending'
         ORDER BY created_at ASC"
    ))
    .bind(machine_id)
    .fetch_all(pool)
    .await
}

pub async fn deliver_actions(pool: &PgPool, action_ids: &[Uuid]) -> Result<(), sqlx::Error> {
    if action_ids.is_empty() {
        return Ok(());
    }
    sqlx::query(
        "UPDATE pending_actions SET status = 'delivered', delivered_at = now()
         WHERE id = ANY($1) AND status = 'pending'",
    )
    .bind(action_ids)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn complete_action(
    pool: &PgPool,
    action_id: Uuid,
    success: bool,
    result: Option<&serde_json::Value>,
) -> Result<Option<PendingActionRow>, sqlx::Error> {
    let new_status = if success { "completed" } else { "failed" };
    sqlx::query_as::<_, PendingActionRow>(&format!(
        "UPDATE pending_actions
         SET status = $2::action_status, completed_at = now(), result = $3
         WHERE id = $1
         RETURNING {ACTION_COLUMNS}"
    ))
    .bind(action_id)
    .bind(new_status)
    .bind(result)
    .fetch_optional(pool)
    .await
}

pub async fn list_machine_actions(
    pool: &PgPool,
    machine_id: Uuid,
) -> Result<Vec<PendingActionRow>, sqlx::Error> {
    sqlx::query_as::<_, PendingActionRow>(&format!(
        "SELECT {ACTION_COLUMNS}
         FROM pending_actions
         WHERE machine_id = $1
         ORDER BY created_at DESC
         LIMIT 100"
    ))
    .bind(machine_id)
    .fetch_all(pool)
    .await
}

// --- Audit event creation ---

pub async fn create_audit_event(
    pool: &PgPool,
    event_type: &str,
    actor: Option<&str>,
    machine_id: Option<Uuid>,
    details: &serde_json::Value,
) -> Result<AuditEventRow, sqlx::Error> {
    sqlx::query_as::<_, AuditEventRow>(
        "INSERT INTO audit_events (event_type, actor, machine_id, details)
         VALUES ($1, $2, $3, $4)
         RETURNING id, event_type, actor, machine_id, details, created_at",
    )
    .bind(event_type)
    .bind(actor)
    .bind(machine_id)
    .bind(details)
    .fetch_one(pool)
    .await
}

// --- Pending user environments (for heartbeat) ---

pub async fn get_pending_user_envs(
    pool: &PgPool,
    machine_id: Uuid,
) -> Result<Vec<PendingUserEnvRow>, sqlx::Error> {
    sqlx::query_as::<_, PendingUserEnvRow>(
        "SELECT username, target_closure
         FROM user_environments
         WHERE machine_id = $1
           AND target_closure IS NOT NULL
           AND (current_closure IS NULL OR current_closure != target_closure)",
    )
    .bind(machine_id)
    .fetch_all(pool)
    .await
}

// --- User list (for identity sync) ---

pub async fn list_users(pool: &PgPool) -> Result<Vec<UserRow>, sqlx::Error> {
    sqlx::query_as::<_, UserRow>(
        "SELECT id, username, display_name, email, kanidm_uuid, groups,
                last_seen, created_at, updated_at
         FROM users
         ORDER BY username",
    )
    .fetch_all(pool)
    .await
}

// --- Compliance report ---

#[derive(Debug, sqlx::FromRow)]
struct ComplianceRow {
    total: Option<i64>,
    compliant: Option<i64>,
    drifted: Option<i64>,
    no_target: Option<i64>,
}

pub async fn get_compliance_report(pool: &PgPool) -> Result<ComplianceReport, sqlx::Error> {
    let row = sqlx::query_as::<_, ComplianceRow>(
        "SELECT
            COUNT(*) AS total,
            COUNT(*) FILTER (WHERE target_closure IS NOT NULL AND current_closure = target_closure) AS compliant,
            COUNT(*) FILTER (WHERE target_closure IS NOT NULL AND (current_closure IS NULL OR current_closure != target_closure)) AS drifted,
            COUNT(*) FILTER (WHERE target_closure IS NULL) AS no_target
         FROM machines
         WHERE enrollment_status = 'active'",
    )
    .fetch_one(pool)
    .await?;

    Ok(ComplianceReport {
        total: row.total.unwrap_or(0),
        compliant: row.compliant.unwrap_or(0),
        drifted: row.drifted.unwrap_or(0),
        no_target: row.no_target.unwrap_or(0),
    })
}

// --- Deployment timeline ---

#[derive(Debug, sqlx::FromRow)]
struct DeploymentTimelineRow {
    date: String,
    completed: Option<i64>,
    failed: Option<i64>,
    rolled_back: Option<i64>,
}

pub async fn get_deployment_timeline(
    pool: &PgPool,
    days: i64,
) -> Result<Vec<DeploymentTimelineEntry>, sqlx::Error> {
    let rows = sqlx::query_as::<_, DeploymentTimelineRow>(
        "SELECT
            to_char(created_at::date, 'YYYY-MM-DD') AS date,
            COUNT(*) FILTER (WHERE status = 'completed') AS completed,
            COUNT(*) FILTER (WHERE status = 'failed') AS failed,
            COUNT(*) FILTER (WHERE status = 'rolled_back') AS rolled_back
         FROM deployments
         WHERE created_at >= now() - ($1 || ' days')::interval
         GROUP BY created_at::date
         ORDER BY created_at::date",
    )
    .bind(days.to_string())
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| DeploymentTimelineEntry {
            date: r.date,
            completed: r.completed.unwrap_or(0),
            failed: r.failed.unwrap_or(0),
            rolled_back: r.rolled_back.unwrap_or(0),
        })
        .collect())
}

// --- Enrollment timeline ---

#[derive(Debug, sqlx::FromRow)]
struct EnrollmentTimelineRow {
    date: String,
    enrolled: Option<i64>,
    pending: Option<i64>,
}

pub async fn get_enrollment_timeline(
    pool: &PgPool,
    days: i64,
) -> Result<Vec<EnrollmentTimelineEntry>, sqlx::Error> {
    let rows = sqlx::query_as::<_, EnrollmentTimelineRow>(
        "SELECT
            to_char(created_at::date, 'YYYY-MM-DD') AS date,
            COUNT(*) FILTER (WHERE enrollment_status IN ('active', 'enrolled', 'approved')) AS enrolled,
            COUNT(*) FILTER (WHERE enrollment_status = 'pending') AS pending
         FROM machines
         WHERE created_at >= now() - ($1 || ' days')::interval
         GROUP BY created_at::date
         ORDER BY created_at::date",
    )
    .bind(days.to_string())
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| EnrollmentTimelineEntry {
            date: r.date,
            enrolled: r.enrolled.unwrap_or(0),
            pending: r.pending.unwrap_or(0),
        })
        .collect())
}
