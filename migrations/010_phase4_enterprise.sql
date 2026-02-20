-- Phase 4: Enterprise Hardening schema additions
--
-- Adds: remote actions, hardware profiles, recovery keys, user-env build tracking

-- Remote actions: dispatched via heartbeat, executed by agents
CREATE TYPE action_type AS ENUM ('lock', 'restart', 'rebuild', 'run_command');
CREATE TYPE action_status AS ENUM ('pending', 'delivered', 'running', 'completed', 'failed');

CREATE TABLE pending_actions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    machine_id UUID NOT NULL REFERENCES machines(id) ON DELETE CASCADE,
    action_type action_type NOT NULL,
    payload JSONB NOT NULL DEFAULT '{}',
    status action_status NOT NULL DEFAULT 'pending',
    created_by TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    delivered_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    result JSONB
);

CREATE INDEX idx_pending_actions_machine_status ON pending_actions (machine_id, status);
CREATE INDEX idx_pending_actions_created_at ON pending_actions (created_at);

-- Hardware profile selection for build pipeline
ALTER TABLE machines ADD COLUMN IF NOT EXISTS hardware_profile TEXT;

-- TPM-FDE recovery key escrow
ALTER TABLE machines ADD COLUMN IF NOT EXISTS recovery_key_encrypted TEXT;

-- User environment build tracking on build_jobs
ALTER TABLE build_jobs ADD COLUMN IF NOT EXISTS user_env_username TEXT;
ALTER TABLE build_jobs ADD COLUMN IF NOT EXISTS user_env_machine_id UUID REFERENCES machines(id);
