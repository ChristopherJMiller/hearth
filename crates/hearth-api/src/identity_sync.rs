//! Identity synchronisation background job.
//!
//! Periodically queries Kanidm's REST API to:
//! 1. Ensure all `hearth-users` group members have POSIX attributes
//!    (`posixaccount` class + unix password) so they can log in via PAM.
//! 2. Ensure the group itself has `posixgroup` class.
//! 3. Upsert discovered users into the local database so the directory
//!    stays in sync with Kanidm even for users who haven't logged in yet.

use rand::Rng;
use serde::Deserialize;
use sqlx::PgPool;
use std::collections::HashMap;
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use crate::auth::{self, AuthConfig};
use crate::repo;

// ---------------------------------------------------------------------------
// Kanidm REST API client
// ---------------------------------------------------------------------------

struct KanidmClient {
    http: reqwest::Client,
    base_url: String,
    token: String,
}

/// Generic Kanidm entry — all entity types share this shape.
#[derive(Debug, Deserialize)]
struct KanidmEntry {
    attrs: HashMap<String, Vec<String>>,
}

impl KanidmEntry {
    fn classes(&self) -> &[String] {
        self.attrs.get("class").map(|v| v.as_slice()).unwrap_or(&[])
    }

    fn has_class(&self, class: &str) -> bool {
        self.classes().iter().any(|c| c == class)
    }

    fn first(&self, attr: &str) -> Option<&str> {
        self.attrs
            .get(attr)
            .and_then(|v| v.first())
            .map(|s| s.as_str())
    }

    fn list(&self, attr: &str) -> &[String] {
        self.attrs.get(attr).map(|v| v.as_slice()).unwrap_or(&[])
    }
}

impl KanidmClient {
    async fn get_group(&self, name: &str) -> Result<Option<KanidmEntry>, reqwest::Error> {
        let resp = self
            .http
            .get(format!("{}/v1/group/{}", self.base_url, name))
            .header("Authorization", format!("Bearer {}", self.token))
            .send()
            .await?;
        Self::parse_entry(resp).await
    }

    async fn get_person(&self, id: &str) -> Result<Option<KanidmEntry>, reqwest::Error> {
        let resp = self
            .http
            .get(format!("{}/v1/person/{}", self.base_url, id))
            .header("Authorization", format!("Bearer {}", self.token))
            .send()
            .await?;
        Self::parse_entry(resp).await
    }

    async fn ensure_posixgroup(&self, name: &str) -> Result<bool, reqwest::Error> {
        let entry = self.get_group(name).await?;
        let Some(entry) = entry else { return Ok(false) };
        if entry.has_class("posixgroup") {
            return Ok(false);
        }
        self.http
            .post(format!("{}/v1/group/{}/_attr/class", self.base_url, name))
            .header("Authorization", format!("Bearer {}", self.token))
            .json(&["posixgroup"])
            .send()
            .await?;
        Ok(true)
    }

    async fn add_posixaccount(&self, username: &str) -> Result<(), reqwest::Error> {
        self.http
            .post(format!(
                "{}/v1/person/{}/_attr/class",
                self.base_url, username
            ))
            .header("Authorization", format!("Bearer {}", self.token))
            .json(&["posixaccount"])
            .send()
            .await?;
        Ok(())
    }

    async fn set_unix_password(
        &self,
        username: &str,
        password: &str,
    ) -> Result<(), reqwest::Error> {
        let body = serde_json::json!({"value": password});
        self.http
            .put(format!(
                "{}/v1/person/{}/_unix/_credential",
                self.base_url, username
            ))
            .header("Authorization", format!("Bearer {}", self.token))
            .json(&body)
            .send()
            .await?;
        Ok(())
    }

    /// Parse a Kanidm response. Returns `None` if the response body is `"null"` or
    /// a JSON string (error like `"accessdenied"`).
    async fn parse_entry(resp: reqwest::Response) -> Result<Option<KanidmEntry>, reqwest::Error> {
        let text = resp.text().await?;
        if text == "null" || text.starts_with('"') {
            return Ok(None);
        }
        match serde_json::from_str::<KanidmEntry>(&text) {
            Ok(entry) => Ok(Some(entry)),
            Err(e) => {
                warn!(error = %e, body = %text, "failed to parse Kanidm response");
                Ok(None)
            }
        }
    }
}

/// Generate a random alphanumeric password.
fn generate_password(len: usize) -> String {
    rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(len)
        .map(char::from)
        .collect()
}

// ---------------------------------------------------------------------------
// Sync loop
// ---------------------------------------------------------------------------

const HEARTH_USERS_GROUP: &str = "hearth-users";

/// Run the identity sync loop until the cancellation token fires.
///
/// Interval is controlled by `HEARTH_IDENTITY_SYNC_INTERVAL_SECS` (default: 300 = 5 min).
pub async fn run(pool: PgPool, auth_config: AuthConfig, cancel: CancellationToken) {
    let interval_secs: u64 = std::env::var("HEARTH_IDENTITY_SYNC_INTERVAL_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(300);

    if !auth_config.is_enabled() {
        info!("identity sync disabled — no OIDC issuer configured");
        return;
    }

    let issuer = match auth_config.oidc_issuers.first() {
        Some(url) => url.clone(),
        None => return,
    };

    let base_url = issuer
        .find("/oauth2/")
        .map(|pos| &issuer[..pos])
        .unwrap_or(&issuer)
        .to_string();

    let token = match std::env::var("HEARTH_API_SVC_TOKEN") {
        Ok(t) if !t.is_empty() => t,
        _ => {
            warn!(
                "HEARTH_API_SVC_TOKEN not set — POSIX provisioning disabled, running read-only sync"
            );
            String::new()
        }
    };

    let http = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .timeout(Duration::from_secs(15))
        .build()
        .unwrap();

    let client = KanidmClient {
        http,
        base_url: base_url.clone(),
        token,
    };

    info!(
        interval_secs,
        base_url = %base_url,
        posix_provisioning = !client.token.is_empty(),
        "starting identity sync loop"
    );

    loop {
        tokio::select! {
            () = tokio::time::sleep(Duration::from_secs(interval_secs)) => {}
            () = cancel.cancelled() => {
                info!("identity sync shutting down");
                return;
            }
        }

        debug!("running identity sync cycle");

        if let Err(e) = run_sync_cycle(&pool, &client).await {
            warn!(error = %e, "identity sync cycle failed");
        }
    }
}

async fn run_sync_cycle(
    pool: &PgPool,
    client: &KanidmClient,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if client.token.is_empty() {
        // No token — just log existing DB users and return.
        let users = repo::list_users(pool).await?;
        debug!(user_count = users.len(), "sync cycle (read-only, no token)");
        return Ok(());
    }

    // Step 1: Ensure hearth-users group has posixgroup class.
    match client.ensure_posixgroup(HEARTH_USERS_GROUP).await {
        Ok(true) => info!("added posixgroup class to {HEARTH_USERS_GROUP}"),
        Ok(false) => {}
        Err(e) => warn!(error = %e, "failed to ensure posixgroup on {HEARTH_USERS_GROUP}"),
    }

    // Step 2: Fetch group members.
    let group = match client.get_group(HEARTH_USERS_GROUP).await? {
        Some(g) => g,
        None => {
            warn!("group {HEARTH_USERS_GROUP} not found in Kanidm — skipping cycle");
            return Ok(());
        }
    };

    let members = group.list("member");
    if members.is_empty() {
        debug!("no members in {HEARTH_USERS_GROUP}");
        return Ok(());
    }

    let mut provisioned = 0u32;
    let mut upserted = 0u32;
    let mut errors = 0u32;

    // Step 3: Process each member.
    for member_ref in members {
        match process_member(pool, client, member_ref).await {
            Ok(MemberResult::Provisioned) => {
                provisioned += 1;
                upserted += 1;
            }
            Ok(MemberResult::Synced) => {
                upserted += 1;
            }
            Ok(MemberResult::Skipped) => {}
            Err(e) => {
                warn!(member = %member_ref, error = %e, "failed to process member");
                errors += 1;
            }
        }
    }

    info!(
        members = members.len(),
        provisioned, upserted, errors, "identity sync cycle complete"
    );

    Ok(())
}

enum MemberResult {
    /// POSIX attributes were newly added.
    Provisioned,
    /// Already had POSIX, just synced to DB.
    Synced,
    /// Skipped (service account, deleted, etc.).
    Skipped,
}

async fn process_member(
    pool: &PgPool,
    client: &KanidmClient,
    member_ref: &str,
) -> Result<MemberResult, Box<dyn std::error::Error + Send + Sync>> {
    let entry = match client.get_person(member_ref).await? {
        Some(e) => e,
        None => {
            debug!(member = %member_ref, "member not found or not a person, skipping");
            return Ok(MemberResult::Skipped);
        }
    };

    // Skip service accounts.
    if entry.has_class("service_account") {
        debug!(member = %member_ref, "skipping service account");
        return Ok(MemberResult::Skipped);
    }

    let username = match entry.first("name") {
        Some(n) => n.to_string(),
        None => {
            warn!(member = %member_ref, "person has no name attribute");
            return Ok(MemberResult::Skipped);
        }
    };

    let needs_posix = !entry.has_class("posixaccount");

    if needs_posix {
        info!(username = %username, "provisioning POSIX attributes");

        client.add_posixaccount(&username).await?;

        // Generate and set a random unix password. The user authenticates via
        // Kanidm's primary credential (web password); kanidm-unixd forwards it
        // to the server. The unix password enables the kanidm-unixd credential
        // cache for offline scenarios.
        let password = generate_password(32);
        client.set_unix_password(&username, &password).await?;

        info!(username = %username, "POSIX provisioning complete");
    }

    // Upsert into local DB.
    let display_name = entry.first("displayname").map(|s| s.to_string());
    let email = entry.first("mail").map(|s| s.to_string());
    let kanidm_uuid = entry.first("uuid").map(|s| s.to_string());
    let groups = auth::normalize_groups(entry.list("memberof"));

    repo::upsert_user(
        pool,
        &username,
        display_name.as_deref(),
        email.as_deref(),
        kanidm_uuid.as_deref(),
        &groups,
    )
    .await?;

    if needs_posix {
        Ok(MemberResult::Provisioned)
    } else {
        Ok(MemberResult::Synced)
    }
}
