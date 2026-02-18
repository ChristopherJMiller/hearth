-- Machine enrollment and lifecycle tracking

CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

CREATE TYPE enrollment_status AS ENUM (
    'pending',
    'approved',
    'enrolled',
    'provisioning',
    'active',
    'decommissioned'
);

CREATE TABLE machines (
    id                   UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    hostname             TEXT NOT NULL,
    hardware_fingerprint TEXT,
    enrollment_status    enrollment_status NOT NULL DEFAULT 'pending',
    current_closure      TEXT,
    target_closure       TEXT,
    rollback_closure     TEXT,
    role                 TEXT,
    tags                 TEXT[] NOT NULL DEFAULT '{}',
    extra_config         JSONB,
    last_heartbeat       TIMESTAMPTZ,
    created_at           TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at           TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_machines_enrollment_status ON machines (enrollment_status);
CREATE INDEX idx_machines_role ON machines (role);
