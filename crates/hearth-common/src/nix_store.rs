//! Utilities for working with Nix store paths.

use std::path::{Path, PathBuf};

/// The standard Nix store prefix.
pub const NIX_STORE_DIR: &str = "/nix/store";

/// Validates that a string looks like a valid Nix store path.
///
/// A store path has the form `/nix/store/<hash>-<name>` where hash is 32 chars
/// of base32 (nix-specific encoding).
pub fn is_valid_store_path(path: &str) -> bool {
    let Some(rest) = path.strip_prefix("/nix/store/") else {
        return false;
    };
    // Hash is 32 chars followed by '-' and at least one char for the name
    if rest.len() < 34 {
        return false;
    }
    let hash = &rest[..32];
    let separator = rest.as_bytes()[32];
    hash.chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        && separator == b'-'
}

/// Extracts the package name from a Nix store path.
///
/// Given `/nix/store/abc123...-firefox-128.0`, returns `Some("firefox-128.0")`.
pub fn store_path_name(path: &str) -> Option<&str> {
    let rest = path.strip_prefix("/nix/store/")?;
    if rest.len() < 34 {
        return None;
    }
    Some(&rest[33..])
}

/// Returns the store path for a given hash and name.
pub fn make_store_path(hash: &str, name: &str) -> PathBuf {
    Path::new(NIX_STORE_DIR).join(format!("{hash}-{name}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_store_path() {
        assert!(is_valid_store_path(
            "/nix/store/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-hello"
        ));
    }

    #[test]
    fn invalid_store_paths() {
        assert!(!is_valid_store_path("/tmp/foo"));
        assert!(!is_valid_store_path("/nix/store/short-hello"));
        assert!(!is_valid_store_path("/nix/store/"));
    }

    #[test]
    fn extract_name() {
        assert_eq!(
            store_path_name("/nix/store/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-firefox-128.0"),
            Some("firefox-128.0")
        );
    }
}
