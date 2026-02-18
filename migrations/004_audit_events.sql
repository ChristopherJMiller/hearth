-- Immutable audit log for all significant actions

CREATE TABLE audit_events (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    event_type  TEXT NOT NULL,
    actor       TEXT,
    machine_id  UUID REFERENCES machines(id),
    details     JSONB NOT NULL DEFAULT '{}',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_audit_events_event_type ON audit_events (event_type);
CREATE INDEX idx_audit_events_machine_id ON audit_events (machine_id);
CREATE INDEX idx_audit_events_created_at ON audit_events (created_at);
