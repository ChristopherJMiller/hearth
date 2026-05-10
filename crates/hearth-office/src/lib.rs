//! Hearth LibreOffice UNO extensions — Nextcloud share, comments, lock status
//!
//! Built as a cdylib (.so) and shipped alongside a small C++ UNO shim
//! (cpp/hearth-office-bridge/) inside the .oxt. The C++ shim implements
//! XSingleComponentFactory and forwards method dispatch to the Rust functions
//! exposed below via this `extern "C"` ABI.
//!
//! Why we don't do component registration in pure Rust: upstream rust_uno
//! (LO 26.2) ships interface-pointer wrappers but not component-registration
//! macros yet, and the rust_uno crate available to us is built from
//! hand-written stubs (see nix/rust-uno/default.nix). When upstream rust_uno
//! gains real `component_getFactory` macros, the C++ shim can retire and
//! these entrypoints can move into a pure-Rust `component_getFactory`.
//!
//! All entrypoints take a UTF-8 document URL string rather than a UNO frame
//! pointer — frame→controller→model→getURL traversal happens in the C++
//! shim against real UNO bindings, so this crate stays free of UNO interop.

pub mod config;
pub mod nextcloud;
pub mod uno;
pub mod util;

use std::ffi::CStr;
use std::os::raw::c_char;
use std::ptr;

/// Helper: convert a C string pointer to a Rust &str, returning None for
/// null/non-UTF-8.
///
/// # Safety
/// `s` must be either null or a valid pointer to a null-terminated C string
/// that lives at least until this function returns.
unsafe fn c_str_to_str<'a>(s: *const c_char) -> Option<&'a str> {
    if s.is_null() {
        return None;
    }
    unsafe { CStr::from_ptr(s) }.to_str().ok()
}

// ---- Exported Rust functions callable from the C++ bridge ----

/// Execute the "Share via Nextcloud" action for the document at `document_url`.
///
/// Returns 0 on success, 1 if the file is not under a Nextcloud mount, -1 on
/// any other error (auth, network, malformed input).
///
/// # Safety
/// `document_url` must be null or a valid pointer to a null-terminated UTF-8
/// C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hearth_share_via_nextcloud(document_url: *const c_char) -> i32 {
    let Some(doc_url) = (unsafe { c_str_to_str(document_url) }) else {
        tracing::warn!("hearth-office: hearth_share_via_nextcloud got null/invalid URL");
        return -1;
    };

    match uno::share_handler::execute_share(doc_url) {
        uno::share_handler::ShareResult::Success { url } => {
            tracing::info!("Share link: {url}");
            0
        }
        uno::share_handler::ShareResult::NotOnNextcloud => {
            tracing::info!("File is not on Nextcloud");
            1
        }
        uno::share_handler::ShareResult::Error(e) => {
            tracing::error!("Share failed: {e}");
            -1
        }
    }
}

/// Poll the WebDAV lock status of the document at `document_url`.
///
/// Writes the lock owner (if any) to `owner_buf` as a null-terminated UTF-8
/// string, truncated to `owner_buf_len - 1` bytes.
///
/// Returns 0 if unlocked, 1 if locked, -1 on error or if the file is not on
/// Nextcloud.
///
/// # Safety
/// `document_url` must be null or a valid pointer to a null-terminated UTF-8
/// C string. `owner_buf` must be null or point to a writable buffer of at
/// least `owner_buf_len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hearth_check_lock_status(
    document_url: *const c_char,
    owner_buf: *mut u8,
    owner_buf_len: usize,
) -> i32 {
    let Some(doc_url) = (unsafe { c_str_to_str(document_url) }) else {
        return -1;
    };

    match uno::lock_status::check_document_lock(doc_url) {
        Ok(Some(info)) => {
            if let Some(ref owner) = info.owner {
                let bytes = owner.as_bytes();
                let copy_len = bytes.len().min(owner_buf_len.saturating_sub(1));
                if !owner_buf.is_null() && copy_len > 0 {
                    unsafe {
                        ptr::copy_nonoverlapping(bytes.as_ptr(), owner_buf, copy_len);
                        *owner_buf.add(copy_len) = 0;
                    }
                }
            }
            if info.is_locked() { 1 } else { 0 }
        }
        Ok(None) => -1,
        Err(_) => -1,
    }
}

/// Fetch Nextcloud comments for the document at `document_url` and serialize
/// them as a JSON array into `json_buf`.
///
/// JSON shape: `[{"author": "...", "message": "...", "creation_datetime":
/// "RFC3339"}, ...]`. Empty array if the document has no comments.
///
/// Returns the number of bytes written to `json_buf` (excluding null
/// terminator), or -1 on error / not on Nextcloud, or -2 if the buffer is
/// too small (caller should retry with a larger buffer).
///
/// # Safety
/// `document_url` must be null or a valid pointer to a null-terminated UTF-8
/// C string. `json_buf` must be null or point to a writable buffer of at
/// least `json_buf_len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hearth_fetch_comments_json(
    document_url: *const c_char,
    json_buf: *mut u8,
    json_buf_len: usize,
) -> i32 {
    let Some(doc_url) = (unsafe { c_str_to_str(document_url) }) else {
        return -1;
    };

    let comments = match uno::comments_panel::fetch_comments(doc_url) {
        Ok(Some(c)) => c,
        Ok(None) => return -1,
        Err(e) => {
            tracing::error!("hearth-office: fetch comments failed: {e}");
            return -1;
        }
    };

    let json = match serde_json::to_string(&comments) {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("hearth-office: serialize comments failed: {e}");
            return -1;
        }
    };

    let bytes = json.as_bytes();
    if bytes.len() + 1 > json_buf_len {
        return -2;
    }
    if !json_buf.is_null() {
        unsafe {
            ptr::copy_nonoverlapping(bytes.as_ptr(), json_buf, bytes.len());
            *json_buf.add(bytes.len()) = 0;
        }
    }
    bytes.len() as i32
}
