-- Per-machine deployment tracking for staged rollouts

CREATE TYPE machine_update_status AS ENUM (
    'pending',
    'downloading',
    'switching',
    'completed',
    'failed',
    'rolled_back'
);

CREATE TABLE deployment_machines (
    deployment_id  UUID NOT NULL REFERENCES deployments(id) ON DELETE CASCADE,
    machine_id     UUID NOT NULL REFERENCES machines(id) ON DELETE CASCADE,
    status         machine_update_status NOT NULL DEFAULT 'pending',
    started_at     TIMESTAMPTZ,
    completed_at   TIMESTAMPTZ,
    error_message  TEXT,
    PRIMARY KEY (deployment_id, machine_id)
);

CREATE INDEX idx_deployment_machines_status ON deployment_machines (status);
CREATE INDEX idx_deployment_machines_machine ON deployment_machines (machine_id);

-- Extend deployments table with rollout configuration
ALTER TABLE deployments
    ADD COLUMN canary_size       INT NOT NULL DEFAULT 1,
    ADD COLUMN batch_size        INT NOT NULL DEFAULT 5,
    ADD COLUMN failure_threshold FLOAT NOT NULL DEFAULT 0.1,
    ADD COLUMN rollback_reason   TEXT;
