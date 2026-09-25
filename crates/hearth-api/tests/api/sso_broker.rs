//! Integration tests for Task 12 (`/oidc/token` persists every minted jti to
//! `sso_issued_tokens`, `/oidc/userinfo` rejects a revoked token) and Task 13
//! (`/oidc/revoke`, plus `sso_broker.token_issued`/`sso_broker.token_revoked`
//! audit events).
//!
//! These exercise real Postgres reads/writes (via `TestDb`'s per-test,
//! migrated database), so they're `#[ignore]`d — run with
//! `cargo test --workspace -- --ignored`. The corresponding fail-closed
//! *unit* tests (record/revocation-check failure against a dangling pool)
//! live alongside the handlers in
//! `crates/hearth-api/src/sso_broker/handlers/{token,userinfo,revoke}.rs`.

use crate::common::{SSO_BROKER_TEST_ISSUER, test_app_with_sso_broker};
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use hearth_api::sso_broker::clients::BrokerClient;
use hearth_api::sso_broker::keys::SigningKey;
use hearth_api::sso_broker::sessions::BrokerCode;
use hearth_api::sso_broker::store::{self, TokenKind};
use hearth_api::sso_broker::tokens::{self, BrokerClaims};
use http_body_util::BodyExt;
use sha2::{Digest, Sha256};
use sqlx::Row;
use tower::ServiceExt;

const TEST_PEM: &str = "\
-----BEGIN PRIVATE KEY-----
MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgevZzL1gdAFr88hb2
OF/2NxApJCzGCEDdfSp6VQO30hyhRANCAAQRWz+jn65BtOMvdyHKcvjBeBSDZH2r
1RTwjmYSi9R/zpBnuQ4EiMnCqfMPWiZqB4QdbAd0E7oH50VpuZ1P087G
-----END PRIVATE KEY-----";

fn synapse_client() -> BrokerClient {
    BrokerClient {
        client_id: "synapse".into(),
        client_secret_hash: format!("{:x}", Sha256::digest(b"sekret")),
        redirect_uris: vec!["https://chat.hearth.local/cb".into()],
        scopes: vec!["openid".into()],
        audience: "synapse".into(),
    }
}

#[tokio::test]
#[ignore] // requires PostgreSQL
async fn token_issuance_records_two_rows() {
    let (router, state, _db) = test_app_with_sso_broker(vec![synapse_client()]).await;

    let verifier = "abcdef123456abcdef123456abcdef123456abcdef123456";
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let code_key = state.sso_broker_code_stash.insert(BrokerCode {
        client_id: "synapse".into(),
        redirect_uri: "https://chat.hearth.local/cb".into(),
        code_challenge: challenge,
        code_challenge_method: "S256".into(),
        user_sub: "u-1".into(),
        user_username: Some("alice".into()),
        user_email: Some("a@b".into()),
        user_email_verified: Some(true),
        user_name: Some("Alice".into()),
        user_groups: vec!["hearth-users".into()],
        scope: "openid profile email".into(),
        nonce: None,
    });

    let body = format!(
        "grant_type=authorization_code&code={}&redirect_uri=https%3A%2F%2Fchat.hearth.local%2Fcb&client_id=synapse&client_secret=sekret&code_verifier={}",
        urlencoding::encode(&code_key),
        verifier,
    );
    let req = Request::builder()
        .method("POST")
        .uri("/oidc/token")
        .header("content-type", "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .unwrap();
    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(v["id_token"].as_str().unwrap().contains('.'));
    assert!(v["access_token"].as_str().unwrap().contains('.'));

    let rows = sqlx::query(
        "SELECT jti, kind, user_sub, audience, client_id, expires_at, revoked_at
         FROM sso_issued_tokens ORDER BY kind",
    )
    .fetch_all(&state.pool)
    .await
    .unwrap();
    assert_eq!(rows.len(), 2, "expected one id row + one access row");
    let kinds: Vec<String> = rows.iter().map(|r| r.get::<String, _>("kind")).collect();
    assert_eq!(kinds, vec!["access".to_string(), "id".to_string()]);
    for row in &rows {
        assert_eq!(row.get::<String, _>("user_sub"), "u-1");
        assert_eq!(row.get::<String, _>("audience"), "synapse");
        assert_eq!(row.get::<String, _>("client_id"), "synapse");
        assert!(
            row.get::<Option<chrono::DateTime<chrono::Utc>>, _>("revoked_at")
                .is_none()
        );
        assert!(row.get::<chrono::DateTime<chrono::Utc>, _>("expires_at") > chrono::Utc::now());
    }

    // jti <-> row integration assertion (Task 13 amendment): decode both
    // returned JWTs and confirm each one's jti equals the row of the
    // matching kind, not just that two rows of the right kinds exist.
    let key = SigningKey::load_from_pem(TEST_PEM).unwrap();
    let id_claims = tokens::verify_jwt(
        v["id_token"].as_str().unwrap(),
        &key,
        SSO_BROKER_TEST_ISSUER,
    )
    .unwrap();
    let access_claims = tokens::verify_jwt(
        v["access_token"].as_str().unwrap(),
        &key,
        SSO_BROKER_TEST_ISSUER,
    )
    .unwrap();
    let id_row = rows
        .iter()
        .find(|r| r.get::<String, _>("kind") == "id")
        .unwrap();
    let access_row = rows
        .iter()
        .find(|r| r.get::<String, _>("kind") == "access")
        .unwrap();
    assert_eq!(id_claims.jti, id_row.get::<String, _>("jti"));
    assert_eq!(access_claims.jti, access_row.get::<String, _>("jti"));

    // Task 13: token issuance emits an audit event, put together from the
    // same jtis/scope/client the rows above hold.
    let audit_rows = sqlx::query(
        "SELECT actor, details FROM audit_events WHERE event_type = 'sso_broker.token_issued'",
    )
    .fetch_all(&state.pool)
    .await
    .unwrap();
    assert_eq!(audit_rows.len(), 1);
    assert_eq!(audit_rows[0].get::<String, _>("actor"), "u-1");
    let details: serde_json::Value = audit_rows[0].get("details");
    assert_eq!(details["client_id"], "synapse");
    assert_eq!(details["id_jti"], id_claims.jti);
    assert_eq!(details["access_jti"], access_claims.jti);
}

#[tokio::test]
#[ignore] // requires PostgreSQL
async fn userinfo_succeeds_then_401_after_revoke() {
    let (router, state, _db) = test_app_with_sso_broker(vec![]).await;
    let key = SigningKey::load_from_pem(TEST_PEM).unwrap();
    let now = chrono::Utc::now().timestamp();
    let jti = uuid::Uuid::new_v4().to_string();
    let claims = BrokerClaims {
        iss: SSO_BROKER_TEST_ISSUER.into(),
        sub: "u-1".into(),
        aud: "synapse".into(),
        exp: now + 3600,
        iat: now,
        jti: jti.clone(),
        scope: "openid profile email".into(),
        preferred_username: Some("alice".into()),
        email: Some("a@b".into()),
        email_verified: Some(true),
        name: Some("Alice".into()),
        groups: vec!["hearth-users".into()],
        nonce: None,
    };
    let token = tokens::mint_jwt(&claims, &key).unwrap();
    let expires_at = chrono::DateTime::from_timestamp(claims.exp, 0).unwrap();

    store::record_token(
        &state.pool,
        &jti,
        "u-1",
        "synapse",
        "synapse",
        TokenKind::Access,
        expires_at,
    )
    .await
    .unwrap();

    let make_req = || {
        Request::builder()
            .uri("/oidc/userinfo")
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap()
    };

    let resp = router.clone().oneshot(make_req()).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["sub"], "u-1");

    assert!(store::revoke(&state.pool, &jti, "synapse").await.unwrap());

    let resp2 = router.clone().oneshot(make_req()).await.unwrap();
    assert_eq!(resp2.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
#[ignore] // requires PostgreSQL
async fn store_record_then_revoke_round_trip() {
    let db = crate::common::TestDb::new().await;
    let jti = uuid::Uuid::new_v4().to_string();
    let exp = chrono::Utc::now() + chrono::Duration::seconds(60);
    store::record_token(
        &db.pool,
        &jti,
        "u-test",
        "synapse",
        "synapse",
        TokenKind::Access,
        exp,
    )
    .await
    .unwrap();
    assert!(!store::is_revoked(&db.pool, &jti).await.unwrap());
    assert!(store::revoke(&db.pool, &jti, "synapse").await.unwrap());
    assert!(store::is_revoked(&db.pool, &jti).await.unwrap());
    assert!(
        !store::revoke(&db.pool, &jti, "synapse").await.unwrap(),
        "second revoke should be a no-op"
    );
}

#[tokio::test]
#[ignore] // requires PostgreSQL
async fn revoke_endpoint_own_token_then_userinfo_401() {
    let (router, state, _db) = test_app_with_sso_broker(vec![synapse_client()]).await;
    let key = SigningKey::load_from_pem(TEST_PEM).unwrap();
    let now = chrono::Utc::now().timestamp();
    let jti = uuid::Uuid::new_v4().to_string();
    let claims = BrokerClaims {
        iss: SSO_BROKER_TEST_ISSUER.into(),
        sub: "u-1".into(),
        aud: "synapse".into(),
        exp: now + 3600,
        iat: now,
        jti: jti.clone(),
        scope: "openid profile email".into(),
        preferred_username: Some("alice".into()),
        email: Some("a@b".into()),
        email_verified: Some(true),
        name: Some("Alice".into()),
        groups: vec!["hearth-users".into()],
        nonce: None,
    };
    let token = tokens::mint_jwt(&claims, &key).unwrap();
    let expires_at = chrono::DateTime::from_timestamp(claims.exp, 0).unwrap();
    store::record_token(
        &state.pool,
        &jti,
        "u-1",
        "synapse",
        "synapse",
        TokenKind::Access,
        expires_at,
    )
    .await
    .unwrap();

    // The token works before revocation.
    let userinfo_req = || {
        Request::builder()
            .uri("/oidc/userinfo")
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap()
    };
    let resp = router.clone().oneshot(userinfo_req()).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // synapse revokes its own token.
    let revoke_body = format!(
        "token={}&client_id=synapse&client_secret=sekret",
        urlencoding::encode(&token)
    );
    let revoke_req = Request::builder()
        .method("POST")
        .uri("/oidc/revoke")
        .header("content-type", "application/x-www-form-urlencoded")
        .body(Body::from(revoke_body))
        .unwrap();
    let revoke_resp = router.clone().oneshot(revoke_req).await.unwrap();
    assert_eq!(revoke_resp.status(), StatusCode::OK);

    // ... and userinfo now rejects it.
    let resp2 = router.clone().oneshot(userinfo_req()).await.unwrap();
    assert_eq!(resp2.status(), StatusCode::UNAUTHORIZED);

    let audit_rows = sqlx::query(
        "SELECT actor, details FROM audit_events WHERE event_type = 'sso_broker.token_revoked'",
    )
    .fetch_all(&state.pool)
    .await
    .unwrap();
    assert_eq!(audit_rows.len(), 1);
    assert_eq!(audit_rows[0].get::<String, _>("actor"), "synapse");
    let details: serde_json::Value = audit_rows[0].get("details");
    assert_eq!(details["jti"], jti);
}

#[tokio::test]
#[ignore] // requires PostgreSQL
async fn revoke_endpoint_foreign_client_token_is_noop_and_token_still_works() {
    let other_client = BrokerClient {
        client_id: "nextcloud".into(),
        client_secret_hash: format!("{:x}", Sha256::digest(b"other-sekret")),
        redirect_uris: vec!["https://cloud.hearth.local/cb".into()],
        scopes: vec!["openid".into()],
        audience: "nextcloud".into(),
    };
    let (router, state, _db) = test_app_with_sso_broker(vec![synapse_client(), other_client]).await;
    let key = SigningKey::load_from_pem(TEST_PEM).unwrap();
    let now = chrono::Utc::now().timestamp();
    let jti = uuid::Uuid::new_v4().to_string();
    let claims = BrokerClaims {
        iss: SSO_BROKER_TEST_ISSUER.into(),
        sub: "u-1".into(),
        aud: "synapse".into(),
        exp: now + 3600,
        iat: now,
        jti: jti.clone(),
        scope: "openid profile email".into(),
        preferred_username: Some("alice".into()),
        email: Some("a@b".into()),
        email_verified: Some(true),
        name: Some("Alice".into()),
        groups: vec!["hearth-users".into()],
        nonce: None,
    };
    let token = tokens::mint_jwt(&claims, &key).unwrap();
    let expires_at = chrono::DateTime::from_timestamp(claims.exp, 0).unwrap();
    // Token belongs to "synapse".
    store::record_token(
        &state.pool,
        &jti,
        "u-1",
        "synapse",
        "synapse",
        TokenKind::Access,
        expires_at,
    )
    .await
    .unwrap();

    // "nextcloud" (a different, legitimately-authenticated client) tries to
    // revoke it.
    let revoke_body = format!(
        "token={}&client_id=nextcloud&client_secret=other-sekret",
        urlencoding::encode(&token)
    );
    let revoke_req = Request::builder()
        .method("POST")
        .uri("/oidc/revoke")
        .header("content-type", "application/x-www-form-urlencoded")
        .body(Body::from(revoke_body))
        .unwrap();
    let revoke_resp = router.clone().oneshot(revoke_req).await.unwrap();
    // RFC 7009 §2.2: still 200 — no oracle for whether a foreign jti exists.
    assert_eq!(revoke_resp.status(), StatusCode::OK);

    // The token is untouched: still not revoked, userinfo still succeeds.
    assert!(!store::is_revoked(&state.pool, &jti).await.unwrap());
    let userinfo_req = Request::builder()
        .uri("/oidc/userinfo")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    let resp = router.clone().oneshot(userinfo_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // No audit event for a no-op revoke.
    let audit_rows =
        sqlx::query("SELECT id FROM audit_events WHERE event_type = 'sso_broker.token_revoked'")
            .fetch_all(&state.pool)
            .await
            .unwrap();
    assert!(audit_rows.is_empty());
}
