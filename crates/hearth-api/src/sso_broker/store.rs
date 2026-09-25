//! Persistence for broker-minted tokens.
//!
//! Every id_token and access_token the broker mints is recorded here by
//! jti so `/oidc/userinfo` (and, eventually, `/oidc/revoke` in Task 13) can
//! check whether a presented token has been revoked. Recording is on the
//! hot path of `/oidc/token` and is fail-closed there: a token that can't
//! be recorded is a token that could never be revoked, so the handler
//! must not hand it out.

use chrono::{DateTime, Utc};
use sqlx::PgPool;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    Access,
    Id,
}

impl TokenKind {
    fn as_str(&self) -> &'static str {
        match self {
            TokenKind::Access => "access",
            TokenKind::Id => "id",
        }
    }
}

/// Records a freshly minted token by its jti.
pub async fn record_token(
    pool: &PgPool,
    jti: &str,
    user_sub: &str,
    audience: &str,
    client_id: &str,
    kind: TokenKind,
    expires_at: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "INSERT INTO sso_issued_tokens (jti, user_sub, audience, client_id, kind, expires_at)
         VALUES ($1, $2, $3, $4, $5, $6)",
        jti,
        user_sub,
        audience,
        client_id,
        kind.as_str(),
        expires_at,
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// Returns `true` if `jti` has been explicitly revoked. An unknown jti
/// (never recorded, or already expired and pruned) is reported as not
/// revoked — callers rely on JWT `exp`/signature checks to have already
/// established the token is otherwise valid.
pub async fn is_revoked(pool: &PgPool, jti: &str) -> Result<bool, sqlx::Error> {
    let row = sqlx::query!(
        "SELECT revoked_at FROM sso_issued_tokens WHERE jti = $1",
        jti
    )
    .fetch_optional(pool)
    .await?;
    Ok(row.and_then(|r| r.revoked_at).is_some())
}

/// Marks `jti` revoked. Returns `true` if a row was updated (it existed and
/// was not already revoked); `false` for an unknown jti or a no-op repeat
/// revoke of an already-revoked token.
pub async fn revoke(pool: &PgPool, jti: &str) -> Result<bool, sqlx::Error> {
    let r = sqlx::query!(
        "UPDATE sso_issued_tokens SET revoked_at = now() WHERE jti = $1 AND revoked_at IS NULL",
        jti
    )
    .execute(pool)
    .await?;
    Ok(r.rows_affected() == 1)
}

/// Revokes every currently-active token for a user (e.g. on account
/// disable or password reset). Not wired to an endpoint yet.
pub async fn revoke_all_for_user(pool: &PgPool, user_sub: &str) -> Result<u64, sqlx::Error> {
    let r = sqlx::query!(
        "UPDATE sso_issued_tokens SET revoked_at = now()
         WHERE user_sub = $1 AND revoked_at IS NULL",
        user_sub
    )
    .execute(pool)
    .await?;
    Ok(r.rows_affected())
}
