use crate::common::{create_machine_http_authed, send_json, send_status, test_app_with_auth};
use hearth_common::api_types::{DesktopPreferences, UserConfig};
use serde_json::json;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// PUT /api/v1/machines/{machine_id}/users/{username}/desktop-prefs
//
// Auth contract (crates/hearth-api/src/routes/user_configs.rs sync_desktop_prefs):
//   - MachineIdentity extractor → 401 without a machine JWT.
//   - machine_id in path must equal machine_id in token → 403 on mismatch.
//   - On success: 204 NO_CONTENT, prefs persisted under
//     user_configs.overrides.desktop, picked up by the next build via
//     config_hash comparison.
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn sync_desktop_prefs_persists_under_overrides_desktop() {
    let ctx = test_app_with_auth().await;
    let machine = create_machine_http_authed(&ctx, "desktop-prefs-host").await;
    let machine_token = ctx.mint_machine_jwt(machine.id);

    let uri = format!("/api/v1/machines/{}/users/alice/desktop-prefs", machine.id);
    let body = json!({
        "desktop": {
            "favorite_apps": ["firefox.desktop", "org.gnome.Nautilus.desktop"],
            "wallpaper_uri": "file:///usr/share/backgrounds/blobs-l.svg",
            "dark_mode": true,
        }
    });

    let status = send_status(&ctx.router, "PUT", &uri, Some(body), Some(&machine_token)).await;
    assert_eq!(status, 204, "expected 204 NO_CONTENT on valid sync");

    // Read back via the admin /users/{u}/config endpoint and verify the
    // prefs landed in overrides.desktop.
    let admin_token = ctx.mint_user_jwt("admin-fixture", "admin-fixture", &["hearth-admins"]);
    let (status, cfg): (_, UserConfig) = send_json(
        &ctx.router,
        "GET",
        "/api/v1/users/alice/config",
        None,
        Some(&admin_token),
    )
    .await;
    assert_eq!(status, 200);

    let desktop = cfg
        .overrides
        .get("desktop")
        .expect("overrides.desktop should be present");
    let parsed: DesktopPreferences =
        serde_json::from_value(desktop.clone()).expect("desktop prefs must round-trip");
    assert_eq!(
        parsed.favorite_apps.as_deref(),
        Some(
            &[
                "firefox.desktop".to_string(),
                "org.gnome.Nautilus.desktop".to_string()
            ][..]
        ),
    );
    assert_eq!(parsed.dark_mode, Some(true));
    assert_eq!(
        parsed.wallpaper_uri.as_deref(),
        Some("file:///usr/share/backgrounds/blobs-l.svg"),
    );
}

#[tokio::test]
#[ignore]
async fn sync_desktop_prefs_rejects_mismatched_machine_token() {
    let ctx = test_app_with_auth().await;
    let machine = create_machine_http_authed(&ctx, "desktop-prefs-mismatch-host").await;

    // Mint a token for a *different* machine id and try to write prefs on
    // behalf of `machine`. The handler must refuse — otherwise any
    // compromised device could rewrite arbitrary users' configs across the
    // fleet.
    let other_machine_id = Uuid::new_v4();
    let bad_token = ctx.mint_machine_jwt(other_machine_id);

    let uri = format!("/api/v1/machines/{}/users/alice/desktop-prefs", machine.id);
    let body = json!({ "desktop": { "dark_mode": false } });

    let status = send_status(&ctx.router, "PUT", &uri, Some(body), Some(&bad_token)).await;
    assert_eq!(
        status, 403,
        "expected 403 Forbidden on machine_id/path mismatch, got {status}"
    );
}

#[tokio::test]
#[ignore]
async fn sync_desktop_prefs_requires_machine_identity() {
    let ctx = test_app_with_auth().await;
    let machine_id = Uuid::new_v4();

    let uri = format!("/api/v1/machines/{machine_id}/users/alice/desktop-prefs");
    let body = json!({ "desktop": { "dark_mode": true } });

    // No token → 401
    let status = send_status(&ctx.router, "PUT", &uri, Some(body.clone()), None).await;
    assert_eq!(status, 401, "expected 401 without any token");

    // User token (not a machine token) → 401
    let user_token = ctx.mint_user_jwt("user-1", "user", &["hearth-users"]);
    let status = send_status(&ctx.router, "PUT", &uri, Some(body), Some(&user_token)).await;
    assert_eq!(
        status, 401,
        "user JWT must not be accepted for machine-scoped endpoint"
    );
}
