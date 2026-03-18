-- Phase 5B: Compliance Engine
-- Adds compliance policies (Nix assertion expressions), per-deployment policy results,
-- and SBOM file references for built closures.

-- Compliance policies: Nix expressions evaluated at build time against machine configs.
CREATE TABLE compliance_policies (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL UNIQUE,
    description TEXT,
    -- Nix expression evaluated against the machine's NixOS `config` attrset.
    -- Example: "config.networking.firewall.enable == true"
    nix_expression TEXT NOT NULL,
    severity TEXT NOT NULL DEFAULT 'medium'
        CHECK (severity IN ('low', 'medium', 'high', 'critical')),
    -- Optional STIG/CIS control ID for traceability.
    control_id TEXT,
    enabled BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Per-deployment, per-machine policy evaluation results.
CREATE TABLE policy_results (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    deployment_id UUID NOT NULL REFERENCES deployments(id) ON DELETE CASCADE,
    machine_id UUID NOT NULL REFERENCES machines(id) ON DELETE CASCADE,
    policy_id UUID NOT NULL REFERENCES compliance_policies(id) ON DELETE CASCADE,
    passed BOOLEAN NOT NULL,
    message TEXT,
    evaluated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (deployment_id, machine_id, policy_id)
);

CREATE INDEX idx_policy_results_deployment ON policy_results (deployment_id);
CREATE INDEX idx_policy_results_machine ON policy_results (machine_id);

-- SBOM file references: pointer to the SBOM file on disk / object storage.
CREATE TABLE deployment_sboms (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    deployment_id UUID NOT NULL REFERENCES deployments(id) ON DELETE CASCADE,
    machine_id UUID NOT NULL REFERENCES machines(id) ON DELETE CASCADE,
    closure TEXT NOT NULL,
    -- Relative path within $HEARTH_SBOM_DIR (e.g., "{deployment_id}/{hostname}.cdx.json")
    sbom_path TEXT NOT NULL,
    format TEXT NOT NULL DEFAULT 'cyclonedx-json',
    generated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (deployment_id, machine_id)
);

CREATE INDEX idx_deployment_sboms_machine ON deployment_sboms (machine_id);
