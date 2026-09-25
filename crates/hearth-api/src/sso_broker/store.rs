//! Persistence for broker-minted tokens.
//!
//! Every id_token and access_token the broker mints is recorded here by
//! jti so `/oidc/userinfo` and `/oidc/revoke` can check whether a presented
//! token has been revoked. Recording is on the hot path of `/oidc/token`
//! and is fail-closed there: a token that can't be recorded is a token
//! that could never be revoked, so the handler must not hand it out.

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

/// Marks `jti` revoked, scoped to `client_id`. Returns `true` if a row was
/// updated (it existed, belonged to `client_id`, and was not already
/// revoked); `false` for an unknown jti, a no-op repeat revoke of an
/// already-revoked token, or — deliberately — a jti that exists but was
/// issued to a *different* client. Scoping the `UPDATE` itself (rather than
/// checking `client_id` separately) means a client can never learn from the
/// response whether some other client's jti exists at all: unknown and
/// foreign both come back `false`.
pub async fn revoke(pool: &PgPool, jti: &str, client_id: &str) -> Result<bool, sqlx::Error> {
    let r = sqlx::query!(
        "UPDATE sso_issued_tokens SET revoked_at = now()
         WHERE jti = $1 AND client_id = $2 AND revoked_at IS NULL",
        jti,
        client_id,
    )
    .execute(pool)
    .await?;
    Ok(r.rows_affected() == 1)
}

/// Records both tokens minted by a single `/oidc/token` grant (an id_token
/// and an access_token) in one transaction, so a partial write — one row
/// persisted, the other lost to an error — can never happen. The caller
/// still treats this fail-closed: on `Err`, neither token may be handed to
/// the client, since a token that couldn't be recorded could never be
/// revoked.
#[allow(clippy::too_many_arguments)]
pub async fn record_token_pair(
    pool: &PgPool,
    user_sub: &str,
    audience: &str,
    client_id: &str,
    id_jti: &str,
    id_expires_at: DateTime<Utc>,
    access_jti: &str,
    access_expires_at: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query!(
        "INSERT INTO sso_issued_tokens (jti, user_sub, audience, client_id, kind, expires_at)
         VALUES ($1, $2, $3, $4, $5, $6)",
        id_jti,
        user_sub,
        audience,
        client_id,
        TokenKind::Id.as_str(),
        id_expires_at,
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!(
        "INSERT INTO sso_issued_tokens (jti, user_sub, audience, client_id, kind, expires_at)
         VALUES ($1, $2, $3, $4, $5, $6)",
        access_jti,
        user_sub,
        audience,
        client_id,
        TokenKind::Access.as_str(),
        access_expires_at,
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
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
