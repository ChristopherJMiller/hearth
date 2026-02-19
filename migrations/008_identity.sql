-- User identity tracking and machine-to-user binding
--
-- Stores known users from the identity provider (Kanidm) and links
-- machines to the user who enrolled them.

CREATE TABLE users (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    username     TEXT NOT NULL UNIQUE,
    display_name TEXT,
    email        TEXT,
    kanidm_uuid  TEXT,
    groups       TEXT[] NOT NULL DEFAULT '{}',
    last_seen    TIMESTAMPTZ,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_users_username ON users(username);

-- Track which user enrolled each machine and the hash of the machine's
-- current auth token (for revocation checks).
ALTER TABLE machines
    ADD COLUMN enrolled_by       TEXT,
    ADD COLUMN machine_token_hash TEXT;

CREATE INDEX idx_machines_enrolled_by ON machines(enrolled_by);
