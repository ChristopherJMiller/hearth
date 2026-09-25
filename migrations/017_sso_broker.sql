-- SSO broker: track issued tokens for revocation and audit.

CREATE TABLE sso_issued_tokens (
    jti          TEXT PRIMARY KEY,
    user_sub     TEXT NOT NULL,
    audience     TEXT NOT NULL,
    issued_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at   TIMESTAMPTZ NOT NULL,
    revoked_at   TIMESTAMPTZ,
    client_id    TEXT NOT NULL,
    -- Optional device hint (filled in once device backchannel exists; null for browser flows)
    device_id    UUID REFERENCES machines(id) ON DELETE SET NULL,
    -- Token kind: 'access' or 'id'
    kind         TEXT NOT NULL CHECK (kind IN ('access', 'id'))
);

CREATE INDEX idx_sso_issued_tokens_user
    ON sso_issued_tokens (user_sub);
CREATE INDEX idx_sso_issued_tokens_expires
    ON sso_issued_tokens (expires_at)
    WHERE revoked_at IS NULL;
CREATE INDEX idx_sso_issued_tokens_active_per_user
    ON sso_issued_tokens (user_sub, audience)
    WHERE revoked_at IS NULL;
