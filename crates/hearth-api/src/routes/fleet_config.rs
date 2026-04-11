use axum::body::Body;
use axum::http::header;
use axum::response::{IntoResponse, Response};
use flate2::write::GzEncoder;
use flate2::Compression;
use std::path::{Path, PathBuf};

use crate::error::AppError;

/// Directories/files to include in the fleet config tarball (relative to the
/// flake source directory).  These are the inputs required by
/// `lib.buildMachineConfig` in the Hearth flake.
const INCLUDE_PATHS: &[&str] = &[
    "flake.nix",
    "flake.lock",
    "lib",
    "modules",
    "home-modules",
    "overlays",
    // Cargo workspace files needed by Crane to evaluate package derivations.
    // The actual build is substituted from the binary cache — Crane just needs
    // these to compute the derivation hash.
    "Cargo.toml",
    "Cargo.lock",
    "crates",
    ".cargo",
    "rust-toolchain.toml",
];

/// `GET /api/v1/fleet-config/flake.tar.gz`
///
/// Serves a gzipped tarball of the Hearth flake — the subset needed by the
/// build worker to evaluate `buildMachineConfig`.  The worker references this
/// endpoint via `tarball+http://hearth-api:3000/api/v1/fleet-config/flake.tar.gz`.
pub async fn flake_tarball() -> Result<Response, AppError> {
    let flake_dir = flake_source_dir();

    let tarball = tokio::task::spawn_blocking(move || build_tarball(&flake_dir))
        .await
        .map_err(|e| AppError::Internal(format!("tarball task failed: {e}")))?
        .map_err(|e| AppError::Internal(format!("failed to build flake tarball: {e}")))?;

    // Compute a short content hash so Nix's tarball fetcher can detect changes.
    use sha2::{Digest, Sha256};
    let hash = format!("{:x}", Sha256::digest(&tarball));
    let etag = format!("\"{}\"", &hash[..16]);

    Ok((
        [
            (header::CONTENT_TYPE, "application/gzip"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"flake.tar.gz\"",
            ),
            (header::ETAG, etag.as_str()),
            (header::CACHE_CONTROL, "no-cache"),
        ],
        Body::from(tarball),
    )
        .into_response())
}

/// Determine the flake source directory.
///
/// In production this is set explicitly via `HEARTH_FLAKE_DIR` (e.g. a mounted
/// volume in k8s).  For local dev it defaults to the current working directory
/// (the repo root when launched via `just dev`).
fn flake_source_dir() -> PathBuf {
    std::env::var("HEARTH_FLAKE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

fn build_tarball(base: &Path) -> Result<Vec<u8>, std::io::Error> {
    let buf = Vec::new();
    let encoder = GzEncoder::new(buf, Compression::fast());
    let mut archive = tar::Builder::new(encoder);

    for entry in INCLUDE_PATHS {
        let full = base.join(entry);
        if !full.exists() {
            tracing::warn!(path = %full.display(), "flake tarball: path not found, skipping");
            continue;
        }
        if full.is_dir() {
            archive.append_dir_all(entry, &full)?;
        } else {
            archive.append_path_with_name(&full, entry)?;
        }
    }

    archive.finish()?;
    let encoder = archive.into_inner()?;
    encoder.finish()
}
