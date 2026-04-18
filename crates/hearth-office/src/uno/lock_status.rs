use crate::nextcloud::{self, OfficeError};
use crate::nextcloud::lock;

/// Human-readable lock status for UI display.
#[derive(Debug, Clone)]
pub struct LockDisplayInfo {
    pub text: String,
    pub owner: Option<String>,
    pub timeout: Option<String>,
}

impl LockDisplayInfo {
    pub fn is_locked(&self) -> bool {
        self.owner.is_some()
    }
}

pub const LOCK_POLL_INTERVAL_SECS: u64 = 30;

/// Check the lock status of a document. Returns None if not on Nextcloud.
pub fn check_document_lock(document_url: &str) -> Result<Option<LockDisplayInfo>, OfficeError> {
    let (client, nc_path) = match nextcloud::resolve_nc_context(document_url)? {
        Some(ctx) => ctx,
        None => return Ok(None),
    };

    let status = lock::check_lock_status(&client, &nc_path)
        .map_err(OfficeError::Lock)?;

    Ok(Some(match status {
        lock::LockStatus::Unlocked => LockDisplayInfo {
            text: "Not locked".into(),
            owner: None,
            timeout: None,
        },
        lock::LockStatus::Locked { owner, timeout } => {
            let display_name = owner.split('@').next().unwrap_or(&owner).to_string();
            LockDisplayInfo {
                text: format!("Locked by {display_name}"),
                owner: Some(owner),
                timeout,
            }
        }
    }))
}
