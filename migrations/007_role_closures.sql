-- Role closure templates: maps role names to pre-built NixOS system closures.
-- One active closure per role, upserted when rebuilt.
CREATE TABLE role_closures (
    role        TEXT PRIMARY KEY,
    closure     TEXT NOT NULL,
    built_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
