use crate::nextcloud::{self, OfficeError};
use crate::nextcloud::share;

#[derive(Debug)]
pub enum ShareResult {
    Success { url: String },
    NotOnNextcloud,
    Error(OfficeError),
}

/// Execute the "Share via Nextcloud" action for a document.
pub fn execute_share(document_url: &str) -> ShareResult {
    let (client, nc_path) = match nextcloud::resolve_nc_context(document_url) {
        Ok(Some(ctx)) => ctx,
        Ok(None) => return ShareResult::NotOnNextcloud,
        Err(e) => return ShareResult::Error(e),
    };

    match share::create_share_link(&client, &nc_path) {
        Ok(link) => {
            if let Err(e) = share::copy_to_clipboard(&link.url) {
                tracing::warn!("Failed to copy to clipboard: {e}");
            }
            tracing::info!("Share link created: {}", link.url);
            ShareResult::Success { url: link.url }
        }
        Err(e) => ShareResult::Error(OfficeError::Share(e)),
    }
}
