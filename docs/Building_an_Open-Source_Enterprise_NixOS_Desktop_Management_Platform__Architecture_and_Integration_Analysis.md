# Building an open-source enterprise NixOS desktop management platform

**No complete enterprise NixOS desktop management platform exists today, but the ecosystem provides surprisingly strong primitives to build one.** NixOS's declarative configuration, hash-verified reproducibility, and atomic rollbacks give it inherent advantages over traditional Linux in enterprise fleet management — the missing piece is the management plane that ties everything together. This report surveys every architectural component needed, evaluates 80+ open-source projects across 9 domains, and identifies exactly what must be built from scratch versus assembled from existing parts.

The most actionable finding: a viable platform can be constructed by combining **Kanidm** (identity), **Clan's deployment primitives**, **Fleet/osquery** (observability), **Crystal Forge's compliance vision**, **Attic** (binary cache), **sops-nix** (secrets), and a custom web console built with **Refine + Tremor** — all orchestrated through a REST API modeled after FleetDM's design. The critical blockers are home-manager's inability to work with directory-authenticated users (issue #5244), the absence of SCAP/STIG tooling for NixOS, and the need to build an enrollment and management protocol from scratch.

---

## Identity and directory services: Kanidm leads, but Kerberos remains a gap

Five open-source LDAP-capable servers were evaluated as the identity backbone. **Kanidm** emerges as the strongest choice for a NixOS-native platform, while **FreeIPA** remains the only option providing a complete integrated identity stack.

**Kanidm** (Rust, ~4,600 GitHub stars) offers the best NixOS integration of any identity server. Its NixOS module at `services.kanidm` supports declarative provisioning of users, groups, and OAuth2 clients — a capability no other identity server matches. It natively provides **LDAP** (read-only gateway), **OAuth2/OIDC**, **RADIUS**, **SSH key distribution**, and **Unix PAM/NSS integration** via `kanidm-unixd` with TPM-protected offline caching. Its full JSON REST API at `/v1/` enables automation, and it benchmarks at **3× faster searches** than FreeIPA. The critical gap: **no Kerberos support**. The Kanidm team has started `libkrimes` (a Rust Kerberos library) signaling future intent, but no timeline exists. Environments requiring NFSv4 `sec=krb5` or Kerberos-authenticated SMB cannot use Kanidm alone.

**LLDAP** (~5,700 stars) provides the **best administrative web UI** of any option — full user/group CRUD, custom attributes, password reset — plus a GraphQL API and Terraform provider. However, it's intentionally simplified: no OAuth2/OIDC, RADIUS, or Kerberos. It requires pairing with Authelia or Keycloak for modern authentication protocols. Its NixOS module at `services.lldap` is complete.

**FreeIPA** bundles 389 Directory Server, MIT Kerberos, BIND DNS, Dogtag CA, and SSSD into a single installer with a comprehensive web UI. It's the only solution providing integrated LDAP + Kerberos + DNS + CA out of the box. However, the **FreeIPA server cannot be packaged natively on NixOS** (nixpkgs issue #265754) — only the client module (`services.ipa`) exists. The workaround is running the server in a Podman/Docker container with NixOS hosts enrolled as clients.

**OpenLDAP** (v2.6.12 LTS) has a full NixOS module with declarative configuration support, but requires building everything else (web UI, OAuth2, Kerberos integration) from scratch. **389 Directory Server** is packaged in nixpkgs but has **no NixOS service module**. **GLAuth** is too lightweight for enterprise use.

For Kerberos on NixOS, a conflict exists between MIT Kerberos (`security.krb5`) and Heimdal (`services.kerberos_server`) — their tools produce incompatible keytab formats when both are enabled. The recommended approach is to pick one implementation consistently.

| Server | Maturity | NixOS Module | Protocols | Web UI | REST API |
|--------|----------|-------------|-----------|--------|----------|
| **Kanidm** | 3/5 | ✅ Full + provisioning | LDAP, OAuth2, OIDC, RADIUS | User portal only | ✅ Full |
| **LLDAP** | 3/5 | ✅ Full | LDAP only | ✅ Best admin UI | GraphQL |
| **FreeIPA** | 5/5 | ⚠️ Client only | LDAP, Kerberos, DNS, CA | ✅ Comprehensive | JSON-RPC |
| **OpenLDAP** | 5/5 | ✅ Full | LDAP only | ❌ None | ❌ None |
| **389-DS** | 4/5 | ❌ Package only | LDAP only | ⚠️ Partial Cockpit | ❌ None |

The recommended identity architecture: use **Kanidm as the primary identity server** with its NixOS declarative provisioning, mapping Kanidm groups to NixOS/home-manager profiles. If Kerberos is a hard requirement, run **FreeIPA in a container** alongside or deploy a separate MIT Kerberos KDC backed by Kanidm's LDAP gateway.

---

## The management console must be built, but strong patterns exist

No existing open-source management console meets enterprise NixOS fleet management needs. However, three categories of existing tools provide essential patterns and components.

**Fleet (fleetdm.com)** is the strongest architectural analog. This open-source device management platform (~3,500 stars, MIT license) uses osquery to manage macOS, Windows, Linux, ChromeOS, and mobile devices at **400,000+ endpoint scale**. Its REST API design (`/api/v1/fleet/hosts`, `/policies`, `/queries`, `/software`), GitOps workflow (`fleetctl apply` with YAML definitions), and policy engine (SQL-based boolean compliance checks) provide the blueprint for a NixOS management API. Fleet handles observability and compliance checking; NixOS provides the declarative configuration layer that Fleet lacks. **Fleet + osquery should be integrated as the observability/compliance component** of the platform, not replaced.

**Clan** (clan.lol, backed by Numtide members including @Mic92) provides the most mature NixOS-specific deployment primitives: machine installation via `nixos-anywhere`, fleet updates, secrets management via sops-nix integration, overlay networking (ZeroTier/WireGuard), hardware detection via NixOS Facter, and recently added macOS/nix-darwin support. Its Git-based declarative approach and `clan machines install/update` CLI directly address fleet lifecycle management. But Clan is **CLI-only with no web UI**, no RBAC, no audit logging, and no compliance features — it's infrastructure, not a management plane.

**Crystal Forge** (announced November 2025, solo developer) has the right compliance vision — mapping STIG controls to NixOS modules, CVE scanning via Vulnix, OSCAL output for accreditation metadata — but is a very early proof-of-concept. Its architecture (Rust, `nix-eval-jobs` for parallel evaluation, `systemd-run` for resource-limited builds) demonstrates the right technical approach for compliance-first NixOS fleet management.

For the enterprise management "gold standard," Red Hat Satellite, SUSE Manager, and Canonical Landscape share a common feature set: **patch management** with staged rollouts, **inventory/asset management** with dynamic grouping, **compliance scanning** (OpenSCAP), **configuration management** with drift detection, **remote execution**, **reporting dashboards**, **RBAC with multi-tenancy**, and **REST APIs**. Any enterprise NixOS platform must match these capabilities.

For building the custom web console, **Refine** (~29,000 stars) paired with **Tremor** dashboard components is recommended. Refine's headless hooks-based architecture allows building NixOS-specific views (flakes, derivations, generations, profiles) without being constrained by generic CRUD patterns. It provides free RBAC, real-time updates, and SSR support. Tremor (acquired by Vercel) provides 35+ copy-paste dashboard visualization components for fleet health monitoring. **React Admin** (~26,000 stars) is a mature alternative if speed-to-prototype is prioritized.

The control plane API should be **REST-first** (matching FleetDM, Foreman, and Satellite patterns), with optional GraphQL for the dashboard. Core data models needed: Machine (hostname, NixOS version, current generation, system closure path, flake URI), NixConfiguration (flake rev, closure hash, build/deploy timestamps), Deployment (strategy, status, rollback target), Policy (compliance query, severity, auto-remediation), and User (role, team, permissions).

---

## Fleet deployment architecture: pull-based GitOps with central builds

Six NixOS deployment tools were evaluated. The recommended architecture combines **comin** (pull-based GitOps agent on each machine), **buildbot-nix** (CI/CD), and **Attic** (binary cache), with **Colmena** as a fallback for ad-hoc push deployments.

**Colmena** (~2,000+ stars, Rust) is the most capable push-based tool. Its three-phase pipeline (evaluate → build → deploy) supports parallel deployment across nodes via Tokio async runtime, tag-based node filtering (`--on @production`), and both local and remote builds. Configuration uses a `colmenaHive` flake output. Colmena is stateless and can be scripted, but its push-over-SSH model doesn't scale past ~500 machines due to connection limits.

**deploy-rs** (Serokell, ~1,300 stars) differentiates through **magic rollback**: after activation, a canary file must be confirmed within 30 seconds or the target automatically reverts. This safety mechanism is unmatched. It also uniquely supports deploying non-system profiles (home-manager, app-specific) to non-root users.

**comin** (Go) is the most important tool for fleet-scale GitOps. Running as a systemd service on each machine, it continuously polls Git remotes, evaluates `nixosConfigurations.<hostname>`, builds locally, and switches configurations. Key features include multi-branch support (a `testing` branch deploys with `nixos-rebuild test` — reboot reverts to the safe `main` branch) and multi-remote support. This is the closest NixOS equivalent to **ArgoCD-style continuous reconciliation**.

**Bento** uses an SFTP-based pull model where a central server builds all configurations and publishes them to per-host SFTP chroot directories. Privacy-first (each client only accesses its own files) and firewall-friendly. Best for small workstation fleets.

The built-in `system.autoUpgrade` module provides primitive GitOps via a systemd timer that runs `nixos-rebuild switch --flake <uri>` on a schedule, but lacks comin's testing branch support, post-deploy hooks, and multi-remote capability.

**NixOps** is explicitly in "low-maintenance mode" and not recommended for new projects.

For scaling past 200 machines, **pull-based deployment becomes essential**. The architecture separates expensive evaluation/building from deployment: CI builds all machine configurations in parallel using `nix-eval-jobs`, pushes artifacts to a binary cache, and machines independently pull and apply updates. At 1,000+ machines, webhook-triggered rebuilds replace polling to reduce Git server load, and staggered rollouts with randomized delays prevent thundering herds.

**Determinate Systems' FlakeHub** offers a compelling enterprise feature: **pre-evaluated store paths**. `fh apply nixos` can deploy NixOS configurations without Nix evaluation on the target device — FlakeHub resolves store paths server-side. This eliminates the 3-15 second per-machine evaluation cost and enables deployment to resource-constrained devices. FlakeHub Cache uses JWT-based authentication (no static tokens) with per-flake access control, and is **SOC2 compliant**. However, it creates vendor lock-in and requires paid plans for private flakes and caching.

For traditional config management integration, the **DebOps nixos role** provides the most mature Ansible-NixOS integration (pushing Nix configs via Ansible, running `nixos-rebuild`), but the NixOS community generally views NixOS modules as a replacement for Puppet/Salt/Ansible rather than a complement.

---

## Home-manager with LDAP users is the hardest unsolved problem

**Home-manager issue #5244 is a critical blocker** with no upstream fix. When home-manager is used as a NixOS module, it creates entries in `users.users.<name>` for each managed user. For LDAP/AD/Kanidm users that exist only at runtime via NSS, this triggers NixOS assertions requiring `isNormalUser` or `isSystemUser` to be set. Additionally, home-manager creates systemd services that run at boot time before LDAP users have logged in or home directories exist.

No patch, fork, or workaround fully resolves this. The three current approaches are: (1) use standalone home-manager instead of the NixOS module (activation must be triggered separately), (2) define stub users in `users.users` matching LDAP UIDs (defeats centralized identity management), or (3) fork home-manager's NixOS module to remove the `users.users` dependency (no public fork exists).

The proposed architecture for a "user environment service" combines **pre-built group-based profiles** with **PAM/systemd-triggered activation**:

At NixOS build time, a fixed set of home-manager profiles are built — one per LDAP group role (developer, designer, default). These profiles live in the Nix store as part of the system closure. When a user logs in, a `systemd --user` service (preferred over PAM for reliability and logging) queries SSSD for group membership, selects the matching pre-built profile, and runs the activation script to create symlinks in the user's home directory.

The critical technical challenge is **username parameterization**: home-manager activation scripts contain hardcoded paths based on `home.username` and `home.homeDirectory`. A profile pre-built for user "template" won't work for user "jdoe" without either (a) running `home-manager switch` per-user at login time (10-60 seconds of latency), or (b) forking the activation logic to accept runtime username/homedir substitution (~1-2 seconds). Option (b) requires custom development but provides acceptable login performance.

**SSSD integration with NixOS is functional.** The `services.sssd` module provides full AD/LDAP client configuration, and the `services.ipa` module handles FreeIPA client enrollment. Known pain points include: LDAP `loginShell` paths like `/bin/bash` not existing on NixOS (fix: `override_shell = /run/current-system/sw/bin/bash`), local group membership not supporting AD group nesting (users must be added individually to `wheel`, `networkmanager`, etc.), and nscd caching conflicts with SSSD (fix: disable passwd/group caching).

**Five components must be built from scratch** for enterprise LDAP user integration: a home-manager patch to skip `users.users` creation, username parameterization for pre-built profiles, a group-to-profile resolver service, profile cache management for garbage collection, and offline activation support using SSSD credential caching.

---

## Compliance is NixOS's biggest enterprise opportunity and biggest gap

NixOS has **zero existing SCAP, STIG, or CIS benchmark support** — no profiles, no scanning tools, no compliance frameworks. OpenSCAP's OVAL definitions are deeply tied to FHS paths and rpm/dpkg package queries that don't exist on NixOS. DISA has published no STIG for NixOS. CIS has published no NixOS benchmark (an issue has been open since 2015).

Yet NixOS's declarative model provides **fundamental advantages** that could make compliance easier than on any traditional distribution. Configuration drift is architecturally impossible — the system state is defined by code. Every system state maps to a Git commit plus a Nix derivation hash. Compliance requirements become NixOS modules that can be version-controlled, reviewed, and tested. A running system can be verified against its declared configuration by comparing `/run/current-system` store paths. Atomic generation switching enforces all controls simultaneously.

**Vulnix** (nix-community) scans Nix derivation closures against the NIST NVD, but suffers from a high false-positive rate due to coarse name/version heuristic matching. The NixOS vulnerability roundups that used Vulnix were discontinued for this reason. **SBOM-based scanning** is the emerging alternative: tools like sbomnix and bombon generate SBOMs from Nix derivations that can be fed to standard scanners (Grype, Trivy). A Nixpkgs SBOM team is forming to improve metadata quality.

**Lanzaboote** (nix-community, **v1.0.0 released**) implements Secure Boot for NixOS using a custom Rust UEFI stub. It signs boot files, creates Unified Kernel Images, and validates kernel signatures via UEFI `LoadImage`. Its "thin" variant deduplicates kernels and initrds on the ESP — essential since standard UKIs would exhaust ESP space with NixOS's many generations. Lanzaboote is **production-ready for single machines** and widely used in the community, but fleet deployment requires planned PKCS#11/HSM key management improvements.

**Impermanence** (nix-community) inverts the persistence model: the root filesystem is wiped on every reboot (via tmpfs, BTRFS subvolume deletion, or ZFS snapshot rollback), and only explicitly declared paths persist. This eliminates malware persistence, unauthorized changes, and state accumulation. Every piece of persistent state becomes a declarative, auditable specification — powerful for compliance mapping.

**SELinux on NixOS** remains experimental. RFC #0041 has been stalled since 2018. Recent progress (April 2025) by Tristan Ross achieved `sestatus` reporting SELinux enabled, but fundamental challenges persist: the read-only Nix store conflicts with SELinux file labeling, and non-standard paths break standard policies. **AppArmor** is more viable — `security.apparmor.enable = true` works, NixOS 25.05 added improved policy management — but only ~4 service profiles ship by default.

The NixOS **hardened profile** (`profiles/hardened.nix`) configures the `linux_hardened` kernel, Scudo allocator, kernel module locking, SMT disabling, page table isolation, and AppArmor. However, a **proposal to deprecate it** was submitted (December 2024) due to unexpected breakage and difficulty managing expectations. The community alternative **nix-mineral** (alpha) provides more comprehensive hardening drawing from multiple security guides.

---

## Binary cache architecture centers on Attic for self-hosted enterprise

**Attic** (~3,900 stars, Rust) is the recommended self-hosted binary cache. Its multi-tenant architecture uses content-addressed NAR and Chunk stores for **global deduplication** — identical packages shared across tenants are stored once. Server-side managed signing means push clients never see private keys. JWT-based authentication supports per-cache scoped permissions with wildcard patterns. Storage backends include S3-compatible services (AWS, MinIO, R2) or local filesystem, with PostgreSQL for metadata.

**Harmonia** (nix-community, Rust) and **nix-serve-ng** (Arista Networks, Haskell) serve the local `/nix/store` over HTTP. nix-serve-ng is **20-30× faster** than the original nix-serve for lookups and fetches. Both are ideal for "serve my build server's store" but lack multi-tenancy, remote storage, and access control.

**Cachix** is the most mature hosted option with CDN-backed unlimited bandwidth, but exact pricing requires contacting sales. Its **Cachix Deploy** feature (agent-based deployment pulling from binary cache) makes it a fleet management competitor, not just caching infrastructure.

For CI/CD, **buildbot-nix** (nix-community, maintained by @Mic92) is the best fully self-hosted, open-source option. Built on battle-tested Buildbot, it uses `nix-eval-jobs` for parallel evaluation, supports GitHub/Gitea webhooks, and integrates with Attic/Cachix for cache pushing. **Garnix** offers the fastest managed CI (persistent `/nix/store`, **2-10× faster** than GitHub Actions) with zero configuration — `flake.nix` is the CI config.

A production enterprise architecture deploys: CI builds all machine configurations in parallel → pushes to Attic with separate tenant caches for `dev`/`staging`/`production` → Nginx reverse proxy with caching + TLS in front → fleet machines pull from cache. Regional Nginx proxy caches at each office site reduce central bandwidth. Storage sizing: **~200 GB cache storage** and **~1 TB build server disk** for a 100-desktop fleet, with 200-500 GB bandwidth per update cycle served from local cache (only 2-5 GB fetched from central).

---

## Monitoring leverages mature NixOS modules plus Fleet for endpoint telemetry

NixOS has **mature, first-class modules for the entire observability stack**: `services.prometheus` (60+ exporter modules), `services.grafana` (declarative provisioning), `services.loki`, `services.promtail`, `services.victoriametrics`, and `services.osquery`. The `node_exporter` module supports desktop-relevant collectors including `systemd`, `wifi`, `logind`, and `smartctl` for hardware health.

For intermittently connected laptops, **VictoriaMetrics vmagent** on each endpoint provides the best architecture — it scrapes local exporters and pushes via `remote_write` with **disk-backed buffers** that survive disconnections. When the laptop reconnects, buffered metrics flush automatically. VictoriaMetrics as the central TSDB offers ~7× less RAM usage than vanilla Prometheus with native push/pull support and multitenancy.

**NixOS configuration drift detection** is architecturally simpler than on any traditional system. The running system profile `/run/current-system` is a symlink to a specific store path. Compare this against the intended store path from `nixosConfigurations.<host>.config.system.build.toplevel` — if they differ, the machine hasn't applied the latest configuration. This single derivation hash comparison replaces the dozens of individual compliance signals that Intune and Jamf must check. A custom Prometheus textfile exporter can expose `nixos_config_drift{host="laptop1"} 1` for Grafana alerting.

**osquery + Fleet** should serve as the endpoint telemetry layer. osquery (v5.19.0 in nixpkgs, NixOS module at `services.osquery`) provides SQL-queryable access to system state, while Fleet provides centralized management, live queries, and vulnerability scanning. The gap: osquery's `packages` table doesn't understand the Nix store. A custom osquery extension or supplemental exporter is needed for NixOS-specific package inventory.

---

## Secrets management: sops-nix as primary, Vault for dynamic secrets

**sops-nix** (~2,600 stars, MIT, maintained by @Mic92) is the recommended primary secrets tool. It stores secrets encrypted in version-controlled YAML/JSON files using Mozilla SOPS, decrypts them during system activation to `/run/secrets/` on tmpfs (never in the Nix store unencrypted). It supports **age, GPG, AWS KMS, GCP KMS, Azure Key Vault**, and HashiCorp Vault as encryption backends. Its master-key architecture scales well: adding a new machine requires adding its age public key to `.sops.yaml` and running `sops updatekeys` once. Templating support, home-manager module, and nix-darwin support round out the feature set.

**agenix** (~2,100 stars) offers a simpler mental model (one `.age` file per secret, SSH host key as identity) at the cost of fewer features — no templating, no cloud KMS, and per-secret key listing that doesn't scale as cleanly. **ragenix** provides a Rust drop-in CLI replacement with better validation.

**HashiCorp Vault** complements sops-nix for dynamic secrets: database credentials, certificate management (PKI engine), and automatically rotated tokens. Two NixOS integration modules exist: `serokell/vault-secrets` (AppRole per-service) and `DeterminateSystems/nixos-vault-service` (Vault Agent sidecars). Note that Vault ≥1.15 uses BSL licensing; **OpenBao** (open-source fork) is an alternative.

The recommended combination: **sops-nix for static secrets** (API keys, passwords, config files — version-controlled, git-friendly, works offline) plus **Vault for dynamic secrets** (database creds, certificates, ephemeral tokens — auto-rotation, audit trail). SOPS itself supports Vault as a KMS backend, enabling Vault-managed master keys with sops-nix handling distribution.

Key gaps at fleet scale: no NixOS-native tool for atomic secret rotation across 1,000 machines (requires CI/CD pipeline wrapping), no standardized machine identity bootstrapping (how does a new machine get its first SSH host key?), and no secret access audit trail (sops-nix/agenix don't log access — Vault provides this natively).

---

## The commercial MDM feature gap is large but addressable

Comparing against **Microsoft Intune** (the gold standard), an enterprise NixOS platform must match capabilities across 14 categories: zero-touch enrollment, directory integration with conditional access, declarative configuration profiles, continuous compliance checking, software deployment with self-service catalog, staged update rollouts, disk encryption enforcement, hardware/software inventory, remote actions (wipe/lock/script), reporting dashboards, workflow automation, multi-tenancy with RBAC, REST APIs, and SIEM/ITSM integration.

NixOS's **existing strengths** already surpass traditional MDM in several areas: declarative configuration-as-code is more powerful than Intune's configuration profiles, hash-verified reproducibility beats any drift detection, atomic rollback surpasses Windows restore points, and the entire OS specification living in Git provides stronger audit trails than any MDM's logging.

The **critical gaps** that must be built from scratch:

- **MDM protocol/agent**: NixOS has no equivalent to Apple MDM or Windows OMA-DM. A custom agent using outbound-initiated connections (for NAT traversal) is needed — likely a systemd service communicating over HTTPS/gRPC with the control plane, using Tailscale/Headscale for mesh networking.
- **Zero-touch enrollment**: No self-service portal exists. Clan's barcode-scan enrollment is a start; `nixos-anywhere` handles remote installation. A web-based enrollment flow generating machine-specific configurations is needed.
- **Web management console**: Everything is CLI/Nix code today. The console must expose fleet status, deployment management, compliance dashboards, and user management to non-technical IT admins.
- **Conditional access**: No "block access if device is non-compliant" capability. This requires integrating compliance state with the identity provider (Kanidm + OAuth2 claims based on Fleet policy results).

**No immutable desktop Linux** (Fedora Silverblue, Vanilla OS, blendOS, Ubuntu Core, Endless OS) has solved enterprise management either — Ubuntu Core with Landscape comes closest. NixOS's fully declarative model is technically superior to all of them for fleet management; it just lacks the management tooling layer.

---

## Conclusion: a realistic integration architecture

The enterprise NixOS desktop management platform should be assembled in layers, building on existing mature components while developing the missing management plane:

The **identity layer** uses Kanidm with declarative NixOS provisioning, supplemented by a containerized FreeIPA server if Kerberos is required. **SSSD** on each desktop provides directory client integration with offline caching. The **deployment layer** combines comin as the on-device GitOps agent (pull-based, firewall-friendly) with Colmena for ad-hoc operations, backed by buildbot-nix for CI and Attic for binary caching. The **observability layer** integrates Fleet/osquery for endpoint telemetry and compliance queries alongside VictoriaMetrics/vmagent for system metrics with offline buffering. The **secrets layer** uses sops-nix for static secrets with Vault for dynamic credentials and PKI.

The **management plane** — the component that doesn't exist and must be built — is a REST API server (Go or Rust) with a Refine + Tremor web frontend, providing machine lifecycle management, deployment orchestration, policy definition, compliance reporting, RBAC, and audit logging. It consumes data from Fleet (device inventory, compliance), the binary cache (build status), comin (deployment state), and Kanidm (identity) to present a unified view.

The **five highest-priority development efforts** are: (1) patching home-manager to support LDAP/AD users without `users.users` dependencies, (2) building the REST API control plane with machine enrollment and fleet orchestration, (3) creating NixOS STIG/CIS compliance modules that leverage declarative configuration for provable compliance, (4) developing the web management console, and (5) implementing the user environment service for PAM-triggered home-manager profile activation. Together, these would close the gap between NixOS's powerful primitives and the enterprise management experience that organizations require.