# Compliance Registry

This is the human-readable view of Hearth's compliance posture. The
machine-readable source of truth is `docs/compliance-controls.yaml` —
this document is derived from it, plus narrative context for operators
and auditors.

## Primary target: CIS Benchmarks Level 1 for Linux Workstations

We chose CIS Level 1 for NixOS workstations as our baseline for these
reasons:

- It maps directly to Hearth's use case (a fleet of Linux desktops)
- NixOS's declarative model makes most Level 1 controls one- or
  two-line module options
- Level 1 is "do no harm" — it doesn't break normal desktop workflows
- We are already ~70% there via `modules/hardening.nix`

**Secondary frameworks we track:**

- **DISA STIG** — relevant for government/defense customers. Already
  5% started with SSH and USB mass storage controls.
- **NIST 800-53 Moderate** — the federal baseline. Technical controls
  are ~45% covered; process controls (incident response, contingency
  planning) are out of scope for the code registry and tracked
  elsewhere.
- **SOC 2** — the trust services criteria. Primarily process; the
  technical piece overlaps with CIS.

We do **not** claim formal compliance certification. This registry is
an engineering gap analysis to help us make informed design choices and
prepare for a future audit.

## How it works

Compliance controls live in `modules/compliance/`. Each file defines
one control following the `cis-1-1-1.nix` template:

```nix
{ config, lib, ... }:
let cfg = config.services.hearth.compliance."cis-X-Y-Z"; in {
  options.services.hearth.compliance."cis-X-Y-Z" = {
    enable = lib.mkEnableOption "CIS X.Y.Z — short title";
    meta = lib.mkOption {
      type = lib.types.attrs;
      readOnly = true;
      default = {
        id = "CIS-X.Y.Z";
        title = "Human-readable control name";
        severity = "low|medium|high|critical";
        description = "What this control enforces";
        family = "filesystem|network|access-control|…";
        benchmark = "CIS NixOS Level 1";
      };
    };
  };
  config = lib.mkIf cfg.enable { /* NixOS settings */ };
}
```

Controls are activated via profiles in `modules/compliance/default.nix`.
A fleet operator enables a profile via their NixOS configuration:

```nix
services.hearth.compliance.profile = "cis-level1";
```

## Current coverage

### CIS Level 1 — implemented

| ID | Title | Module |
|----|-------|--------|
| CIS-1.1.1 | Disable mounting of uncommon filesystems | `modules/compliance/cis-1-1-1.nix` |
| CIS-3.4.1 | Ensure firewall is enabled with logging | `modules/compliance/cis-3-4-1.nix` |
| CIS-4.2.1 | Ensure audit framework logs security events | `modules/compliance/cis-4-2-1.nix` |

### CIS Level 1 — partial or inherent

Controls that are effectively satisfied but not formalized as compliance
modules. These should be promoted to modules (even if the config block
is empty with a comment) so they appear in the tracking system.

| ID | Title | Current state |
|----|-------|---------------|
| CIS-1.1.2 | `/tmp` with nosuid,nodev,noexec | Set in strict mode only |
| CIS-1.3.1 | Filesystem integrity checking | Inherent (Nix store immutability) |
| CIS-1.4.1 | Bootloader password / tamper resistance | Inherent (Secure Boot + TPM) |
| CIS-1.5.1 | Core dumps restricted | Set via `hardening.nix` |
| CIS-3.1.1 | IPv4 forwarding disabled | `hardening.nix` sets related sysctls but not this one explicitly |
| CIS-3.3.1 | Source-routed packets rejected | Partial |
| CIS-5.2.2 | SSH host key permissions | Inherent (NixOS default) |
| CIS-5.2.6 | SSH X11 forwarding disabled | Set in strict mode only |
| CIS-5.2.8 | SSH MaxAuthTries ≤ 4 | Set via `stig-v-230223.nix`, needs to activate under cis-level1 profile |
| CIS-5.4.1 | Password creation requirements | Delegated to Kanidm |
| CIS-5.4.2 | System accounts without login shell | Inherent (NixOS default) |

### DISA STIG — implemented

| ID | Title | Module |
|----|-------|--------|
| STIG-V-230223 | SSH protocol 2 with strong crypto | `modules/compliance/stig-v-230223.nix` |
| STIG-V-230271 | Disable USB mass storage | `modules/compliance/stig-v-230271.nix` |

### NIST 800-53 Moderate — technical controls

| ID | Title | Status | Where |
|----|-------|--------|-------|
| AC-2 | Account Management | Implemented | Kanidm + identity_sync |
| AC-3 | Access Enforcement | Implemented | 5 Axum auth extractors |
| AU-2 | Event Logging | Implemented | `audit_events` table |
| AU-9 | Audit Information Protection | Partial | No DB-level delete prevention |
| SC-8 | Transmission Confidentiality | Implemented | Headscale mesh + cert-manager TLS |
| SC-12 | Cryptographic Key Management | Partial | Keys in K8s Secrets, no rotation |
| IA-5 | Authenticator Management | Partial | Machine tokens don't rotate |
| SC-5 | Denial of Service Protection | Missing | No rate limiting |

## Priority queue — next controls to implement

Ordered by impact, effort, and coverage improvement.

### Phase 1 (immediate, ~1 week)

1. **CIS-1.5.1 — Core dumps restricted** (trivial)
   - Create `modules/compliance/cis-1-5-1.nix` with empty config and a
     note that it's satisfied by `hardening.nix`
2. **CIS-3.1.1 — IPv4 forwarding disabled** (trivial)
   - Create `modules/compliance/cis-3-1-1.nix` setting
     `boot.kernel.sysctl."net.ipv4.ip_forward" = 0`
3. **CIS-3.3.1 — Source-routed packets rejected** (trivial)
   - Set `accept_source_route = 0` for IPv4 and IPv6
4. **STIG-V-230234 — Disable Ctrl-Alt-Del** (trivial)
   - `systemd.services."ctrl-alt-del.target".enable = false;`
5. **CIS-5.2.8 — SSH MaxAuthTries wired into cis-level1 profile**
   (trivial) — update `modules/compliance/default.nix` only

### Phase 2 (first month, ~4 weeks)

6. **CIS-5.3.1 / STIG-V-230330 — pam_faillock account lockout**
   (moderate)
7. **CIS-5.2.6 — SSH X11 forwarding disabled unconditionally**
   (trivial) — move from strict mode into a compliance module
8. **CIS-1.1.2 — /tmp mount options** (moderate) — fleet operators
   should be able to enable this without switching to strict hardening
9. **NIST-SC-5 — Rate limiting middleware** (moderate) — implement
   `tower_governor` in `hearth-api`
10. **NIST-AU-9 — Audit log DB-level delete prevention** (moderate) —
    trigger or role-based restriction

### Phase 3 (first quarter, ~3 months)

11. **Helm chart hardening** — add pod and container `securityContext`,
    `NetworkPolicy` resources, namespace PSS labels (maps to CIS
    Kubernetes 5.2.x and 5.3.x; NIST SC-39, SC-7)
12. **CORS restriction** — replace `allow_origin(Any)` with an env-var
    allowlist (NIST AC-4)
13. **Machine token rotation** — refresh via heartbeat (NIST IA-5(1))
14. **CIS 4.1.x — audit rule coverage expansion** — cover all
    CIS-required events in the audit subsystem
15. **Kanidm password policy documentation** — formalize CIS-5.4.1 as
    "satisfied by the Kanidm deployment with policy X"

### Phase 4 (six months and beyond)

- Full CIS Level 1 coverage (~40 more controls, most trivial)
- Expand DISA STIG coverage to ~30 controls
- NIST 800-53 process control documentation (in a separate document —
  incident response, contingency planning, personnel security)
- SOC 2 evidence collection framework
- FIPS 140-2 crypto module validation (if a customer requests it)

## How to add a new control

1. Copy `modules/compliance/cis-1-1-1.nix` to a new file with the
   control's ID as the filename (e.g., `cis-5-3-1.nix`)
2. Fill in the `meta` attribute with the correct id, title, severity,
   description, family, and benchmark
3. Implement the NixOS settings in the `config` block (or leave it
   empty with a comment if the control is satisfied by NixOS defaults
   or other modules)
4. Add the file to `imports` in `modules/compliance/default.nix`
5. Wire the new control into the appropriate profile's `mkIf` block
6. Update `docs/compliance-controls.yaml` with the new entry
7. Update this document if the control is significant enough to
   surface in the priority queue
8. If the control changes NixOS behavior, add or update a VM test
   under `tests/` to verify
9. Run `just check` to verify the change doesn't break anything

You can automate steps 1–2 by running `/compliance-audit cis-level1`
in Claude Code and asking the `compliance-auditor` agent to generate a
skeleton for a specific control ID.

## How to check coverage

```
just compliance-status
```

...shows the list of implemented compliance modules. For a full gap
analysis against a target framework, use:

```
/compliance-audit cis-level1
```

(or `cis-level2`, `stig`, `nist-800-53`, `soc2`) in Claude Code.

## Caveats and honest limitations

- **This is not a certification.** We self-attest to control
  implementation. Formal compliance (e.g., FedRAMP, SOC 2 Type II)
  requires a qualified external auditor and evidence collection that
  goes beyond configuration inspection.
- **Process controls are not tracked here.** NIST 800-53 families like
  CP (contingency planning), IR (incident response), and PS (personnel
  security) require organizational documentation, not code.
- **Compliance is a floor, not a ceiling.** Hearth's threat model
  includes risks that aren't enumerated in any standard benchmark.
  See `docs/threat-model.md` for the full picture.
- **Fleet operators must still make choices.** A Hearth deployment is
  only as compliant as the profile it enables and the hardening level
  it runs. Shipping a compliance module doesn't enforce it — the
  operator must opt in.
