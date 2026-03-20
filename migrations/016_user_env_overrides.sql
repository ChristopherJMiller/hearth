-- Per-user environment configuration and build tracking.
--
-- user_configs stores per-user overrides (machine-independent source of truth).
-- Role templates are initial seeds; once a user_config row exists, the per-user
-- closure is the source of truth for that user's environment.
--
-- user_env_build_jobs tracks per-user closure builds (separate from machine-level
-- build_jobs which handle fleet-wide NixOS system closures).

CREATE TYPE user_env_build_status AS ENUM ('pending', 'building', 'built', 'failed');

CREATE TABLE user_configs (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    username        TEXT NOT NULL UNIQUE,
    base_role       TEXT NOT NULL DEFAULT 'default',
    overrides       JSONB NOT NULL DEFAULT '{}',
    config_hash     TEXT,
    latest_closure  TEXT,
    build_status    user_env_build_status NOT NULL DEFAULT 'pending',
    build_error     TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_user_configs_build_status ON user_configs(build_status);

CREATE TABLE user_env_build_jobs (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    username        TEXT NOT NULL,
    config_hash     TEXT NOT NULL,
    status          build_job_status NOT NULL DEFAULT 'pending',
    worker_id       TEXT,
    claimed_at      TIMESTAMPTZ,
    closure         TEXT,
    error_message   TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_user_env_build_jobs_status ON user_env_build_jobs(status);
CREATE INDEX idx_user_env_build_jobs_username ON user_env_build_jobs(username);
