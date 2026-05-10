// rust_ffi.hxx — declarations for the Rust ABI exposed by libhearth_office.so
//
// These functions are implemented in crates/hearth-office/src/lib.rs as
// `#[unsafe(no_mangle)] extern "C"`. The bridge .so links against the Rust
// .so via -lhearth_office; the dynamic linker resolves the symbols at load
// time using $ORIGIN-relative RPATH (both .so files live in the same .oxt
// directory after unopkg unpacks).
//
// All entrypoints take a UTF-8 document URL string. The C++ shim is
// responsible for traversing XFrame → XController → XModel → getURL() and
// passing the resulting URL down — the Rust side stays free of UNO interop.

#pragma once

#include <cstddef>
#include <cstdint>

extern "C" {

// Execute "Share via Nextcloud" for the document at `document_url`.
// Returns 0 on success, 1 if not under a Nextcloud mount, -1 on error.
int32_t hearth_share_via_nextcloud(const char* document_url);

// Poll the WebDAV lock status for the document at `document_url`.
// On lock, fills `owner_buf` with a null-terminated UTF-8 owner name
// (truncated to owner_buf_len-1 bytes).
// Returns 0 unlocked, 1 locked, -1 error/not on Nextcloud.
int32_t hearth_check_lock_status(const char* document_url,
                                 uint8_t* owner_buf,
                                 size_t owner_buf_len);

// Fetch comments for the document as a JSON array
// (`[{"id","author_display_name","message","creation_date_time"}, ...]`).
// Writes a null-terminated JSON string to `json_buf`.
// Returns bytes written (excluding null terminator), or:
//   -1 = error / not on Nextcloud
//   -2 = json_buf too small (caller should retry with a larger buffer)
int32_t hearth_fetch_comments_json(const char* document_url,
                                   uint8_t* json_buf,
                                   size_t json_buf_len);

}  // extern "C"
