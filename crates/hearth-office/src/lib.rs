//! Hearth LibreOffice UNO extensions — Nextcloud share, comments, lock status
//!
//! Built as a cdylib (.so) and packaged into an .oxt extension for LibreOffice 26.2+
//! with Rust UNO support (--enable-rust-uno).

pub mod config;
pub mod nextcloud;
pub mod uno;
pub mod util;

use rust_uno::generated::rustmaker::com::sun::star::frame::{XFrame, XModel};
use std::ffi::{c_char, c_void, CStr};
use std::ptr;

/// Get the document URL from a UNO XFrame pointer.
///
/// Traverses frame → controller → model → getURL().
/// Returns None if any step fails (e.g., no document open).
pub fn get_document_url_from_frame(frame_ptr: *mut c_void) -> Option<String> {
    let frame = XFrame::XFrame::from_ptr(frame_ptr)?;
    let controller = frame.getController()?;
    let model = XModel::XModel::from_ptr(controller.as_ptr())?;
    let url = model.getURL();
    let url_str = url.to_string();
    if url_str.is_empty() { None } else { Some(url_str) }
}

// ---- UNO Component Entry Points ----

/// UNO component environment identifier.
///
/// # Safety
/// Called by LibreOffice during extension loading.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn component_getImplementationEnvironment(
    env_type_name: *mut *const c_char,
    _env_fn: *mut *const c_void,
) {
    // "unsafe" environment = native shared library (C ABI)
    static ENV: &[u8] = b"unsafe\0";
    unsafe {
        *env_type_name = ENV.as_ptr() as *const c_char;
    }
}

/// UNO component factory.
///
/// Returns a UNO XInterface pointer for the requested implementation name.
/// LibreOffice calls this once per registered service during startup.
///
/// # Safety
/// Called by LibreOffice with C strings and a UNO service manager pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn component_getFactory(
    impl_name: *const c_char,
    _service_manager: *mut c_void,
    _registry_key: *mut c_void,
) -> *mut c_void {
    let name = unsafe { CStr::from_ptr(impl_name) };
    let name_str = name.to_str().unwrap_or("");

    tracing::debug!("hearth-office: component_getFactory({name_str})");

    // TODO: Return actual XSingleComponentFactory via rust_uno generated wrappers.
    // The LO example extension uses a C++ bridge for this (example.cxx).
    // Once rust_uno exposes component registration macros, these can be pure Rust.
    // For now, the C++ bridge in the .oxt handles registration and delegates
    // dispatch calls to our Rust business logic.
    match name_str {
        "com.hearth.ShareHandler" => {
            tracing::info!("hearth-office: ShareHandler factory requested");
            ptr::null_mut()
        }
        "com.hearth.LockStatusController" => {
            tracing::info!("hearth-office: LockStatusController factory requested");
            ptr::null_mut()
        }
        "com.hearth.CommentsPanel" => {
            tracing::info!("hearth-office: CommentsPanel factory requested");
            ptr::null_mut()
        }
        _ => ptr::null_mut(),
    }
}

// ---- Exported Rust functions callable from the C++ bridge ----

/// Execute the "Share via Nextcloud" action.
/// Called by the C++ dispatch handler when the toolbar button is clicked.
///
/// # Safety
/// `frame_ptr` must be a valid UNO XFrame pointer or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hearth_share_via_nextcloud(frame_ptr: *mut c_void) -> i32 {
    let doc_url = match get_document_url_from_frame(frame_ptr) {
        Some(url) => url,
        None => {
            tracing::warn!("hearth-office: no document URL available");
            return -1;
        }
    };

    match uno::share_handler::execute_share(&doc_url) {
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

/// Check the lock status of the current document.
/// Called by the C++ status bar controller on a 30-second timer.
///
/// Returns: 0 = unlocked, 1 = locked, -1 = error/not on NC
/// Writes the lock owner to `owner_buf` (max `owner_buf_len` bytes).
///
/// # Safety
/// `frame_ptr` must be a valid UNO XFrame pointer or null.
/// `owner_buf` must point to a buffer of at least `owner_buf_len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hearth_check_lock_status(
    frame_ptr: *mut c_void,
    owner_buf: *mut u8,
    owner_buf_len: usize,
) -> i32 {
    let doc_url = match get_document_url_from_frame(frame_ptr) {
        Some(url) => url,
        None => return -1,
    };

    match uno::lock_status::check_document_lock(&doc_url) {
        Ok(Some(info)) => {
            if let Some(ref owner) = info.owner {
                let bytes = owner.as_bytes();
                let copy_len = bytes.len().min(owner_buf_len.saturating_sub(1));
                if !owner_buf.is_null() && copy_len > 0 {
                    unsafe {
                        ptr::copy_nonoverlapping(bytes.as_ptr(), owner_buf, copy_len);
                        *owner_buf.add(copy_len) = 0; // null terminator
                    }
                }
            }
            if info.is_locked() { 1 } else { 0 }
        }
        Ok(None) => -1, // not on Nextcloud
        Err(_) => -1,
    }
}
