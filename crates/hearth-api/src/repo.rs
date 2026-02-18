//! Repository layer: database queries for machines, heartbeats, etc.

use hearth_common::api_types::{
    CreateCatalogEntryRequest, CreateMachineRequest, HeartbeatRequest, HeartbeatResponse,
    TargetState, UpdateCatalogEntryRequest, UpdateMachineRequest,
};
use sqlx::PgPool;
use uuid::Uuid;

use crate::db::{
    CatalogEntryRow, HeartbeatResultRow, InstallMethodDb, MachineRow, PendingInstallRow,
    SoftwareRequestRow, SoftwareRequestStatusDb, TargetStateRow, UserEnvStatusDb,
    UserEnvironmentRow,
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
            Ok(Some(HeartbeatResponse {
                target_closure: r.target_closure,
                pending_installs: installs.into_iter().map(Into::into).collect(),
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
) -> Result<Option<MachineRow>, sqlx::Error> {
    sqlx::query_as::<_, MachineRow>(
        "UPDATE machines SET enrollment_status = 'approved', role = $2, updated_at = now()
         WHERE id = $1 AND enrollment_status = 'pending'
         RETURNING id, hostname, hardware_fingerprint, enrollment_status,
                   current_closure, target_closure, rollback_closure,
                   role, tags, extra_config, last_heartbeat,
                   created_at, updated_at",
    )
    .bind(id)
    .bind(role)
    .fetch_optional(pool)
    .await
}
