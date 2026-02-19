//! NSS utilities for resolving a user's group memberships via libc.
//!
//! Used to determine the user's role after authentication so the agent can
//! prepare the correct environment.

use std::ffi::{CStr, CString};
use std::io;
use thiserror::Error;
use tracing::debug;

#[derive(Debug, Error)]
pub enum NssError {
    #[error("username contains interior NUL byte")]
    InvalidUsername,
    #[error("user not found: {0}")]
    UserNotFound(String),
    #[error("group ID {0} could not be resolved")]
    GroupNotFound(libc::gid_t),
    #[error("OS error during NSS lookup: {0}")]
    Os(#[from] io::Error),
}

/// Look up all group names for `username` using POSIX NSS functions.
///
/// Returns the names of every group the user belongs to (including the primary
/// group).
pub fn get_user_groups(username: &str) -> Result<Vec<String>, NssError> {
    let c_username = CString::new(username).map_err(|_| NssError::InvalidUsername)?;

    // Step 1: getpwnam_r to get the user's primary GID.
    let primary_gid = lookup_user_gid(&c_username, username)?;

    // Step 2: getgrouplist to get all GIDs (primary + supplementary).
    let gids = get_group_list(&c_username, primary_gid)?;

    // Step 3: Resolve each GID to a group name via getgrgid_r.
    let mut names = Vec::with_capacity(gids.len());
    for gid in &gids {
        match resolve_group_name(*gid) {
            Ok(name) => names.push(name),
            Err(NssError::GroupNotFound(gid)) => {
                debug!(gid, "skipping unresolvable GID");
            }
            Err(e) => return Err(e),
        }
    }

    debug!(username, ?names, "resolved user groups");
    Ok(names)
}

/// Use `getpwnam_r` to look up the user's primary GID.
fn lookup_user_gid(c_username: &CStr, username: &str) -> Result<libc::gid_t, NssError> {
    let mut pwd: libc::passwd = unsafe { std::mem::zeroed() };
    let mut result: *mut libc::passwd = std::ptr::null_mut();
    let mut buf = vec![0u8; 4096];

    loop {
        let rc = unsafe {
            libc::getpwnam_r(
                c_username.as_ptr(),
                &mut pwd,
                buf.as_mut_ptr().cast::<libc::c_char>(),
                buf.len(),
                &mut result,
            )
        };

        if rc == libc::ERANGE {
            // Buffer too small, double it and retry.
            buf.resize(buf.len() * 2, 0);
            continue;
        }

        if rc != 0 {
            return Err(NssError::Os(io::Error::from_raw_os_error(rc)));
        }

        if result.is_null() {
            return Err(NssError::UserNotFound(username.to_string()));
        }

        return Ok(pwd.pw_gid);
    }
}

/// Use `getgrouplist` to get all group IDs for the user.
fn get_group_list(
    c_username: &CStr,
    primary_gid: libc::gid_t,
) -> Result<Vec<libc::gid_t>, NssError> {
    // Start with room for 32 groups; expand if needed.
    let mut ngroups: libc::c_int = 32;
    let mut groups: Vec<libc::gid_t> = vec![0; ngroups as usize];

    loop {
        let rc = unsafe {
            libc::getgrouplist(
                c_username.as_ptr(),
                primary_gid,
                groups.as_mut_ptr(),
                &mut ngroups,
            )
        };

        if rc == -1 {
            // ngroups has been updated to the required count.
            if ngroups <= 0 {
                // Shouldn't happen, but be safe.
                return Ok(vec![primary_gid]);
            }
            groups.resize(ngroups as usize, 0);
            continue;
        }

        groups.truncate(ngroups as usize);
        return Ok(groups);
    }
}

/// Use `getgrgid_r` to resolve a GID to a group name.
fn resolve_group_name(gid: libc::gid_t) -> Result<String, NssError> {
    let mut grp: libc::group = unsafe { std::mem::zeroed() };
    let mut result: *mut libc::group = std::ptr::null_mut();
    let mut buf = vec![0u8; 4096];

    loop {
        let rc = unsafe {
            libc::getgrgid_r(
                gid,
                &mut grp,
                buf.as_mut_ptr().cast::<libc::c_char>(),
                buf.len(),
                &mut result,
            )
        };

        if rc == libc::ERANGE {
            buf.resize(buf.len() * 2, 0);
            continue;
        }

        if rc != 0 {
            return Err(NssError::Os(io::Error::from_raw_os_error(rc)));
        }

        if result.is_null() {
            return Err(NssError::GroupNotFound(gid));
        }

        let name = unsafe { CStr::from_ptr(grp.gr_name) };
        return Ok(name.to_string_lossy().into_owned());
    }
}
