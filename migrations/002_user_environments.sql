-- Per-user environment state on each machine

CREATE TYPE user_env_status AS ENUM (
    'pending',
    'building',
    'ready',
    'activating',
    'active',
    'failed'
);

CREATE TABLE user_environments (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    machine_id      UUID NOT NULL REFERENCES machines(id),
    username        TEXT NOT NULL,
    role            TEXT NOT NULL,
    current_closure TEXT,
    target_closure  TEXT,
    status          user_env_status NOT NULL DEFAULT 'pending',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),

    UNIQUE (machine_id, username)
);
