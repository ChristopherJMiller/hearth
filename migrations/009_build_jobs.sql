-- Build job queue for the standalone build worker.

CREATE TYPE build_job_status AS ENUM (
    'pending',
    'claimed',
    'evaluating',
    'building',
    'pushing',
    'deploying',
    'completed',
    'failed'
);

CREATE TABLE build_jobs (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    status          build_job_status NOT NULL DEFAULT 'pending',

    -- Build inputs
    flake_ref       TEXT NOT NULL,
    target_filter   JSONB,
    canary_size     INT NOT NULL DEFAULT 1,
    batch_size      INT NOT NULL DEFAULT 5,
    failure_threshold DOUBLE PRECISION NOT NULL DEFAULT 0.1,

    -- Worker tracking
    worker_id       TEXT,               -- Identifier of the worker that claimed this job
    claimed_at      TIMESTAMPTZ,

    -- Build outputs
    deployment_id   UUID REFERENCES deployments(id),
    closure         TEXT,               -- Primary closure built
    closures_built  INT,
    closures_pushed INT,
    total_machines  INT,

    -- Error tracking
    error_message   TEXT,

    -- Timestamps
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_build_jobs_status ON build_jobs(status);
CREATE INDEX idx_build_jobs_created ON build_jobs(created_at);
