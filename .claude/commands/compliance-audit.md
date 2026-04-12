---
description: Audit Hearth compliance posture against a target framework (cis-level1, cis-level2, stig, nist-800-53, soc2) and produce a gap report with recommended next controls to implement.
argument-hint: "<framework> [--output json]"
---

Run a compliance audit against the framework specified in `$ARGUMENTS`.

## Arguments

- **First positional argument:** framework name. Must be one of:
  - `cis-level1` (default if omitted — CIS Benchmarks Level 1 for Linux
    workstations — **primary target for Hearth**)
  - `cis-level2`
  - `stig` (DISA STIG for RHEL-like systems, mapped to NixOS)
  - `nist-800-53` (Moderate baseline)
  - `soc2`
- `--output json` — emit the gap report as JSON instead of markdown
  (useful for CI integration)

If the argument is missing or invalid, default to `cis-level1` and tell
the user.

## Execution

Spawn the `compliance-auditor` subagent with a prompt that:

1. Provides the target framework from `$ARGUMENTS`
2. Asks it to inventory existing controls by reading
   `modules/compliance/*.nix` and `docs/compliance-controls.yaml`
3. Asks it to scan `modules/hardening.nix` for overlapping settings
4. Asks it to scan application code
   (`crates/hearth-api/src/routes/compliance.rs`,
   `crates/hearth-api/src/auth.rs`,
   `migrations/004_audit_events.sql`) for runtime/auth controls
5. Produces the gap report in the format specified in the agent's
   system prompt

## Presentation

Show the user:
1. The coverage percentage summary
2. The 5 highest-priority missing controls with effort estimates
3. A list of partial-coverage items that could be promoted to full
   coverage cheaply
4. Any "inherent" controls that should be documented but need no new
   code

Offer to:
- Generate skeleton `.nix` files for the top missing controls
- Update `docs/compliance-controls.yaml` with newly-discovered inherent
  or partial controls
- Wire new controls into the correct profile in
  `modules/compliance/default.nix`

## Caveat

Always remind the user that this audit is an engineering gap analysis,
not a formal compliance certification. Formal attestation requires a
qualified external auditor and evidence collection beyond configuration
inspection.
