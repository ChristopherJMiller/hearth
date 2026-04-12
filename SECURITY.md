# Security Policy

Hearth manages enterprise desktop fleets. Security is core to the project's
value proposition, and we take vulnerability reports seriously.

## Scope

**In scope:**

- `hearth-api` — the control plane REST API
- `hearth-agent` — on-device agent
- `hearth-build-worker` — build orchestrator
- `hearth-enrollment` — device enrollment TUI and flow
- `hearth-greeter` — GTK4 greeter
- NixOS modules under `modules/` (including hardening, compliance, PAM,
  enrollment, secure-boot, TPM-FDE)
- The Helm chart at `chart/hearth-home/`
- The web console at `web/apps/hearth/` and shared UI at
  `web/packages/ui/`
- Database schema and migrations at `migrations/`

**Out of scope:**

- Upstream dependencies: Kanidm, Headscale, Attic, nixpkgs, PostgreSQL,
  Synapse, Nextcloud. Report those to their respective projects.
- Misconfiguration by operators (e.g., deploying with
  `KANIDM_OIDC_ISSUER` unset or running without TLS).
- Denial-of-service via resource exhaustion that requires privileged
  access (authenticated fleet-admin flooding the build queue).
- Issues in development tooling (justfile, dev shell, flake inputs)
  that do not affect a production deployment.

## Supported versions

We support the latest tagged release and the previous minor release for
security fixes. Older versions receive best-effort fixes only.

## Reporting a vulnerability

**Do not open a public issue.** Instead:

1. Email the project maintainers (see `CODEOWNERS` or the repository
   metadata for current contacts).
2. Include:
   - A description of the vulnerability and its impact
   - Reproduction steps or a proof of concept
   - Affected versions / commits
   - Your disclosure timeline preference

We aim to acknowledge reports within **5 business days** and to publish
a fix or mitigation within **90 days** of the initial report. We'll
credit reporters in the release notes unless they prefer to remain
anonymous.

## Disclosure timeline

- **Day 0** — Report received, acknowledged within 5 business days
- **Days 1–30** — Triage, reproduction, severity assessment
- **Days 30–75** — Fix development, testing, coordinated disclosure
  with downstream packagers if needed
- **Day 90** — Public disclosure via a GitHub Security Advisory and a
  release

If a fix is trivial, we'll ship sooner. If the issue is complex or
requires coordination across multiple projects, we may request an
extension.

## Security architecture at a glance

Hearth has defense in depth at several layers:

- **Identity:** Kanidm OIDC with ES256/RS256 JWT validation against
  JWKS, group-based RBAC (`hearth-admins`, `hearth-operators`,
  `hearth-users`) via 5 Axum extractors (`UserIdentity`,
  `MachineIdentity`, `OptionalIdentity`, `OperatorIdentity`,
  `AdminIdentity`).
- **Device trust:** Machine tokens (HS256) minted at enrollment
  approval, SHA-256 hash stored in DB for revocation, 90-day validity.
- **Transport:** Headscale WireGuard mesh between agents and control
  plane; cert-manager TLS on the Kubernetes ingress.
- **Device hardening:** `modules/hardening.nix` (standard/strict
  levels), `modules/compliance/` (CIS + STIG controls), TPM-backed FDE
  (`modules/tpm-fde.nix`), Secure Boot via Lanzaboote
  (`modules/secure-boot.nix`).
- **Integrity:** NixOS store is hash-addressed; closures are signed and
  fetched from the Attic binary cache; CycloneDX SBOMs are tracked per
  deployment.
- **Audit:** Immutable `audit_events` table; Linux audit rules enabled
  by hardening module.
- **SQL safety:** sqlx compile-time checked queries (SQL injection
  impossible by construction in checked paths).

See `docs/threat-model.md` for the full threat analysis and
`docs/compliance-registry.md` for compliance framework coverage.

## Known limitations

The following are documented accepted risks. They are tracked and will
be improved over time:

- No rate limiting middleware on the API
- CORS is wide open pending a restricted allowlist
- Machine tokens do not rotate (90-day expiry)
- The enrollment status polling endpoint is intentionally
  unauthenticated
- Helm chart deployments currently lack pod-level `securityContext`
- Dev mode (`KANIDM_OIDC_ISSUER` unset) disables auth — production
  deployments must set this

## Tooling

The repository ships Claude Code agents and slash commands to help
developers keep Hearth secure as they build:

- `/security-review` — review code changes for vulnerabilities
- `/compliance-audit` — audit against CIS / STIG / NIST / SOC 2
- `/threat-model` — STRIDE analysis for components and data flows
- `/hardening-check` — infrastructure hardening audit

See `.claude/agents/` and `.claude/commands/` for the definitions and
`CLAUDE.md` for when to use each.
