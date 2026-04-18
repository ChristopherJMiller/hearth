// hearth-office util — path detection and Nextcloud file path resolution
//
// Determines whether a document URL refers to a Nextcloud-managed file and
// extracts the relative Nextcloud path for API calls.

use std::path::{Path, PathBuf};

/// Describes where a document lives relative to Nextcloud.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileLocation {
    /// File is in the local Nextcloud sync folder (e.g., ~/Nextcloud/Documents/report.odt).
    /// Contains the relative path within Nextcloud (e.g., "/Documents/report.odt").
    Synced { nc_path: String },

    /// File is accessed via a WebDAV URL (davs:// or https://.../remote.php/dav/files/...).
    /// Contains the relative path within the user's DAV space.
    WebDav { nc_path: String },

    /// File is not associated with Nextcloud.
    External,
}

/// Resolve a LibreOffice document URL to its Nextcloud file location.
///
/// Handles three cases:
/// 1. Local file under the Nextcloud sync directory (~/Nextcloud/ by default)
/// 2. WebDAV URL (davs:// or https://.../remote.php/dav/files/USERNAME/...)
/// 3. Anything else (external — not on Nextcloud)
pub fn resolve_file_location(
    document_url: &str,
    sync_dir: &Path,
    webdav_base_url: &str,
) -> FileLocation {
    // Case 1: file:// URL pointing into the sync directory
    if let Some(local_path) = strip_file_url(document_url) {
        if let Ok(relative) = local_path.strip_prefix(sync_dir) {
            let nc_path = format!("/{}", relative.display());
            return FileLocation::Synced { nc_path };
        }
    }

    // Case 2: WebDAV URL (strip the base to get the relative path)
    let url_lower = document_url.to_lowercase();
    if url_lower.starts_with("davs://") || url_lower.starts_with("dav://") {
        // davs://server/remote.php/dav/files/USERNAME/path/to/file
        if let Some(nc_path) = extract_dav_path(document_url) {
            return FileLocation::WebDav { nc_path };
        }
    }

    // Also check for https:// WebDAV URLs matching the configured base
    if document_url.starts_with(webdav_base_url) {
        let remainder = &document_url[webdav_base_url.len()..];
        // remainder is "USERNAME/path/to/file" — strip the username prefix
        if let Some((_user, path)) = remainder.split_once('/') {
            let nc_path = format!("/{path}");
            return FileLocation::WebDav { nc_path };
        }
    }

    FileLocation::External
}

/// Convert a file:// URL to a local filesystem Path.
fn strip_file_url(url: &str) -> Option<PathBuf> {
    url.strip_prefix("file://").map(|p| {
        // URL-decode %20 etc.
        let decoded = urlish_decode(p);
        PathBuf::from(decoded)
    })
}

/// Extract the user-relative path from a davs:// or dav:// URL.
///
/// Input:  davs://cloud.example.com/remote.php/dav/files/alice/Documents/report.odt
/// Output: Some("/Documents/report.odt")
fn extract_dav_path(url: &str) -> Option<String> {
    // Find "/remote.php/dav/files/" in the URL
    let marker = "/remote.php/dav/files/";
    let idx = url.find(marker)?;
    let after = &url[idx + marker.len()..];
    // Skip the username segment
    let (_user, path) = after.split_once('/')?;
    Some(format!("/{path}"))
}

/// Minimal percent-decoding for file URLs (handles %20, %23, etc.).
fn urlish_decode(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '%' {
            let hex: String = chars.by_ref().take(2).collect();
            if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                result.push(byte as char);
            } else {
                result.push('%');
                result.push_str(&hex);
            }
        } else {
            result.push(c);
        }
    }
    result
}

/// Get the default Nextcloud sync directory for the current user.
/// Returns None if the home directory cannot be determined.
pub fn default_sync_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join("Nextcloud"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synced_file_detected() {
        let sync_dir = Path::new("/home/alice/Nextcloud");
        let url = "file:///home/alice/Nextcloud/Documents/report.odt";
        let loc = resolve_file_location(url, sync_dir, "https://cloud.example.com/remote.php/dav/files/");
        assert_eq!(loc, FileLocation::Synced { nc_path: "/Documents/report.odt".into() });
    }

    #[test]
    fn webdav_url_detected() {
        let sync_dir = Path::new("/home/alice/Nextcloud");
        let url = "davs://cloud.example.com/remote.php/dav/files/alice/Documents/report.odt";
        let loc = resolve_file_location(url, sync_dir, "https://cloud.example.com/remote.php/dav/files/");
        assert_eq!(loc, FileLocation::WebDav { nc_path: "/Documents/report.odt".into() });
    }

    #[test]
    fn https_webdav_url_detected() {
        let sync_dir = Path::new("/home/alice/Nextcloud");
        let url = "https://cloud.example.com/remote.php/dav/files/alice/Documents/report.odt";
        let webdav_base = "https://cloud.example.com/remote.php/dav/files/";
        let loc = resolve_file_location(url, sync_dir, webdav_base);
        assert_eq!(loc, FileLocation::WebDav { nc_path: "/Documents/report.odt".into() });
    }

    #[test]
    fn external_file_detected() {
        let sync_dir = Path::new("/home/alice/Nextcloud");
        let url = "file:///tmp/scratch.odt";
        let loc = resolve_file_location(url, sync_dir, "https://cloud.example.com/remote.php/dav/files/");
        assert_eq!(loc, FileLocation::External);
    }

    #[test]
    fn url_decoding_works() {
        let sync_dir = Path::new("/home/alice/Nextcloud");
        let url = "file:///home/alice/Nextcloud/My%20Documents/report.odt";
        let loc = resolve_file_location(url, sync_dir, "https://cloud.example.com/remote.php/dav/files/");
        assert_eq!(loc, FileLocation::Synced { nc_path: "/My Documents/report.odt".into() });
    }
}
