-- Fleet-wide deployment tracking

CREATE TYPE deployment_status AS ENUM (
    'pending',
    'canary',
    'rolling',
    'completed',
    'failed',
    'rolled_back'
);

CREATE TABLE deployments (
    id                 UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    closure            TEXT NOT NULL,
    module_library_ref TEXT NOT NULL,
    instance_data_hash TEXT NOT NULL,
    status             deployment_status NOT NULL DEFAULT 'pending',
    target_filter      JSONB NOT NULL DEFAULT '{}',
    total_machines     INT NOT NULL DEFAULT 0,
    succeeded          INT NOT NULL DEFAULT 0,
    failed             INT NOT NULL DEFAULT 0,
    created_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at         TIMESTAMPTZ NOT NULL DEFAULT now()
);
