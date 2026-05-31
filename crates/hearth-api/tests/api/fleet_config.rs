use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::routing::get;
use hearth_api::fleet_config::{
    FlakeLatestResponse, flake_latest, flake_tarball, flake_tarball_by_hash, new_tarball_cache,
};
use http_body_util::BodyExt;
use tower::ServiceExt;

// ---------------------------------------------------------------------------
// fleet-config endpoints
//
// These don't need a DB — the cache is in-memory. Build a minimal router so
// the tests don't require DATABASE_URL or any auth scaffolding. The
// endpoints are intentionally unauthenticated (see route doc comments
// explaining the rationale: build worker + Nix tarball fetcher both lack a
// clean way to pass an auth header, and the tarball content is open-source
// code); these tests pin that contract so a future "add auth here" doesn't
// break the build worker without an explicit decision.
// ---------------------------------------------------------------------------

fn fleet_config_router() -> Router {
    let cache = new_tarball_cache();
    Router::new()
        .route("/api/v1/fleet-config/flake.tar.gz", get(flake_tarball))
        .route("/api/v1/fleet-config/latest", get(flake_latest))
        .route(
            "/api/v1/fleet-config/{hash}/flake.tar.gz",
            get(flake_tarball_by_hash),
        )
        .with_state(cache)
}

async fn body_bytes(resp: axum::response::Response) -> Vec<u8> {
    resp.into_body()
        .collect()
        .await
        .expect("read body")
        .to_bytes()
        .to_vec()
}

#[tokio::test]
async fn flake_latest_returns_hash_and_content_addressed_url() {
    let app = fleet_config_router();

    let req = Request::builder()
        .method("GET")
        .uri("/api/v1/fleet-config/latest")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = body_bytes(resp).await;
    let body: FlakeLatestResponse =
        serde_json::from_slice(&bytes).expect("response is FlakeLatestResponse JSON");

    // Hash is a 64-char hex sha256.
    assert_eq!(
        body.hash.len(),
        64,
        "expected sha256 hex hash, got {body:?}"
    );
    assert!(
        body.hash.chars().all(|c| c.is_ascii_hexdigit()),
        "hash must be hex: {body:?}"
    );

    // Tarball URL is content-addressed by the same hash and uses the
    // tarball+ scheme that Nix's fetcher understands.
    assert!(
        body.tarball_url.starts_with("tarball+"),
        "tarball_url must use the tarball+ scheme: {body:?}"
    );
    assert!(
        body.tarball_url.contains(&body.hash),
        "tarball_url must embed the response hash for cache busting: {body:?}"
    );
    assert!(
        body.tarball_url.ends_with("/flake.tar.gz"),
        "tarball_url must point at the gzipped tarball: {body:?}"
    );
}

#[tokio::test]
async fn flake_tarball_by_hash_serves_gzip_with_cache_headers() {
    let app = fleet_config_router();

    // First fetch /latest to discover the current hash.
    let req = Request::builder()
        .uri("/api/v1/fleet-config/latest")
        .body(Body::empty())
        .unwrap();
    let latest: FlakeLatestResponse =
        serde_json::from_slice(&body_bytes(app.clone().oneshot(req).await.unwrap()).await).unwrap();

    // Pull the tarball at the content-addressed URL.
    let uri = format!("/api/v1/fleet-config/{}/flake.tar.gz", latest.hash);
    let req = Request::builder().uri(&uri).body(Body::empty()).unwrap();
    let resp = app.oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let headers = resp.headers().clone();
    assert_eq!(
        headers.get("content-type").map(|v| v.to_str().unwrap()),
        Some("application/gzip"),
    );
    assert_eq!(
        headers
            .get("content-disposition")
            .map(|v| v.to_str().unwrap()),
        Some("attachment; filename=\"flake.tar.gz\""),
    );
    // Content-addressed URL → may be cached forever.
    assert_eq!(
        headers.get("cache-control").map(|v| v.to_str().unwrap()),
        Some("public, max-age=31536000, immutable"),
    );
    // ETag is the first 16 hex chars of the sha256, quoted.
    let etag = headers.get("etag").expect("etag header present");
    assert_eq!(
        etag.to_str().unwrap(),
        format!("\"{}\"", &latest.hash[..16])
    );

    // Body is non-empty gzip. (Magic bytes 0x1f 0x8b.)
    let body = body_bytes(resp).await;
    assert!(!body.is_empty(), "tarball body must not be empty");
    assert!(
        body.starts_with(&[0x1f, 0x8b]),
        "tarball body must start with gzip magic, got: {:?}",
        &body[..body.len().min(4)],
    );
}

#[tokio::test]
async fn flake_tarball_by_hash_ignores_url_hash_for_content() {
    // Documented quirk (see routes/fleet_config.rs:flake_tarball_by_hash):
    // the hash in the URL is purely for Nix's URL-keyed cache busting, not
    // for content validation. A request for a stale hash returns the
    // *current* tarball. Pin this so a future "validate the hash matches"
    // change is an explicit decision, not an accidental break.
    let app = fleet_config_router();
    let uri = "/api/v1/fleet-config/deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef/flake.tar.gz";
    let req = Request::builder().uri(uri).body(Body::empty()).unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "stale hash should still serve current tarball"
    );
}

#[tokio::test]
async fn flake_tarball_backwards_compat_path() {
    let app = fleet_config_router();
    let req = Request::builder()
        .uri("/api/v1/fleet-config/flake.tar.gz")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    // Static path doesn't get content-addressed cache headers — Nix may
    // refetch on every build, but the path itself remains for legacy
    // callers (kept by `routes::fleet_config::flake_tarball`).
    assert_eq!(
        resp.headers()
            .get("cache-control")
            .map(|v| v.to_str().unwrap()),
        Some("no-cache"),
    );
}

#[tokio::test]
async fn flake_latest_endpoint_is_intentionally_unauthenticated() {
    // Pin the auth contract: no Authorization header, no machine token —
    // still 200. The route doc comments explain why (build worker has no
    // machine token, Nix's tarball fetcher can't pass headers). Change
    // these tests at the same time as flipping the auth model so it's an
    // explicit decision.
    let app = fleet_config_router();
    for uri in [
        "/api/v1/fleet-config/latest",
        "/api/v1/fleet-config/flake.tar.gz",
    ] {
        let req = Request::builder().uri(uri).body(Body::empty()).unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::OK,
            "{uri} must be reachable without any auth"
        );
    }
}
