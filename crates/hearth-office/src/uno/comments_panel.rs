use crate::nextcloud::{self, OfficeError};
use crate::nextcloud::comments::{self, FileComment};
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

/// Cached file ID mappings (document URL -> Nextcloud file ID).
/// Capped at MAX_CACHE_ENTRIES to prevent unbounded growth.
static FILE_ID_CACHE: LazyLock<Mutex<HashMap<String, String>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

const MAX_CACHE_ENTRIES: usize = 100;

/// Fetch comments for the current document. Returns None if not on Nextcloud.
pub fn fetch_comments(document_url: &str) -> Result<Option<Vec<FileComment>>, OfficeError> {
    let (client, nc_path) = match nextcloud::resolve_nc_context(document_url)? {
        Some(ctx) => ctx,
        None => return Ok(None),
    };

    let file_id = resolve_file_id_cached(document_url, &nc_path, &client)?;
    let file_comments = comments::get_comments(&client, &file_id)
        .map_err(OfficeError::Comments)?;
    Ok(Some(file_comments))
}

/// Post a comment on the current document.
pub fn post_comment(document_url: &str, message: &str) -> Result<(), OfficeError> {
    let (client, nc_path) = match nextcloud::resolve_nc_context(document_url)? {
        Some(ctx) => ctx,
        None => return Err(OfficeError::NotOnNextcloud),
    };

    let file_id = resolve_file_id_cached(document_url, &nc_path, &client)?;
    comments::post_comment(&client, &file_id, message)
        .map_err(OfficeError::Comments)
}

fn resolve_file_id_cached(
    document_url: &str,
    nc_path: &str,
    client: &nextcloud::NextcloudClient,
) -> Result<String, OfficeError> {
    {
        let cache = FILE_ID_CACHE.lock().unwrap();
        if let Some(id) = cache.get(document_url) {
            return Ok(id.clone());
        }
    }

    let file_id = comments::resolve_file_id(client, nc_path)
        .map_err(OfficeError::Comments)?;

    {
        let mut cache = FILE_ID_CACHE.lock().unwrap();
        // Evict oldest entries if cache is full
        if cache.len() >= MAX_CACHE_ENTRIES {
            cache.clear();
        }
        cache.insert(document_url.to_string(), file_id.clone());
    }

    Ok(file_id)
}

pub fn clear_cache() {
    FILE_ID_CACHE.lock().unwrap().clear();
}
