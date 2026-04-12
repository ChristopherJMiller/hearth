# Hearth Threat Model

This document captures the threat surface, trust boundaries, and known
accepted risks of the Hearth control plane and fleet.

It is a **living document**. When a component changes in a way that
affects security, update the relevant section. Use the `/threat-model
<component>` slash command to have the `threat-model` agent generate
fresh STRIDE findings for a component.

## System overview

Hearth is a control plane for a fleet of NixOS desktop workstations. It
has four major subsystems:

1. **Control plane** (`hearth-api`) — Axum REST API on port 3000 with
   PostgreSQL storage. Enforces auth, stores state, orchestrates
   deployments.
2. **Fleet devices** (`hearth-agent`) — on-device systemd service that
   polls the control plane, reports heartbeats, and applies target
   state.
3. **Build infrastructure** (`hearth-build-worker`, Attic cache) —
   evaluates NixOS flakes, builds closures, pushes to the binary cache.
4. **Identity & mesh** — Kanidm provides OIDC identity; Headscale
   provides a WireGuard mesh between agents and the control plane.

```
                          +-------------------+
                          |   Kanidm (OIDC)   |
                          +---------+---------+
                                    |
                         JWKS/OIDC  |
                                    v
  hearth-agent   <---mesh TLS---+---TLS---> Web SPA (React)
    (fleet                      |              ^
     device)                    |              |
       |                        v              |
       |           +------>  hearth-api <------+
       |           |            |
       |           |            v
       |           |       PostgreSQL
       |           |            ^
       |           |            |
       |           +---  hearth-build-worker
       |                        |
       |                        v
       +--------------->  Attic binary cache
                                ^
                                |
                         Headscale mesh
```

## Trust boundaries

| Boundary | Inside (trusted) | Outside (untrusted) | Control |
|----------|------------------|---------------------|---------|
| Device enrollment | Fleet admins | Pre-enrollment device | Hardware fingerprint + operator approval |
| Fleet device ↔ API | Control plane | Post-enrollment device | Machine token (HS256, SHA-256 hash in DB) + mesh TLS |
| API ↔ Kanidm | Control plane | Identity provider | JWKS with ES256/RS256 signature verification |
| API ↔ PostgreSQL | Control plane | Database | Single DB user, k8s NetworkPolicy (to be added) |
| Build worker ↔ Attic | Build infra | Fleet devices | Signed closures, Attic auth token |
| Web SPA ↔ API | Browser user | API | OIDC id_token, CORS (currently permissive) |
| Operator ↔ Kanidm | Operators | Everyone else | Kanidm credentials + MFA (operator responsibility) |

## Data classification

| Classification | Data |
|----------------|------|
| **Confidential** | Machine tokens, user OIDC tokens, Kanidm credentials, Attic cache signing keys, PostgreSQL password, HEARTH_MACHINE_TOKEN_SECRET, HEARTH_ATTIC_TOKEN_SECRET |
| **PII** | Usernames, emails, display names, hardware fingerprints, device hostnames, user environment closures |
| **Integrity-critical** | NixOS target closures, compliance policies, deployment plans, audit log entries, SBOMs |
| **Availability-critical** | Heartbeat endpoint, target-state endpoint, Attic cache, build queue |
| **Public** | Release artifacts, public OCI images |

## STRIDE analysis — highlights

The per-component analyses live in this section. Use `/threat-model
<component>` to regenerate or extend.

### Control plane API

**Existing mitigations:**
- All write endpoints use `AdminIdentity` or `OperatorIdentity` extractors
- sqlx compile-time SQL checking eliminates injection in all checked
  queries
- Error responses sanitized via `AppError`
- Immutable audit log with actor, machine_id, and details

**Top threats:**

#### THR-API-001: Unauthenticated enrollment-status polling enables device enumeration and token-mint pressure
- STRIDE: Information Disclosure, Denial of Service
- Component: `crates/hearth-api/src/routes/enrollment.rs` (enrollment_status handler)
- Description: The endpoint is intentionally unauthenticated to support
  device polling during the enrollment flow. An unauthenticated attacker
  can enumerate machine UUIDs and force repeated token-mint attempts.
- Likelihood: Medium (requires knowing/guessing a machine UUID, but
  UUIDs may appear in logs or enrollment QR codes)
- Impact: Low (read-only; tokens only mint for already-approved devices)
- Severity: **Medium**
- Accepted risk with mitigation plan: add rate limiting (THR-API-002)
  and random delay on repeated queries for the same UUID.

#### THR-API-002: No rate limiting on any endpoint
- STRIDE: Denial of Service
- Component: Entire API, no `tower::limit` or `governor` middleware
- Description: Any endpoint can be flooded. Heartbeat and enrollment
  endpoints are the highest-value targets for an unauthenticated
  attacker; authenticated operator endpoints are exposed to insider DoS.
- Likelihood: High (network access only)
- Impact: Medium (operational only, data is not compromised)
- Severity: **High**
- Recommended mitigation: add `tower::limit::RateLimitLayer` with
  per-route configuration; `tower_governor::GovernorLayer` is a more
  expressive option. Per-IP limits on unauthenticated endpoints, per-token
  limits on authenticated ones. Maps to NIST **SC-5**.

#### THR-API-003: Wide-open CORS enables cross-origin API abuse
- STRIDE: Elevation of Privilege (via CSRF-like cross-origin calls with
  stolen id_token)
- Component: `crates/hearth-api/src/lib.rs` (CorsLayer::new with Any)
- Description: The API currently sets `allow_origin(Any)`,
  `allow_methods(Any)`, `allow_headers(Any)`. A malicious site visited by
  an authenticated operator could issue cross-origin requests carrying
  the user's id_token.
- Likelihood: Low (requires the user to visit a malicious page with an
  active session and the attacker to know how to extract the id_token,
  which is in sessionStorage)
- Impact: High (any admin action the operator can perform)
- Severity: **High**
- Recommended mitigation: restrict `allow_origin` to the operator
  console origin(s) configured via `HEARTH_WEB_ORIGIN` env var. Maps to
  CIS and NIST **AC-4**.

#### THR-API-004: Machine tokens do not rotate
- STRIDE: Elevation of Privilege (via stolen credential)
- Component: `crates/hearth-api/src/auth.rs` (machine token minting, 90d
  validity)
- Description: A compromised device keeps its machine token for up to
  90 days. There is a revocation path (SHA-256 hash in DB) but no
  automatic rotation, so normal operation does not limit the exposure
  window.
- Likelihood: Low (requires device compromise)
- Impact: Medium (compromised device is already compromised, but the
  token extends trust beyond the compromise window)
- Severity: **Medium**
- Recommended mitigation: add token refresh via heartbeat response;
  rotate at 25% of remaining lifetime. Maps to NIST **IA-5(1)**.

### Fleet device agent

**Existing mitigations:**
- systemd service confinement (when present in agent module)
- Machine token stored at `/var/lib/hearth/machine-token` with
  restricted permissions
- IPC Unix socket limited to greeter communication

**Top threats:**

#### THR-AGENT-001: `run_command` action provides arbitrary command execution
- STRIDE: Elevation of Privilege, Tampering
- Component: `crates/hearth-agent/src/` action handlers
- Description: Fleet admins can send `run_command` actions that execute
  arbitrary shell commands on devices. This is the intended design but
  has a large blast radius: an operator account compromise translates
  directly to fleet-wide RCE.
- Likelihood: Low (requires admin-level API access)
- Impact: High (fleet-wide)
- Severity: **High** — but inherent to the use case
- Mitigation: strong auth on action-create endpoint (`AdminIdentity`),
  command allowlist option for fleet operators who want to lock down,
  audit logging of every action. The allowlist is not yet implemented.
  Maps to NIST **AC-6(1)**.

### Enrollment flow

#### THR-ENROLL-001: Hardware fingerprint spoofing
- STRIDE: Spoofing
- Component: `crates/hearth-api/src/routes/enrollment.rs`
- Description: Hardware fingerprint is the hash of serial number + TPM
  EK cert + motherboard UUID. Without TPM attestation, a malicious
  device can submit any fingerprint.
- Likelihood: Low (requires physical or remote access + fingerprint
  knowledge)
- Impact: Medium (bogus device enters enrollment queue; still requires
  admin approval)
- Severity: **Low** — mitigated by admin approval step
- Future mitigation: TPM remote attestation using the EK certificate.
  Maps to NIST **IA-3**.

### Build pipeline

#### THR-BUILD-001: Attic cache signing key compromise delivers arbitrary code fleet-wide
- STRIDE: Tampering, Elevation of Privilege
- Component: Attic cache deployment (`chart/hearth-home/` Attic subchart)
- Description: All fleet devices trust the Attic cache signing key.
  Compromise of that key permits substitution of any closure with a
  malicious replacement.
- Likelihood: Low (requires compromise of the cache host or the signing
  key material)
- Impact: Critical (fleet-wide RCE as root)
- Severity: **Critical** — already highest-impact key in the system
- Existing mitigation: key stored in Kubernetes Secret with
  `existingSecret` pattern; Helm chart supports BYO secret management.
- Additional mitigation: hardware-backed key storage (HSM, cloud KMS),
  key rotation procedure, separate signing key per environment. Maps to
  NIST **SC-12**, **SC-13**.

### Helm chart / Kubernetes deployment

#### THR-K8S-001: No pod-level securityContext on any workload
- STRIDE: Elevation of Privilege (via container escape)
- Component: All templates under `chart/hearth-home/templates/`
- Description: Every Deployment, StatefulSet, and Job in the chart
  runs with default Kubernetes security defaults — no
  `runAsNonRoot`, `readOnlyRootFilesystem`, dropped capabilities, or
  seccomp profile. The `busybox:1.36` init container in
  `templates/api/deployment.yaml` runs as root.
- Likelihood: Medium (container breakout is a common attack path after
  initial compromise)
- Impact: High (node compromise → cluster compromise)
- Severity: **High**
- Recommended mitigation: add pod and container securityContext to
  every workload with `runAsNonRoot: true`, `readOnlyRootFilesystem:
  true`, `allowPrivilegeEscalation: false`, `capabilities.drop:
  [ALL]`, `seccompProfile.type: RuntimeDefault`. Label the namespace
  with `pod-security.kubernetes.io/enforce: restricted`. Maps to CIS
  Kubernetes **5.2.1–5.2.6** and NIST **SC-39**.

#### THR-K8S-002: No NetworkPolicy resources
- STRIDE: Information Disclosure, Elevation of Privilege
- Component: `chart/hearth-home/` (no NetworkPolicy templates exist)
- Description: All pods can talk to all other pods in the namespace and
  cluster. A compromised observability pod can reach the API and
  PostgreSQL; a compromised chat pod can exfiltrate user environments.
- Likelihood: Medium
- Impact: High
- Severity: **High**
- Recommended mitigation: default-deny NetworkPolicy plus per-service
  allow rules. Maps to CIS Kubernetes **5.3.2** and NIST **SC-7**.

### Web frontend

#### THR-WEB-001: id_token (not access_token) sent as bearer
- STRIDE: Information Disclosure
- Component: `web/apps/hearth/src/auth.ts`
- Description: The frontend sends the OIDC id_token as the bearer
  token. This is intentional (Kanidm access tokens lack rich claims)
  but non-standard. An id_token leak exposes user claims; access_token
  leak exposes capabilities. id_token in a bearer header flows through
  logs and proxies differently than an access_token.
- Likelihood: Low (requires token exfiltration from sessionStorage or
  HTTPS compromise)
- Impact: Medium
- Severity: **Low** — accepted pending Kanidm support for rich access
  tokens.
- Maps to NIST **IA-2**, **SC-8**.

## Accepted risks (summary)

These are known and documented. The mitigation plan for each is either
tracked in `docs/compliance-registry.md` or called out above.

| ID | Risk | Severity | Plan |
|----|------|----------|------|
| AR-01 | CORS wide open (`allow_origin(Any)`) | High | Restrict via env var |
| AR-02 | No rate limiting middleware | High | Add `tower_governor` |
| AR-03 | Enrollment status endpoint unauthenticated | Medium | Rate limit + random delay |
| AR-04 | `danger_accept_invalid_certs` in 3 locations | Medium | Gate behind `HEARTH_DEV_MODE` env var |
| AR-05 | No Helm pod securityContext | High | Add to all templates |
| AR-06 | No NetworkPolicy resources | High | Add default-deny + per-service rules |
| AR-07 | Machine tokens don't rotate | Medium | Add refresh via heartbeat response |
| AR-08 | id_token used as bearer | Low | Wait for Kanidm access_token claims |
| AR-09 | No request body size limits | Medium | Add `DefaultBodyLimit` per route |
| AR-10 | `run_command` action has no allowlist | High (inherent) | Add optional allowlist configuration |

## Top 3 priorities

1. **Add pod securityContext and NetworkPolicy to the Helm chart**
   (AR-05, AR-06) — the largest attack-surface reduction for the
   smallest change
2. **Add rate limiting middleware** (AR-02) — closes the
   unauthenticated DoS path and reinforces AR-03
3. **Restrict CORS** (AR-01) — closes cross-origin abuse path against
   authenticated operators

## How to update this document

- When adding a new route, handler, or module that crosses a trust
  boundary, run `/threat-model <component>` and append findings here
- When fixing an accepted risk, move its row from the AR table into a
  "Resolved" section and cite the PR that fixed it
- When introducing a new accepted risk, add it to the AR table with an
  explicit plan and owner
- Review the full document at least once per release
