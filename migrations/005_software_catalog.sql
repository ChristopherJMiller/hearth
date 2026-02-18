-- Self-service software catalog and request workflow

CREATE TYPE install_method AS ENUM (
    'nix_system',
    'nix_user',
    'flatpak',
    'home_manager'
);

CREATE TYPE software_request_status AS ENUM (
    'pending',
    'approved',
    'denied',
    'installing',
    'installed',
    'failed'
);

CREATE TABLE software_catalog (
    id                 UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name               TEXT NOT NULL,
    description        TEXT,
    category           TEXT,
    install_method     install_method NOT NULL,
    flatpak_ref        TEXT,
    nix_attr           TEXT,
    icon_url           TEXT,
    approval_required  BOOLEAN NOT NULL DEFAULT true,
    auto_approve_roles TEXT[] NOT NULL DEFAULT '{}',
    created_at         TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE software_requests (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    catalog_entry_id UUID NOT NULL REFERENCES software_catalog(id),
    machine_id       UUID NOT NULL REFERENCES machines(id),
    username         TEXT NOT NULL,
    status           software_request_status NOT NULL DEFAULT 'pending',
    requested_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    resolved_at      TIMESTAMPTZ,
    resolved_by      TEXT
);

CREATE INDEX idx_software_requests_machine_username ON software_requests (machine_id, username);
CREATE INDEX idx_software_requests_status ON software_requests (status);
