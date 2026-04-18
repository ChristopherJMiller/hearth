use axum::body::Body;
use axum::extract::{Path as AxumPath, State};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use axum::Json;
use flate2::Compression;
use flate2::write::GzEncoder;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;

use crate::error::AppError;

/// Directories/files to include in the fleet config tarball.
const INCLUDE_PATHS: &[&str] = &[
    "flake.nix",
    "flake.lock",
    "lib",
    "modules",
    "home-modules",
    "overlays",
    "nix",
    "Cargo.toml",
    "Cargo.lock",
    "crates",
    ".cargo",
    "rust-toolchain.toml",
];

/// How long to cache the tarball before regenerating (seconds).
const CACHE_TTL_SECS: u64 = 30;

/// Cached tarball with its content hash.
pub struct CachedFlakeTarball {
    pub hash: String,
    pub bytes: Vec<u8>,
    generated_at: Instant,
}

/// Shared cache for the flake tarball, stored in AppState.
pub type FlakeTarballCache = Arc<RwLock<Option<CachedFlakeTarball>>>;

pub fn new_tarball_cache() -> FlakeTarballCache {
    Arc::new(RwLock::new(None))
}

/// Ensure the cache is fresh, regenerating if stale or empty.
async fn ensure_cached(cache: &FlakeTarballCache) -> Result<(), AppError> {
    {
        let guard = cache.read().await;
        if let Some(ref cached) = *guard {
            if cached.generated_at.elapsed().as_secs() < CACHE_TTL_SECS {
                return Ok(());
            }
        }
    }

    // Cache is stale or empty — rebuild
    let flake_dir = flake_source_dir();
    let bytes = tokio::task::spawn_blocking(move || build_tarball(&flake_dir))
        .await
        .map_err(|e| AppError::Internal(format!("tarball task failed: {e}")))?
        .map_err(|e| AppError::Internal(format!("failed to build flake tarball: {e}")))?;

    let hash = format!("{:x}", Sha256::digest(&bytes));

    let mut guard = cache.write().await;
    *guard = Some(CachedFlakeTarball {
        hash,
        bytes,
        generated_at: Instant::now(),
    });

    Ok(())
}

/// Response for the `/latest` endpoint.
#[derive(Serialize)]
pub struct FlakeLatestResponse {
    pub hash: String,
    pub tarball_url: String,
}

/// `GET /api/v1/fleet-config/latest`
///
/// Returns the current flake tarball content hash and the content-addressed URL.
/// The build worker calls this before each build to get a cache-busting URL.
pub async fn flake_latest(
    State(cache): State<FlakeTarballCache>,
) -> Result<Json<FlakeLatestResponse>, AppError> {
    ensure_cached(&cache).await?;

    let guard = cache.read().await;
    let cached = guard.as_ref().unwrap();

    let server_url = std::env::var("HEARTH_SERVER_URL")
        .unwrap_or_else(|_| "http://localhost:3000".to_string());

    Ok(Json(FlakeLatestResponse {
        hash: cached.hash.clone(),
        tarball_url: format!(
            "tarball+{server_url}/api/v1/fleet-config/{}/flake.tar.gz",
            cached.hash
        ),
    }))
}

/// `GET /api/v1/fleet-config/:hash/flake.tar.gz`
///
/// Serves the flake tarball at a content-addressed URL. Nix caches by URL,
/// so a new hash = new URL = forced re-fetch.
pub async fn flake_tarball_by_hash(
    State(cache): State<FlakeTarballCache>,
    AxumPath(_hash): AxumPath<String>,
) -> Result<Response, AppError> {
    ensure_cached(&cache).await?;

    let guard = cache.read().await;
    let cached = guard.as_ref().unwrap();

    // Serve the tarball regardless of hash match — the hash in the URL is
    // for cache busting, not for content validation. If the hash doesn't
    // match the current content, the caller gets the latest anyway (which
    // is the correct behavior for Nix's tarball fetcher).
    let etag = format!("\"{}\"", &cached.hash[..16]);

    Ok((
        [
            (header::CONTENT_TYPE, "application/gzip"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"flake.tar.gz\"",
            ),
            (header::ETAG, etag.as_str()),
            (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
        ],
        Body::from(cached.bytes.clone()),
    )
        .into_response())
}

/// `GET /api/v1/fleet-config/flake.tar.gz` (backwards compat)
///
/// Serves the tarball at the static URL. Still works but Nix may cache it.
pub async fn flake_tarball(
    State(cache): State<FlakeTarballCache>,
) -> Result<Response, AppError> {
    ensure_cached(&cache).await?;

    let guard = cache.read().await;
    let cached = guard.as_ref().unwrap();
    let etag = format!("\"{}\"", &cached.hash[..16]);

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
        Body::from(cached.bytes.clone()),
    )
        .into_response())
}

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
