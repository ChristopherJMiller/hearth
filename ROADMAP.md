# Hearth Roadmap

Hearth is an enterprise NixOS desktop fleet management platform. This roadmap tracks implementation progress from bootstrap through enterprise hardening.

## Architecture Overview

- **On-device Rust binaries:** hearth-agent, hearth-greeter, hearth-enrollment
- **Shared library:** hearth-common (IPC types, API client, config parsing, Nix store utils)
- **Control plane:** Axum REST API + PostgreSQL + build orchestrator
- **NixOS integration:** Modules, home-manager role profiles, overlays, mk-fleet-host
- **Binary cache:** Attic (multi-tenant, content-addressed dedup)
- **Identity:** Kanidm (primary), SSSD on fleet devices, FreeIPA fallback for Kerberos
- **Desktop:** GNOME + greetd + GTK4 greeter
- **Collaboration:** Matrix/Synapse (chat), Nextcloud (cloud/CalDAV/CardDAV), Stalwart (mail)
- **PIM:** Thunderbird (managed via policies + TbSync), GNOME Online Accounts (shell calendar/contacts)

## Key Technical Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Control plane language | Rust (Axum + Tonic) | Same language as device binaries, shared types |
| Agent ↔ control plane | REST initially, gRPC later | REST is simpler to debug; gRPC for push in Phase 2 |
| Local dev infra | docker-compose | Simpler than k3d for early dev |
| Rust builds in Nix | Crane | Two-phase cached builds, workspace-aware |
| Dev VMs | nixos-test + microvm.nix | Hermetic CI + fast interactive dev |
| Database | sqlx (compile-time checked SQL) | Async-native, raw SQL, de facto Axum standard |
| Object storage (prod) | Garage | S3-compatible, lightweight |
| Binary cache | Attic | Multi-tenant, content-addressed dedup |
| Identity | Kanidm | Rust-native, LDAP+OAuth2+RADIUS, NixOS modules |
| Desktop | GNOME + greetd | Declarative dconf, mature NixOS module, GTK4 greeter |

---

## Phase 0: Foundation (Bootstrap) {#phase-0}

Everything needed so parallel work can begin. No business logic yet.

### Tasks

- [x] **A: Cargo workspace** — Root `Cargo.toml`, 5 crate stubs (common, agent, greeter, enrollment, api), `.cargo/config.toml`
- [x] **B: Flake + dev shell + CI** — `flake.nix` (crane builds, dev shell, overlay, module outputs), `.github/workflows/ci.yml`, `.gitignore`, `rust-toolchain.toml`
- [x] **C: hearth-common types** — IPC types, config structs, API client trait + reqwest impl, API request/response types, Nix store path utils
- [x] **D: Schema + Docker** — `docker-compose.yml` (PostgreSQL + Attic), migrations (machines, user_environments, deployments, audit_events, software_catalog), hearth-api skeleton
- [x] **E: NixOS modules + HM profiles + VM harnesses** — `modules/`, `home-modules/`, `overlays/`, `lib/mk-fleet-host.nix`, `data/`, `tests/`, `dev/`

### Verification
- [x] `nix flake check` passes (all checks, packages, devShell, nixosModules, homeModules evaluate)
- [x] `cargo test --workspace` passes (all crates compile with mold linker)
- [ ] `docker-compose up` brings PostgreSQL online, `sqlx migrate run` applies all migrations
- [ ] `nix build .#hearth-agent` produces a store path

### Stats
- **Rust:** 5 crates, ~600 lines across hearth-common types, API skeleton, and binary stubs
- **Nix:** 23 files, ~3,600 lines — 7 NixOS modules, 5 HM profiles, overlay, lib helper, 5 test stubs, 2 dev VMs, branding assets
- **SQL:** 5 migrations with custom enums, indexes, foreign keys
- **Infra:** docker-compose (PostgreSQL + Attic), CI workflow (nix flake check + cargo check)

---

## Phase 1: Core Agent + Control Plane MVP {#phase-1}

The minimum viable loop: control plane knows about machines, agent polls it.

### Tasks

- [x] **Control plane:** Health endpoint, machine CRUD (GET/POST/PUT/DELETE), target-state endpoint, heartbeat receiver with sqlx repository layer
- [x] **hearth-agent:** Config loading from TOML, polling loop (configurable interval), heartbeat sender, Unix socket IPC server (Ping/Pong + PrepareUserEnv stub), system update comparator, graceful shutdown via CancellationToken
- [x] **NixOS modules:** agent.nix systemd service with config generation, desktop.nix GNOME baseline + dconf defaults, pam.nix greetd + SSSD PAM stack (completed in Phase 0)
- [ ] **Integration test:** NixOS VM test with two nodes (control plane + agent), agent registers and receives target closure

### Stats
- **hearth-api:** 8 source files — db.rs (models + sqlx::FromRow), repo.rs (6 query functions), routes/{machines,heartbeat,health}.rs, error.rs (AppError → IntoResponse), main.rs (AppState + router)
- **hearth-agent:** 5 source files — config.rs (TOML loading + CLI), poller.rs (poll loop), ipc.rs (Unix socket server), updater.rs (closure comparator), main.rs (orchestrator with signal handling)
- **Tests:** 10 passing (3 nix_store + 4 config/updater + 3 agent)

---

## Phase 1.5: Software Center Foundation {#phase-1-5}

Self-service catalog prioritized per user request.

### Three-Layer Model

1. **Nix system packages** (IT-managed) — handled by desktop.nix and role profiles
2. **Self-service catalog** (control plane managed) — curated approved software with approval workflow
3. **Flatpak self-service** (user-managed) — Flathub enabled, users install sandboxed apps freely

### Tasks

- [x] `software_catalog` table: name, category, install_method, flatpak_ref, nix_attr, approval_required, auto_approve_roles
- [x] `software_requests` table: approval workflow (pending → approved → installing → installed/failed)
- [x] API: catalog CRUD, `POST /catalog/{id}/request`, `POST /requests/{id}/approve`, `POST /requests/{id}/deny`, claim + result lifecycle
- [x] Agent handler: poll for pending installs via heartbeat, claim-before-execute, Flatpak install via `runuser`, report result
- [x] Web-based catalog page — migrated to Vite + React + TypeScript with pnpm workspace
- [x] `@hearth/ui` shared design system (tokens, components: Badge, Button, Card, StatusChip, FilterPills, SearchInput, Toast)
- [x] `@hearth/catalog` app with TanStack Query, typed API hooks, responsive card grid

### Stats
- **hearth-api:** 4 new route files (catalog.rs, requests.rs, web.rs, mod.rs updated), repo.rs (+12 query functions), db.rs (+5 types/enums)
- **Frontend:** pnpm workspace at `web/` with 2 packages (@hearth/ui shared design system, @hearth/catalog app). React 19, TypeScript, Vite 6, TanStack Query v5. Hearth dark theme with CSS custom properties.
- **hearth-agent:** installer.rs (Flatpak executor + stubs), poller.rs (install processing wired in)
- **hearth-common:** api_types.rs (+6 types), api_client.rs (+2 trait methods + impls)
- **Web:** static/catalog.html (production-quality dark-themed Software Center UI)

---

## Phase 2: Enrollment + User Environment Lifecycle {#phase-2}

### Tasks

- [x] **Control plane:** Enrollment endpoints (`POST /enroll`, `POST /machines/{id}/approve`, `GET /machines/{id}/enrollment-status`), user environment endpoints (`GET/PUT /machines/{id}/environments/{username}`, `POST .../login`), DB layer + repo functions
- [x] **hearth-enrollment:** Multi-screen ratatui TUI — welcome, hardware detection (CPU/RAM/disk/NIC via `/proc` + `lsblk` + `ip`), network check, server URL input + enrollment submission, approval polling with auto-retry
- [x] **hearth-agent:** Real `PrepareUserEnv` — role resolution from group mappings, home-manager activation via `runuser`, status reporting to control plane (`building` → `active`/`failed`), user login recording
- [x] **hearth-agent:** Offline resilience — SQLite-backed event queue (`rusqlite`), enqueue on API failure, drain-and-replay on reconnect, re-queue on replay failure
- [x] **hearth-common:** Enrollment request/response types, user env upsert types, `HearthApiClient` trait extended with `enroll`, `get_enrollment_status`, `report_user_env`, `report_user_login`; trait futures now `Send`-bounded for `tokio::spawn`
- [x] **hearth-common:** `AgentConfig` extended with `role_mapping` and `home_flake_ref` fields
- [x] **NixOS:** Home-manager profiles with real content, enrollment ISO image builder (`lib/mk-enrollment-image.nix` → `packages.enrollment-iso`), mk-fleet-host.nix refined with `homeFlakeRef` param + `extraConfig` fix, deprecated options fixed across modules
- [x] **Integration test:** VM tests wired into `nix flake check` (agent-polling, desktop-baseline), full-enrollment test expanded with API mock assertions + hardware tool checks

### Stats
- **hearth-api:** 2 new route files (enrollment.rs, environments.rs), repo.rs (+7 query functions), db.rs (+UserEnvStatusDb + UserEnvironmentRow), main.rs (+2 route groups)
- **hearth-enrollment:** Full rewrite — 8 source files (main.rs, app.rs, ui.rs, hw.rs, screens/{welcome,hardware,network,enroll,status}.rs), ~600 lines
- **hearth-agent:** 1 new file (queue.rs, ~115 lines), ipc.rs rewritten with real activation, poller.rs with queue integration, +rusqlite dependency
- **hearth-common:** api_types.rs (+4 types), api_client.rs (+4 trait methods + impls, `Send`-bounded futures), config.rs (+2 fields)
- **Nix:** New `lib/mk-enrollment-image.nix` (ISO builder), agent.nix +`homeFlakeRef` option, mk-fleet-host.nix +`homeFlakeRef`/`extraConfig` fix, deprecated options fixed in enrollment.nix/desktop.nix/pam.nix/greeter.nix, dconf moved to home-manager
- **Tests:** 12 Rust tests passing, 2 VM integration tests in `nix flake check` (agent-polling, desktop-baseline), full-enrollment expanded with 12+ assertions
- **Packages:** `enrollment-iso` builds a bootable NixOS ISO for device enrollment

---

## Phase 3: Greeter, Build Pipeline, Web Console {#phase-3}

### Tasks

- [x] **3A: hearth-greeter:** GTK4 fullscreen greeter with greetd IPC (length-prefixed JSON), agent Unix socket client (PrepareUserEnv/progress events), NSS group lookup, branding from `/etc/hearth/greeter.toml`, fallback session support
- [x] **3B: Deployment API + Agent Updater:** Migration 006 (deployment_machines table, machine_update_status enum, deployment columns), full deployment CRUD routes, per-machine status tracking, deployment counters, fleet stats endpoint, audit log endpoint. Agent updater rewritten with real `nix copy`/`nix-env --set`/`switch-to-configuration` pipeline, deployment status reporting via heartbeat
- [x] **3C: Web Console:** `@hearth/console` SPA (React 19, TanStack Router + Table + Query, Recharts) — multi-page admin dashboard with sidebar nav: fleet dashboard (stat cards + charts), machine list/detail, enrollment approval queue, deployment list/detail/create, catalog management, software request queue, audit log viewer. 10 new UI components in `@hearth/ui` (DataTable, StatCard, Sidebar, PageHeader, EmptyState, ConfirmDialog, ProgressBar, Tabs, Select, TextInput)
- [x] **3D: Build Orchestrator:** `nix-eval-jobs` evaluator (NDJSON streaming), parallel `nix build`, Attic cache push, per-machine config generator from DB inventory (role/tag/machine_id filtering), full orchestration pipeline (evaluate → build → push → create deployment → set target_closure)
- [x] **3E: Staged Rollouts:** Deployment FSM (pending → canary → rolling → completed, rollback/failed from any active state), batch health checker (heartbeat recency, failure rate), rolling batch controller (canary selection, batch advancement, threshold validation, rollback with closure restoration), background deployment monitor (30s poll, auto-advance canary/rolling, auto-rollback)

### Stats
- **hearth-greeter:** Full rewrite — 5 source files (main.rs, greetd.rs, agent_client.rs, ui.rs, nss.rs), GTK4 + glib + libc + async-channel
- **hearth-api:** 7 new source files (routes/deployments.rs, routes/stats.rs, routes/audit.rs, deployment_fsm.rs, health_check.rs, rollout.rs, deployment_monitor.rs), build pipeline module (5 files: evaluator.rs, builder.rs, cache.rs, config_gen.rs, orchestrator.rs), 1 new migration
- **hearth-agent:** updater.rs rewritten with real Nix commands, poller.rs with deployment status reporting
- **hearth-common:** api_types.rs (+10 types), api_client.rs (+report_update_status), config.rs (+GreeterConfig/BrandingConfig/AgentConnectionConfig/SessionConfig)
- **Frontend:** `@hearth/console` app (TanStack Router + Table + Query + Recharts), 11 pages, 10 API hooks, 10 new UI components in `@hearth/ui`, react-icons integration

---

## Phase 3.5: Identity & Authentication (Kanidm) {#phase-3-5}

Full identity stack using Kanidm as the enterprise IdP. Replaces SSSD-only auth with native Kanidm integration across all layers.

### Tasks

- [x] **3.5A: Kanidm dev stack** — Kanidm container in docker-compose (`ghcr.io/kanidm/server:latest`), idempotent bootstrap script (groups: hearth-users/admins/developers/designers, test users, service account, OAuth2 clients for console + enrollment), dev `.env` generation
- [x] **3.5B: Identity database schema** — Migration 008: `users` table (kanidm_sub, username, email, groups, timestamps), `enrolled_by` FK + `machine_token_hash` on machines table
- [x] **3.5C: API authentication middleware** — Axum extractors: `UserIdentity` (JWKS/RS256), `MachineIdentity` (HS256), `OptionalIdentity`, `AdminIdentity`. JWKS fetching with 15-min TTL cache. `AuthConfig` from env vars. Dev mode bypass when OIDC issuer unset. `/api/v1/auth/me` endpoint
- [x] **3.5D: API client auth + agent tokens** — Bearer token support in `ReqwestApiClient` (`Arc<RwLock<Option<String>>>`), `new_with_token`/`set_token`/`authed_*` helpers, machine token read from disk at startup, token refresh via heartbeat response, `machineTokenPath` in agent config + NixOS module
- [x] **3.5E: Authenticated enrollment** — OAuth2 Device Authorization Grant (RFC 8628) in enrollment TUI: QR code display (Unicode half-blocks), device code polling, JWT username extraction. Enrollment submits user token. `enrollment_status` mints HS256 machine token on first post-approval poll, stores hash. Machine token persisted to disk alongside machine-id
- [x] **3.5F: Kanidm client NixOS module** — `modules/kanidm-client.nix` (configures kanidm-unixd for PAM/NSS on fleet devices: URI, CA cert, allowed login groups, shell, home prefix, HSM type). `modules/pam.nix` updated with `authBackend` option (`kanidm`/`sssd`/`none`). `mk-fleet-host.nix` extended with `kanidmUrl`/`kanidmCaCert` params. Enrollment module extended with `kanidmUrl`/`kanidmClientId`
- [x] **3.5G: Web console OIDC login** — `oidc-client-ts` integration in `@hearth/console`: `auth.ts` (UserManager, PKCE Authorization Code flow), `AuthGuard.tsx` (redirect to Kanidm when unauthenticated), `useAuth` hook, OIDC callback handler, `apiFetch` auto-attaches Bearer token + 401→re-login. User display + sign-out in sidebar

### Stats
- **hearth-api:** 2 new files (auth.rs ~450 lines: JWKS, JWT validation, 4 extractors, token minting; routes/auth_me.rs), error.rs +3 variants, enrollment.rs rewritten for auth + machine token minting, repo.rs +set_machine_token_hash
- **hearth-enrollment:** 2 new files (oauth.rs: device flow client; screens/login.rs: QR code display + polling), app.rs rewritten with Login screen, enroll/status/provision screens updated for authenticated flow + machine token
- **hearth-common:** api_client.rs (Bearer token support, `Arc<RwLock>`), api_types.rs (+EnrollmentResponse, HeartbeatResponse.machine_token, Machine.machine_token_hash), config.rs (+machine_token_path)
- **hearth-agent:** main.rs (token from disk), poller.rs (token refresh from heartbeat)
- **Nix:** New `modules/kanidm-client.nix`, pam.nix rewritten with authBackend enum, mk-fleet-host.nix +kanidmUrl/kanidmCaCert, enrollment.nix +kanidm options, mk-enrollment-image.nix +kanidm passthrough
- **Frontend:** 4 new files in console (auth.ts, AuthGuard.tsx, useAuth.ts, routes/callback.tsx), client.ts rewritten with Bearer injection, __root.tsx with user menu, +oidc-client-ts dep
- **Infra:** docker-compose +kanidm, dev/kanidm/ (server.toml, bootstrap.sh), dev/setup.sh updated, migration 008

---

## Phase 4: Enterprise Hardening {#phase-4}

Close the gap between the development platform and something deployable into a real enterprise environment. The control plane becomes container-ready, the enrollment flow becomes a real provisioner, and per-user environments move beyond role profile fallbacks.

### 4A: Secure Provisioning Pipeline ✓

Complete the enrollment flow — currently the TUI registers the device but doesn't install NixOS.

- [x] **disko integration in enrollment:** Declarative disk partitioning configs (`lib/disko-configs/standard.nix` for GPT+EFI+ext4, `lib/disko-configs/luks-lvm.nix` for LUKS-encrypted LVM). `mk-fleet-host.nix` accepts `diskoConfig` parameter to select partitioning layout per machine.
- [x] **Lanzaboote Secure Boot:** `modules/secure-boot.nix` with Lanzaboote configuration. `mk-fleet-host.nix` accepts `secureBoot` toggle.
- [x] **TPM-backed full disk encryption:** `modules/tpm-fde.nix` with `systemd-cryptenroll` and TPM2 PCR binding. First-boot oneshot service (`hearth-tpm-enroll`) for automatic key enrollment. Configurable device path and PCR list.
- [x] **Hardware profile library:** Three hardware profiles — `hardware/thinkpad-t14s.nix` (AMD, TLP, amd_pstate), `hardware/framework-13.nix` (Intel, fprintd, PSR fix), `hardware/dell-latitude.nix` (Intel, TLP, modesetting). `mk-fleet-host.nix` accepts `hardwareProfile` parameter.

### 4B: Per-User Environment Generation ✓

The Configuration Generator — the most novel component in the architecture. Completes the home-manager #5244 solution by building real per-user closures on the control plane.

- [x] **Configuration Generator:** When the agent reports a first login, the API queries Kanidm for user groups, resolves groups → role, and queues a build. Per-user closure paths stored on UserEnvironment records. Pending user environments delivered via heartbeat response (`pending_user_envs` field).
- [x] **Agent per-user closure activation:** Agent receives pending user environments via heartbeat and processes them. User environment count tracked in Prometheus textfile metrics.
- [x] **Identity sync job:** `identity_sync.rs` background task (5-min default interval) queries Kanidm for all users/groups, diffs against DB, updates user records and triggers rebuilds for changed group memberships. Runs as a spawned background task in the API server with cancellation token support.

### 4C: Build Worker Separation ✓

Extract the build orchestrator into a standalone worker process for container deployment.

- [x] **Build worker process:** The API server enqueues build jobs into a PostgreSQL-backed queue (`build_jobs` table with `build_job_status` enum). A separate `hearth-build-worker` process polls for pending jobs using `SELECT ... FOR UPDATE SKIP LOCKED` for safe concurrent claiming. Workers execute the full pipeline (`nix-eval-jobs` → `nix build` → `attic push` → deployment creation) and update job status throughout. Multiple workers can run in parallel. The API server no longer needs `nix` in its container image.
- [x] **Container images:** OCI images for hearth-api (stateless web server) and hearth-build-worker (with Nix, nix-eval-jobs, attic-client) via `dockerTools.buildLayeredImage` in the flake. REST endpoints for job status: `GET /api/v1/build-jobs` (list with status filter), `GET /api/v1/build-jobs/{id}`.
- [x] **Library extraction:** hearth-api split into lib.rs + main.rs so the build worker can reuse the build pipeline, DB types, and repo layer without duplicating code.

### 4D: Console & API Hardening ✓

- [x] **RBAC for web console:** Three roles — viewer, operator, admin — mapped to Kanidm groups (hearth-viewers, hearth-operators, hearth-admins). `OperatorIdentity` extractor (requires operators OR admins) wired to all write endpoints. `AdminIdentity` for machine CRUD and role closure management. `UserIdentity` for all read endpoints. `MachineIdentity` for device-facing endpoints. Console `useRoles` hook derives permissions from OIDC profile groups, UI hides/disables unauthorized actions.
- [x] **Remote actions:** `pending_actions` table with action types (lock, restart, rebuild, run_command). Actions created via `POST /api/v1/machines/{id}/actions`, delivered via heartbeat response, executed by agent (`actions.rs` — loginctl lock, systemctl reboot, rebuild flag), results reported back via `POST /api/v1/actions/{id}/result`. Console `MachineActions` component with confirm dialogs.
- [x] **`extra_config` structured forms:** Console exposes per-machine `extra_config` field via the existing machine detail page.
- [x] **Basic reporting pages:** `routes/reports.rs` with three endpoints — compliance report (current vs target closure match rates), deployment timeline, enrollment timeline. Console Reports page with StatCards for compliance metrics, recharts BarChart for deployments, LineChart for enrollments.

### 4E: Observability ✓

Hearth ships its own observability stack as part of the control plane deployment.

- [x] **API server metrics:** `metrics` + `metrics-exporter-prometheus` crates. Prometheus `/metrics` endpoint via `PrometheusHandle`. Heartbeat counter (`hearth_heartbeats_total`). Extensible via `metrics::counter!`/`gauge!`/`histogram!` macros.
- [x] **Structured logging:** JSON log output from hearth-api, hearth-agent, and hearth-build-worker. Controlled via `LOG_FORMAT=json` env var. Uses `tracing-subscriber` with `json` feature. Compatible with any log aggregator.
- [x] **hearth-agent Prometheus textfile exporter:** `metrics.rs` writes to `/var/lib/prometheus-node-exporter/hearth.prom` using `prometheus-client` crate — `hearth_agent_info` (machine_id label), `hearth_agent_heartbeat_age_seconds`, `hearth_agent_user_environments`. Atomic write (`.tmp` + rename) for crash safety.
- [x] **Control plane Grafana dashboards:** `deploy/grafana/fleet-overview.json` — 8-panel dashboard (stat panels for active machines/pending enrollments/active deployments/pending builds, time series for heartbeats/deployments/build jobs/agent heartbeat age). Grafana added to docker-compose with dashboard auto-provisioning.
- [x] **Loki for fleet log aggregation:** Loki added to docker-compose (`grafana/loki:3.0.0`). `modules/logging.nix` configures Promtail on fleet devices for journald log forwarding. `deploy/promtail-config.yml` provides standard config. Grafana pre-configured with Loki datasource.

### 4F: Fleet Agent Metrics on Endpoints ✓

- [x] **VictoriaMetrics vmagent NixOS module option:** `modules/metrics.nix` with `services.hearth.metrics.enable` — deploys vmagent with disk-backed WAL buffering (`/var/lib/vmagent`). Scrapes local node_exporter (including Hearth textfile metrics) at configurable interval (default 15s) and pushes via `remote_write` to the control plane. Handles intermittent connectivity automatically. Also enables node_exporter with textfile collector for Hearth agent metrics.

### 4G: Per-Machine Closure Builds & Hardware Capture ✓

Close the enrollment → build → deploy pipeline so that every machine gets a unique NixOS closure incorporating its actual hardware configuration, role, tags, and instance data — rather than a static per-role closure.

- [x] **Device hardware capture:** Enrollment TUI runs `nixos-generate-config --show-hardware-config --no-filesystems` to capture real kernel modules, CPU microcode, firmware, and PCI/USB device requirements. Detects serial number via `dmidecode`. Generates a JSON hardware report (CPU/RAM/disk/NIC) for the control plane.
- [x] **Hardware data transmission:** `EnrollmentRequest` extended with `hardware_report` (JSON), `serial_number`, and `hardware_config` (raw NixOS hardware-configuration.nix content). All stored on the machine record for builds.
- [x] **Database schema:** Migration 012 adds `hardware_config TEXT`, `hardware_report JSONB`, `serial_number TEXT`, `instance_data_hash TEXT`, `module_library_ref TEXT` columns to the machines table.
- [x] **`lib.buildMachineConfig` flake function:** Reads per-machine instance data JSON, uses `builtins.toFile` to inject the device's hardware-configuration.nix as a NixOS module, resolves role/tags/extra_config/kanidm/cache settings into a full `mkFleetHost` call.
- [x] **Build pipeline rewrite:** The orchestrator now generates a temp directory with per-machine JSON files + an `eval.nix` wrapper that creates `nixosConfigurations.<hostname>` for each machine. `nix-eval-jobs --expr 'import eval.nix'` evaluates all machines in a single process with shared thunk efficiency. Each machine gets its own closure path via a `hostname → out_path` map.
- [x] **Per-machine closure assignment:** Deployments track per-machine closures rather than one shared closure. Canary machines receive their machine-specific closure. Instance data hash computed for reproducibility tracking.
- [x] **Auto-rebuild triggers:** When `role` or `extra_config` change on a machine via the API, a build job is automatically enqueued for that specific machine. Enrollment approval also queues a machine-specific build job.
- [x] **Provisioning safety net:** `mk-fleet-host.nix` imports `not-detected.nix` (redistributable firmware + common initrd modules) when no hardware config is provided, preventing non-bootable systems.
- [x] **Provisioning hardening:** `nixos-install` runs with `--no-channel-copy`. Mount verification after disko ensures `/mnt` and `/mnt/boot` are properly mounted before proceeding.

### Stats
- **hearth-api:** 4 new source files (routes/actions.rs, routes/reports.rs, identity_sync.rs, metrics.rs), auth extractors wired to all routes (OperatorIdentity + AdminIdentity for writes, UserIdentity for reads, MachineIdentity for device endpoints), repo.rs extended with pending_actions/user_envs in heartbeat, JSON logging
- **hearth-agent:** 2 new files (actions.rs: lock/restart/rebuild/run_command executor; metrics.rs: Prometheus textfile exporter), poller.rs extended with action processing + metrics writing + action_result replay, JSON logging
- **hearth-common:** api_types.rs (+PendingAction, PendingUserEnv, ActionResultReport, ActionType, ActionStatus types), api_client.rs (+report_action_result)
- **hearth-build-worker:** JSON logging support
- **Frontend:** 4 new files in console (api/actions.ts, api/reports.ts, hooks/useRoles.ts, routes/reports.tsx, components/MachineActions.tsx), router.tsx + __root.tsx updated with Reports nav
- **NixOS:** 3 new modules (tpm-fde.nix, logging.nix, metrics.nix), secure-boot.nix, 2 disko configs (standard, luks-lvm), 3 hardware profiles (thinkpad-t14s, framework-13, dell-latitude)
- **Observability:** deploy/grafana/fleet-overview.json (8-panel dashboard), deploy/promtail-config.yml, docker-compose.yml +loki +grafana
- **SQL:** migration 010 (pending_actions, action_type/action_status enums, compliance/timeline views), migration 012 (hardware_config, hardware_report, serial_number, instance_data_hash, module_library_ref)
- **mk-fleet-host.nix:** Extended with hardwareProfile, secureBoot, tpmFde, tpmDevice, diskoConfig, metricsRemoteWriteUrl, lokiUrl params; `not-detected.nix` safety net when no hardware config
- **Build pipeline:** config_gen.rs (MachineConfig + instance_data_hash + write_build_dir), evaluator.rs (+evaluate_expr), orchestrator.rs (full rewrite for per-machine closures)
- **Enrollment TUI:** hw.rs (+generate_hardware_config, +detect_serial_number, +to_hardware_report), screens/enroll.rs (sends hardware data), screens/provision.rs (+mount verification, +--no-channel-copy)
- **Flake:** `lib.buildMachineConfig` function for per-machine NixOS evaluation

---

## Phase 5: Scale & Advanced Features {#phase-5}

### 5A: Headscale Mesh ✓

Optional VPN overlay for direct device access and secure fleet communication. MagicDNS with `hearth.local` base domain gives every fleet device a stable DNS name (e.g., `ws-0042.hearth.local`), laying the foundation for future intranet services.

- [x] **Headscale server deployment:** Headscale added to docker-compose (`headscale/headscale:0.23`, port 8085). Dev config at `dev/headscale/config.yaml` with SQLite, `100.64.0.0/10` prefix, MagicDNS on `hearth.local`. `just headscale-setup` recipe for user/API key provisioning.
- [x] **Enrollment integration:** Control plane generates a single-use Headscale pre-auth key (1h TTL) during enrollment approval via REST API client (`headscale.rs`). Key stored in `extra_config` JSON and returned in `EnrollmentResponse`. Enrollment TUI writes key to `/mnt/var/lib/hearth/headscale-key` during provisioning. NixOS `headscale-client.nix` module consumes key on first boot via oneshot service (`tailscale up --login-server --authkey`), then deletes it.
- [x] **Direct device SSH:** `headscale_ip` and `headscale_node_id` columns on machines table. Agent detects Headscale IP via `tailscale status --json` and reports it in heartbeats. Console machine detail page shows "Mesh VPN Address" with copy-SSH button. SSH enabled by headscale-client module.
- [x] **Agent communication over mesh:** Agent config supports `headscale.mesh_server_url`. When set, agent uses the mesh URL as its primary API endpoint. NixOS agent module generates the TOML config. `mk-fleet-host.nix` accepts `headscaleUrl` parameter.
- [x] **VM integration test:** `tests/headscale-mesh.nix` validates module wiring, join service ConditionPathExists gating, firewall rules, and agent heartbeat flow.

#### Future: Intranet Services over Mesh

The Headscale mesh with MagicDNS provides the foundation for fleet-internal services accessible via `*.hearth.local` DNS names. Planned capabilities:

- **Internal knowledge base / wiki** accessible at e.g. `wiki.hearth.local` from any fleet device
- **Custom DNS records** via Headscale `dns.extra_records` for named service endpoints
- **Subnet routing** to bridge the mesh into existing on-prem infrastructure (office LANs, NAS, printers)
- **Control plane over mesh** for air-gapped deployments where fleet devices have no public internet route

### Stats
- **hearth-api:** 1 new source file (headscale.rs: REST client for pre-auth keys + node listing), enrollment.rs extended with pre-auth key generation on approval, repo.rs heartbeat query +headscale_ip, lib.rs AppState +headscale field, main.rs HeadscaleClient init
- **hearth-common:** api_types.rs +headscale fields on Machine/HeartbeatRequest/EnrollmentResponse, config.rs +HeadscaleAgentConfig
- **hearth-agent:** 1 new source file (headscale.rs: detect_headscale_ip via tailscale status), poller.rs +headscale_ip in heartbeat, main.rs mesh_server_url support
- **hearth-enrollment:** status.rs +headscale_preauth_key capture, app.rs +state transfer, provision.rs +headscale-key file write
- **Frontend:** types.ts +headscale_ip/headscale_node_id on Machine, $machineId.tsx +Mesh VPN Address field with Copy SSH button
- **NixOS:** New `modules/headscale-client.nix` (Tailscale + oneshot join service + firewall + SSH), agent.nix +headscale config options + tailscale in PATH, mk-fleet-host.nix +headscaleUrl parameter
- **Infra:** docker-compose.yml +headscale service, dev/headscale/config.yaml, justfile +headscale-setup recipe
- **SQL:** migration 015 (headscale_ip, headscale_node_id columns on machines)
- **Tests:** New `tests/lib/headscale-test.nix` (reusable Headscale server + bootstrap helper), `tests/headscale-mesh.nix` (4-node VM test: real Headscale + Tailscale mesh with peer connectivity + agent heartbeat verification)

### 5B: Compliance Engine ✓

- [x] **Config drift detection API:** Per-machine drift detail endpoint (`GET /api/v1/compliance/drift`) with status filtering (drifted/compliant/no_target). Console compliance page with summary stat cards, donut chart, filterable drift table with click-through to machine detail. Sidebar nav integration.
- [x] **Nix assertion policies:** Policy CRUD endpoints (`GET/POST/PUT/DELETE /api/v1/compliance/policies`). Policies stored in `compliance_policies` table with name, Nix expression, severity, control ID, enabled flag. Build pipeline evaluates all enabled policies per-machine via `nix eval --json` with `builtins.tryEval` fault isolation. Results recorded in `policy_results` table per-deployment per-machine. Non-blocking — violations are recorded but don't stop deployments. Console policy management tab.
- [x] **SBOM generation:** Build worker generates CycloneDX JSON SBOMs via `sbomnix` for each built closure. Stored on disk at `$HEARTH_SBOM_DIR/{deployment_id}/{hostname}.cdx.json` with DB references in `deployment_sboms` table. API endpoints to list, download per-deployment, and retrieve current SBOM for any machine. Non-blocking — failures logged but don't stop deployments.
- [x] **STIG/CIS NixOS module library:** 5 starter compliance control modules following the `hardening.nix` pattern — CIS 1.1.1 (uncommon filesystem mounting), CIS 3.4.1 (firewall enabled), CIS 4.2.1 (persistent journald), STIG V-230223 (SSH hardening), STIG V-230271 (USB mass storage disabled). Each module exposes `enable` + read-only `meta` (id, title, severity, description, family, benchmark). Profile-based activation via `services.hearth.compliance.profile` (cis-level1, cis-level2, stig). Integrated into `mk-fleet-host.nix` with `complianceProfile` parameter. Build pipeline extracts `compliance_profile` from machine `extra_config`.

### Stats
- **hearth-api:** 3 new source files (routes/compliance.rs, build/policy_eval.rs, build/sbom.rs), orchestrator.rs extended with policy eval + SBOM pipeline steps, repo.rs +14 query functions, db.rs +4 row types
- **hearth-common:** api_types.rs +9 types (DriftedMachine, DriftStatus, CompliancePolicy, PolicyResult, DeploymentSbom, etc.)
- **Frontend:** 2 new files (api/compliance.ts with 8 hooks, routes/compliance.tsx with drift table + policy management), sidebar nav + router integration
- **NixOS:** New `modules/compliance/` directory with default.nix + 5 control modules (CIS + STIG), mk-fleet-host.nix +complianceProfile parameter
- **SQL:** migration 014 (compliance_policies, policy_results, deployment_sboms tables)

### 5C: Hearth Home Cluster Helm Chart ✓

Production-ready Kubernetes deployment for the Hearth control plane and all supporting services. Uses a capabilities toggle model for incremental adoption.

- [x] **Helm chart scaffolding:** `chart/hearth-home/` with Chart.yaml, values.yaml (full schema), values-production.yaml example overlay, `_helpers.tpl` (labels, names, URL assembly, secret resolution, database URL construction).
- [x] **Core templates (always deployed):** hearth-api Deployment (initContainer wait-for-postgres, conditional env vars per capability, secret injection, liveness/readiness probes on /api/v1/health), Service, ConfigMap, Secret (auto-generated with upgrade-safe lookup), Ingress, ServiceAccount, PDB. Attic binary cache Deployment (initContainer for JWT secret injection into server.toml), ConfigMap, PVC (local or S3), Ingress. PostgreSQL via Bitnami subchart (or external DB).
- [x] **Identity capability (Kanidm):** StatefulSet (SQLite on PVC), Service, ConfigMap (server.toml), TLS Secret (self-signed cert generation with upgrade-safe lookup, or existing secret), Ingress (backend-protocol: HTTPS). Bootstrap Job (post-install Helm hook) with ServiceAccount + RBAC — creates groups, OAuth2 clients, API service account via kubectl exec + REST API. Production-minimal: no test users.
- [x] **Mesh capability (Headscale):** Deployment, Service, ConfigMap (config.yaml with MagicDNS, DERP, prefixes), PVC, Ingress. API key injection via existingSecret.
- [x] **Builds capability (Build worker):** Deployment with wait-for-postgres, ConfigMap, PVCs (persistent Nix store + scratch space). Heavy resource defaults (2 CPU, 4GB RAM).
- [x] **Observability capability:** Grafana, Loki, Prometheus subcharts (conditionally pulled). ServiceMonitors for hearth-api and Headscale /metrics. Grafana dashboard ConfigMap (fleet overview).
- [x] **Testing infrastructure:** 12 helm-unittest test suites (105 tests) covering all templates, capabilities toggles, conditional env vars, secret generation, Kanidm bootstrap RBAC. Kubeconform schema validation against K8s 1.29.0. Chart-testing (ct) smoke test config with CI values for Kind cluster. `helmChartLint` Nix flake check.
- [x] **CI workflow:** `.github/workflows/helm.yml` — lint + unittest, kubeconform (3 value combos), ct install on Kind. Triggers on `chart/` changes.
- [x] **Local cluster bootstrap:** `just helm-up` / `just helm-down` recipes for Kind cluster lifecycle. `just helm-check` runs lint + unittest + kubeconform.

### 5D: Per-User Environment System ✓

Role templates are initial seeds; each user gets a managed per-user closure that follows them across machines (Azure AD-style roaming profiles).

- [x] **Per-user config schema:** `user_configs` table (base_role, JSONB overrides, config_hash, latest_closure, build_status), `user_env_build_jobs` queue. Migration 016.
- [x] **User config API:** CRUD at `/api/v1/users/{username}/config`, env-closure lookup at `/api/v1/users/{username}/env-closure`, force-build trigger.
- [x] **Per-user build pipeline:** `lib.buildUserEnv` Nix expression composes role template + override module (git config, extra packages, editor, shell aliases, session variables). Build worker polls user_env_build_jobs. Background sweep enqueues pending builds.
- [x] **Agent per-user closure activation:** Agent queries control plane for pre-built closure at login. When no closure is ready, agent polls the API for up to 5 minutes with build-status-aware progress messages (queued/building/failed) instead of falling back. Heartbeat pre-stages closures from cache.
- [x] **Systemd socket activation:** Agent IPC socket managed by systemd with correct greeter permissions.
- [x] **Greeter fixes:** File-based password delivery for headless tests, HEARTH_GREETER_LOG_FILE support, proper Kanidm TLS cert chain.
- [x] **Agent home directory creation:** Agent resolves users via `getent passwd` to get the canonical SPN username and home path, creates home directories with correct ownership before activation. `resolve_and_ensure_home()` returns the passwd name for API calls.
- [x] **Refactored activation paths:** Extracted `realise_closure_with_progress()` helper that streams `nix-store --realise` stderr as progress events to the greeter (e.g., "fetching firefox-147.0.3 (12 fetched)"). Replaced `.output()` blocking calls with streamed subprocess management with cancellation support.
- [x] **Binary cache pipeline:** Fleet hosts configured with Attic + cache.nixos.org as Nix substituters (`nix.settings.substituters` + `trusted-public-keys`). Agent uses `nix-store --realise` (pulls from all substituters) instead of `nix copy --from` (single source). Attic cache key auto-captured during `just setup` with retry loop. `nix.settings.netrc-file` points at agent-managed credentials for authenticated cache access.
- [x] **Closure failure recovery:** Agent reports broken closures to the server via `POST /api/v1/users/{username}/env-closure/report-failure`, which invalidates the closure and enqueues a rebuild. Prevents rebuild loops by checking for existing pending/building jobs. `env-closure` response includes `build_status` enum for agent progress display.
- [x] **SPN-based home directories:** `kanidm-client.nix` uses `home_attr = "spn"` so passwd home = `/home/user@domain`, matching what `buildUserEnv` produces. Agent resolves greeter's short username to the full SPN before API calls.
- [x] **kanidm-unixd boot resilience:** Added `Restart=on-failure` with network-online ordering to kanidm-unixd and kanidm-unixd-tasks services, fixing permanent failure when the Kanidm server isn't reachable during early boot.
- [x] **Full login flow VM test:** Fixed dynamic home directory resolution from getent passwd (handles kanidm-unixd SPN-based home_attr), mock home-manager activation marker verification works end-to-end.
- [x] **homeConfigurations flake output:** CI-verifiable home-manager configurations for all four roles (default, developer, designer, admin). Fixed home-manager deprecation warnings (git.extraConfig → git.settings, git.delta → programs.delta, ssh.extraConfig → ssh.matchBlocks).
- [x] **Package allowlist/denylist:** `HEARTH_PACKAGE_ALLOWLIST` env var restricts `extra_packages` in user overrides (API returns 400 listing disallowed packages). Nix-side defense-in-depth: `buildUserEnv` filters `package_denylist` field.
- [x] **Self-service config UI:** `/api/v1/me/config` GET/PUT endpoints with `UserIdentity` auth (restricted fields: git config, editor, shell aliases, session variables). Admin-only fields preserved on merge. Settings page at `/settings` with key-value editors, visible to all users. `AuthClaims::username()` method consolidates duplicated username extraction.
- [x] **Fleet VM dev tooling:** `just fleet-exec <cmd>` and `just fleet-ssh` for remote command execution on running fleet VMs via SSH. Dedicated SSH keypair generated by `just setup` (gitignored). QEMU port forwarding (host 2222 → guest 22). Software rendering (`virtio-vga` without GL) for stable GNOME in QEMU.

### 5D-2: Desktop Personalization & Dash to Panel ✓

User-owned desktop preferences that survive closure rebuilds, plus a traditional taskbar replacing GNOME's Activities overview.

- [x] **Dash to Panel extension:** `gnomeExtensions.dash-to-panel` installed system-wide (`modules/desktop.nix`), enabled and configured via dconf in `home-modules/common.nix` (bottom panel, 42px, always visible). All roles inherit automatically; developer role merges with existing AppIndicator extension.
- [x] **Desktop preferences sync-back:** New `DesktopPreferences` type (favorite_apps, wallpaper_uri, wallpaper_color, dark_mode) in `hearth-common`. Agent reads curated dconf keys via `runuser`/`dconf read` after login activation and periodically (every 30 min for active sessions via `loginctl`). Syncs to control plane via new machine-scoped endpoint `PUT /api/v1/machines/{machine_id}/users/{username}/desktop-prefs` (`MachineIdentity` auth). Preferences merged into `user_configs.overrides.desktop` JSONB; rebuilds triggered only when config_hash changes.
- [x] **Nix override module:** `flake.nix:buildUserEnv` applies `cfg.overrides.desktop` to dconf settings with `lib.mkForce`, overriding role defaults for favorite-apps, wallpaper, and color scheme.
- [x] **Self-service endpoint:** `PUT /api/v1/me/config` extended with `desktop` field so users can also set preferences via the web UI.
- [x] **Role-specific dash favorites:** Curated per-role favorite-apps (default: Firefox/Nautilus/Terminal/LibreOffice; developer: Firefox/VSCodium/Kitty/Nautilus; admin: Firefox/Kitty/Nautilus; designer: Firefox/GIMP/Inkscape/Nautilus) with conditional Element/Nextcloud/Thunderbird appends.

### 5E: User Environment Polish (Future)

- [ ] **Closure pre-warming:** When a machine enrolls or changes role, the control plane enumerates likely users (from Kanidm group membership for the assigned role) and queues pre-builds of their per-user closures. Reduces first-login latency from "1–3 minute build" to "15–60 second cache pull."
- [ ] **WiFi/802.1X certificate distribution:** The control plane provisions 802.1X machine certificates as part of enrollment secrets. The NixOS module configures `wpa_supplicant` or `iwd` with the certificate and network profile. Certificates rotate via the control plane's secret management.
- [ ] **Expanded desktop personalization:** Extend the sync-back key set beyond the initial curated list — keyboard shortcuts, accessibility settings (font scaling, high contrast), notification preferences, GNOME extension toggles. Each key requires explicit opt-in to the "user-owned" set to prevent security-sensitive settings from being user-modifiable.
- [ ] **Org-level theming & branding:** Organization-wide wallpapers, color schemes, and branding assets pushed from the control plane as a theming layer between role defaults and user overrides. Configurable per-org in Helm values. Applied via a separate dconf priority so users can override aesthetics but not branding.
- [ ] **Preference conflict policy:** Define behavior when fleet policy changes a key the user has customized (fleet wins / user wins / notify). Configurable per-key or per-category. Default: user wins for personalization keys, fleet wins for security/compliance keys.
- [ ] **Catalog approval persistence:** When a user requests a Nix package through the Software Center and it's approved, write the package into `user_configs.overrides.extra_packages` so it survives closure rebuilds. Currently approved Nix catalog items are installed on the device but not persisted into the user's config — the next closure rebuild removes them. Flatpak catalog items are unaffected (they live outside the closure).
- [ ] **Cached closure for offline subsequent logins:** First login on a machine must sync against the control plane to build/pull the per-user closure. Subsequent logins should work immediately via PAM with the locally cached closure, regardless of connectivity. The agent activates the last-known-good closure from the local Nix store. Internet is only required for the initial bootstrap — after that, login is fully offline-capable.
- [ ] **Heartbeat staleness policy:** Define a configurable staleness threshold (e.g., 7 days without heartbeat) after which a machine is flagged as out-of-compliance in the dashboard. The agent tracks its last successful heartbeat locally and can display a warning in the greeter ("last synced N days ago"). IT can configure the policy response: warn-only, restrict to read-only session, or require re-sync before login. The threshold and response are fleet-wide settings, not per-machine.
- [ ] **Closure rollback:** Keep the previous home-manager generation's closure path on the device. If the latest closure fails activation (e.g., bad package, config regression), the agent falls back to the previous generation and reports the failure to the control plane. The greeter could also offer a "Use previous environment" option when the current one fails. Home-manager already tracks generations — the agent just needs to remember the last-known-good path.
- [ ] **Role+fleet closure layering:** Split the per-user closure into two layers to reduce build fan-out from fleet-wide config changes. A **role+fleet closure** (shared, built once per role when fleet config changes) provides the base with all collaboration apps configured. A **user overlay** (personal customizations: git config, extra packages, desktop prefs) is composed on top. When IT enables a new service, only 4–5 role closures rebuild instead of N per-user closures. The overlay is small and fast to build since it only contains deltas.
- [ ] **Extension/add-on allowlists:** Firefox `ExtensionSettings` policy supports per-extension `installation_mode` — expose an `allowedExtensions` list in the fleet config so IT can define which extensions users may self-install beyond the force-installed set. Same pattern for LibreOffice extensions via the managed extension list. Blocked extensions can also be specified (malware, data exfiltration risk).
- [ ] **Machine-specific vs. roaming preferences:** Separate machine-local preferences (display scaling, default printer, Bluetooth pairings, network profiles) from roaming preferences (wallpaper, dark mode, favorite apps) that follow the user across devices. Machine-local prefs stored on the `user_environments` record scoped to `machine_id + username`, not synced into the per-user closure. Roaming prefs continue to flow through `user_configs.overrides.desktop`.
- [ ] **Closure preview for IT changes:** Before applying a fleet config change that triggers mass closure rebuilds, IT can trigger a "preview build" for a single test user/role via the console. Shows a diff of what packages/configs would change. Prevents surprises from config typos or unexpected module interactions. Could also integrate with the existing deployment canary system.

### 5F: Secure Remote Access (Future)

Production-grade remote access to fleet devices for IT operations and debugging. Dev environments use direct SSH over QEMU port forwarding (`just fleet-exec`); production needs audit-logged, time-limited access with proper authentication.

- [ ] **On-demand SSH via Headscale mesh:** SSH enabled on fleet devices but only reachable over the Headscale VPN. Access requires being authenticated on the mesh. Short-lived SSH certificates issued by the control plane (similar to Teleport/Smallstep) replace long-lived keys. Certificate TTL configurable per-role (e.g., 8h for operators, 1h for developers). All sessions audit-logged.
- [ ] **Break-glass emergency access:** SSH disabled by default on fleet devices. An admin can trigger an "enable SSH" remote action via the console that temporarily opens SSH access with a one-time key, auto-disables after a configurable timeout (default 30 minutes). Requires `AdminIdentity` auth. Creates an audit event with the admin's identity and justification.
- [ ] **Enhanced remote actions:** Extend the existing `run_command` remote action with streaming output (SSE or WebSocket), timeout controls, and working directory selection. Console UI for interactive command execution against selected machines. All commands audit-logged with full input/output capture.

### 5G: Scale (Future)

- [ ] **PXE/iPXE boot service:** Control plane serves boot images based on device identity — unknown devices get the enrollment image, known devices boot from local disk, reprovisioning devices get a fresh installer. Uses iPXE chain-loading from an HTTP endpoint. Enables zero-touch provisioning of 50+ machines simultaneously.
- [ ] **gRPC/SSE push notifications:** Optional push channel from control plane to agent for latency-sensitive deployments. Agent maintains a long-lived connection over the Headscale mesh (or direct HTTPS). Control plane wakes the agent immediately when a new target closure is set, rather than waiting for the next 60-second poll cycle.

---

## Phase 6: Collaboration Services {#phase-6}

Extend the Hearth platform with collaboration services deployed as Helm capabilities on the control plane, integrated with Kanidm SSO. Services are accessible over the regular network (no VPN required) and optionally via MagicDNS for mesh-connected devices.

### 6A: Matrix/Synapse + Element (Chat) ✓

Internal-only corporate chat with Kanidm SSO. Synapse runs as part of the control plane (no VPN required). Element Desktop pre-configured with SSO immediate redirect and GNOME Keyring session persistence.

- [x] **Docker-compose:** Synapse container (`matrixdotorg/synapse:v1.122.0`, port 8008) + Element Web container (`vectorim/element-web:v1.11.96`, port 8088) for local dev. PostgreSQL init script creates separate `synapse` database. Synapse config: client-only listener (no federation), `federation_domain_whitelist: []`, OIDC provider pointing to Kanidm, auto-join `#general` and `#random` rooms.
- [x] **Kanidm OIDC integration:** `hearth-matrix` OAuth2 client (confidential) in `dev/kanidm/bootstrap.sh` and Helm bootstrap configmap. Scopes: `openid`, `profile`, `email`. Client secret written to `.env` and injected into Synapse container.
- [x] **Synapse bootstrap:** Idempotent `dev/synapse/bootstrap.sh` — registers `hearth-bot` admin user via `registration_shared_secret`, creates default rooms (`#general`, `#random`, `#it-support`) with federation disabled per-room, posts welcome messages. `just matrix-setup` recipe, integrated into `just setup`.
- [x] **Helm capability:** `capabilities.chat: false` (off by default). Templates: Synapse Deployment (wait-for-postgres initContainer, health probes, configmap checksum), ConfigMap (homeserver.yaml with conditional OIDC when `capabilities.identity` enabled), Service, PVC (media store), Ingress, Secret (auto-generated registration shared secret with upgrade-safe lookup), bootstrap Job (post-install hook, creates admin bot + default rooms). Kanidm bootstrap configmap extended to create `hearth-matrix` confidential OAuth2 client when chat enabled.
- [x] **NixOS desktop integration:** `home-modules/chat.nix` — Element Desktop with pre-configured homeserver URL, `sso_redirect_options.immediate: true` (skips login form), `disable_custom_urls: true` (corporate lockdown), XDG autostart (`element-desktop --use-keychain --hidden` — GNOME Keyring session persistence, minimized to tray). `modules/chat.nix` NixOS module, `mk-fleet-host.nix` extended with `matrixUrl`/`matrixServerName` parameters. Element Desktop added to GNOME favorites conditionally across all role profiles.
- [x] **Helm tests:** 24 new tests in `synapse_test.yaml` (deployment, service, configmap, PVC, secret, ingress, bootstrap job). 4 new tests in `capabilities_test.yaml` for chat toggle. All 131 tests passing.

### Stats
- **Dev infra:** 5 new files in `dev/synapse/` (homeserver.yaml, element-config.json, log.config, init-db.sh, bootstrap.sh)
- **Docker-compose:** +synapse service, +element-web service, +postgres init script mount, +2 volumes
- **Kanidm bootstrap:** +hearth-matrix confidential OAuth2 client, +MATRIX_OIDC_CLIENT_SECRET in .env
- **Home-manager:** New `home-modules/chat.nix` (Element Desktop module with SSO, autostart, keychain), `common.nix` imports chat.nix, 4 role profiles updated with conditional Element favorites
- **NixOS:** New `modules/chat.nix`, `mk-fleet-host.nix` +matrixUrl/matrixServerName parameters
- **Helm chart:** 6 new templates in `templates/synapse/` (deployment, service, configmap, pvc, ingress, secret, job-bootstrap), `values.yaml` +capabilities.chat +synapse config section, `kanidm/bootstrap-configmap.yaml` extended for hearth-matrix client
- **Tests:** 28 new helm-unittest tests (24 synapse + 4 capabilities), 131 total passing

### 6B: Nextcloud (Cloud Storage & Collaboration) ✓

File sync and collaboration with Kanidm SSO, GNOME desktop integration, WebDAV mounts, and LibreOffice integration.

- [x] **Docker-compose:** Nextcloud (`nextcloud:30-apache`, port 8089) + Redis (`redis:7-alpine`) containers for local dev. PostgreSQL init script creates separate `nextcloud` database. Healthcheck on `/status.php`. Depends on postgres + redis healthy.
- [x] **Kanidm OIDC integration:** `hearth-nextcloud` confidential OAuth2 client in `dev/kanidm/bootstrap.sh` and Helm bootstrap configmap. Scopes: `openid`, `profile`, `email`. Client secret written to `.env` and injected into Nextcloud container.
- [x] **Nextcloud bootstrap:** Idempotent `dev/nextcloud/bootstrap.sh` — installs `user_oidc` app, configures Kanidm as OIDC provider, sets up Redis caching, configures trusted domains, creates default folders (Documents, Projects, Shared). `just nextcloud-setup` recipe, integrated into `just setup`.
- [x] **Helm capability:** `capabilities.cloud: false` (off by default). Templates: Nextcloud Deployment (Redis sidecar, wait-for-postgres initContainer, health probes on `/status.php`, Recreate strategy), ConfigMap (trusted domains, server URL), Service, PVC (50Gi data), Secret (auto-generated admin + DB passwords with upgrade-safe lookup), Ingress (proxy-body-size 16G annotation), bootstrap Job (post-install hook weight 25). Kanidm bootstrap configmap extended to create `hearth-nextcloud` confidential OAuth2 client when cloud enabled.
- [x] **NixOS desktop integration:** `modules/nextcloud.nix` system module (GVFS + davfs2 for WebDAV mount support). `home-modules/nextcloud.nix` home-manager module — Nextcloud Desktop sync client with pre-configured server URL, XDG autostart (`nextcloud --background`), systemd user service for GVFS WebDAV mount on login (`gio mount davs://...`), per-user WebDAV bookmark in Nautilus sidebar (`davs://server/remote.php/dav/files/USERNAME/ Cloud Storage`). LibreOffice works natively with synced ~/Nextcloud folder and `davs://` URLs via GVFS. `mk-fleet-host.nix` extended with `nextcloudUrl` parameter. All 4 role profiles updated with conditional Nextcloud favorites. Default + designer profiles get WebDAV bookmarks.
- [x] **Helm tests:** 24 new tests in `nextcloud_test.yaml` (deployment, Redis sidecar toggle, service, configmap, PVC, secret, ingress, bootstrap job). 4 new tests in `capabilities_test.yaml` for cloud toggle. All 159 tests passing.

### Stats
- **Dev infra:** 2 new files in `dev/nextcloud/` (init-db.sh, bootstrap.sh)
- **Docker-compose:** +nextcloud service, +nextcloud-redis service, +postgres init script mount, +1 volume
- **Kanidm bootstrap:** +hearth-nextcloud confidential OAuth2 client, +NEXTCLOUD_OIDC_CLIENT_SECRET in .env
- **Home-manager:** New `home-modules/nextcloud.nix` (Nextcloud Desktop module with sync client, WebDAV mount service, autostart), `common.nix` imports nextcloud.nix, 4 role profiles updated with conditional Nextcloud favorites, 2 role profiles with WebDAV Nautilus bookmarks
- **NixOS:** New `modules/nextcloud.nix` (GVFS + davfs2), `mk-fleet-host.nix` +nextcloudUrl parameter
- **Helm chart:** 7 new templates in `templates/nextcloud/` (deployment, service, configmap, pvc, ingress, secret, job-bootstrap), `values.yaml` +capabilities.cloud +nextcloud config section, `_helpers.tpl` +nextcloudUrl, `kanidm/bootstrap-configmap.yaml` extended for hearth-nextcloud client
- **Tests:** 28 new helm-unittest tests (24 nextcloud + 4 capabilities), 159 total passing

### 6C: Shared Service Infrastructure ✓

Common patterns extracted as services multiply. Includes Nextcloud OIDC bootstrap automation fix.

- [x] **Service OIDC proxy:** oauth2-proxy forward-auth middleware deployed as Helm capability (`oauth2Proxy.enabled`, auto-enabled with `capabilities.identity`). Kanidm bootstrap creates `hearth-proxy` confidential OAuth2 client. Deployment, Service, Secret templates with health probes. Future services can add nginx ingress auth annotations to use the proxy.
- [x] **Service discovery API:** `GET /api/v1/services` endpoint returning enabled service URLs, descriptions, icons, and categories. Config-driven from environment variables (`HEARTH_CHAT_URL`, `HEARTH_CLOUD_URL`, `HEARTH_IDENTITY_URL`). Services also delivered in heartbeat response for agent consumption. API ConfigMap extended with conditional service URL env vars.
- [x] **Service directory page:** `/services` page in the web app listing all enabled collaboration services as cards grouped by category (Infrastructure, Communication, Storage, Identity). Available to all authenticated users. Added to sidebar navigation.
- [x] **Agent desktop integration:** Agent writes `/var/lib/hearth/services/services.json` manifest and `.desktop` link files from heartbeat response. New `home-modules/services.nix` home-manager module syncs desktop files via systemd user service/timer. `mk-fleet-host.nix` auto-enables when chat or cloud is configured.
- [x] **Nextcloud OIDC bootstrap fix:** Nextcloud Helm bootstrap job upgraded from manual OIDC instructions to automated `occ` commands via `kubectl exec`. Installs `user_oidc` app, configures Kanidm provider, sets up Redis caching. Added RBAC (ServiceAccount, Role, RoleBinding) for pod exec access. Matches dev bootstrap automation.
- [x] **Helm tests:** 24 new tests (14 oauth2-proxy, 4 capabilities, 6 API configmap service URLs). All 183 tests passing.

### Stats
- **Rust:** New `ServiceInfo`/`ServiceCategory` types in hearth-common, new `services` field on `HeartbeatResponse` and `AppState`, new `/api/v1/services` route, env var parsing in main.rs, agent writes service bookmarks from heartbeat
- **Frontend:** New `api/services.ts` hook, `ServiceInfo` type, `/services` page with categorized service cards, sidebar nav item
- **Helm chart:** 3 new templates in `templates/oauth2-proxy/` (deployment, service, secret), `values.yaml` +oauth2Proxy config section, API ConfigMap +service URL env vars, Nextcloud bootstrap rewritten with RBAC + kubectl exec OIDC automation, Kanidm bootstrap +hearth-proxy client + proxy-oidc-secret
- **NixOS:** New `home-modules/services.nix` (systemd user service/timer for .desktop sync), `common.nix` imports services.nix, `mk-fleet-host.nix` auto-enables services module
- **Tests:** 24 new helm-unittest tests, 183 total passing

### 6D: Enterprise Productivity {#phase-6d}

Surfaces collaboration tools in ways that integrate naturally with the GNOME desktop and each other. Builds on the identity, chat, and cloud capabilities from earlier phases.

#### 6D-1: People Directory ✓

Company-wide people directory sourced from Kanidm identity data, enriched with derived contact info from enabled services. Zero extra infrastructure — a view over data that already exists.

- [x] **Directory API:** `GET /api/v1/directory/people` endpoint (requires `UserIdentity` auth). Queries the `users` table, enriches each person with derived Matrix ID (`@username:{matrix_server_name}` via `HEARTH_MATRIX_SERVER_NAME` env var) and Nextcloud profile URL (derived from the `cloud` service entry). 6 unit tests covering all service combinations, trailing-slash normalization, and missing fields.
- [x] **Directory page:** `/directory` page in the web app — searchable card grid showing each person with initials avatar, display name, username, group badges, contact links (email `mailto:`, Matrix via `matrix.to`, Nextcloud profile), and relative "last seen" timestamp. Available to all authenticated users.
- [x] **Sidebar navigation:** "People" item with `LuUsers` icon added to the user-visible nav (alongside Catalog, Services, Settings).

##### Stats
- **hearth-common:** `DirectoryPerson` type in api_types.rs (username, display_name, email, groups, matrix_id, nextcloud_url, last_seen)
- **hearth-api:** New `routes/directory.rs` (handler + 6 unit tests), `matrix_server_name` field on `AppState`, Nextcloud URL derived from `state.services` at request time
- **Frontend:** New `api/directory.ts` (useDirectory hook), new `routes/directory.tsx` (DirectoryPage with search + card grid), router + sidebar wiring

#### 6D-2: Email, Calendar & Contacts ✓

Thunderbird as the unified PIM client (mail + calendar + contacts), connected to Nextcloud CalDAV/CardDAV and the org's mail server. GNOME Online Accounts also connected for shell panel calendar integration. Stalwart Mail Server available as an optional self-hosted mail capability.

- [x] **Thunderbird home-manager module:** `home-modules/thunderbird.nix` deploying Thunderbird with managed policies — pre-configured Nextcloud CalDAV/CardDAV via TbSync + DAV-4-TbSync force-installed extensions, disabled telemetry, autostart option. Added to GNOME favorites across all role profiles.
- [x] **Kanidm OIDC for mail:** Deploy `thunderbird-custom-idp` extension via `ExtensionSettings` policy — enables OAuth2/OIDC auth for IMAP/SMTP via Kanidm. Eliminates stored passwords for mail accounts. Conditional on `mail.useOidc`.
- [x] **Mail autoconfig:** Local Thunderbird autoconfig XML placed via NixOS home-manager, pre-configuring IMAP/SMTP server settings with OAuth2 auth. Supports both self-hosted (Stalwart) and external IMAP/SMTP providers.
- [x] **Stalwart Mail Server:** Optional Helm capability (`capabilities.mail`) with hosted/external toggle. When `stalwart.hosted=true`, deploys Stalwart in-cluster with Kanidm OIDC, shared PostgreSQL, bootstrap job. When `stalwart.hosted=false`, passes external IMAP/SMTP host settings to fleet devices. Kanidm bootstrap auto-creates `hearth-stalwart` OAuth2 client.
- [x] **GNOME Online Accounts:** `home-modules/gnome-online-accounts.nix` pre-seeds a Nextcloud account in GOA. Feeds `evolution-data-server` so GNOME Shell panel clock shows upcoming events and GNOME Contacts works alongside Thunderbird.
- [x] **Shared organizational calendar:** Nextcloud bootstrap creates calendar/contacts apps and default shared calendars (company holidays, all-hands) via `occ dav:create-calendar`. Configurable in `values.yaml` under `nextcloud.bootstrap.sharedCalendars`.
- [x] **NixOS module:** `modules/thunderbird.nix` ensures system-level dependencies (gnome-online-accounts, evolution-data-server). `mk-fleet-host.nix` auto-enables Thunderbird when `nextcloudUrl` is set; mail is opt-in via `mailImapHost`/`mailSmtpHost`/`mailDomain`.

**Stats:** 3 new NixOS/home-manager modules, 7 Helm templates, 1 Helm test suite (30 tests), 5 modified role profiles, Nextcloud bootstrap extended. Helm tests: 211 total (was 105).

#### 6D-3: LibreOffice-First Document Workflow + Rust UNO Extensions

LibreOffice as the primary document editor with Nextcloud for sync and sharing. Custom Rust UNO extensions (using LibreOffice 26.2's new Rust bindings) fill the integration gaps — no browser-based Collabora deployment needed.

**Phase 1: Managed LibreOffice Configuration**

- [x] **LibreOffice home-manager module:** `home-modules/libreoffice.nix` with managed `registrymodifications.xcu` — default save path (`~/Nextcloud/Documents/`), template path (`~/Nextcloud/Templates/`), pre-configured WebDAV remote server, file locking enabled, macro security (high/very-high), telemetry disabled, fleet-standard fonts (DM Sans, Noto Serif, JetBrains Mono). Extension installation support via `unopkg`.
- [x] **NixOS option interface:** `modules/libreoffice.nix` following the chat/thunderbird pattern. Options: enable, nextcloudUrl, defaultFonts, macroSecurity, enableExtensions.
- [x] **Fleet wiring:** `mk-fleet-host.nix` auto-enables LibreOffice when `nextcloudUrl` is set. Module imported in flake.nix meta-module.
- [x] **Role profile cleanup:** LibreOffice package moved from individual role profiles to the centralized libreoffice module. GNOME favorites conditional on `hearth.libreoffice.enable` across default, designer, and admin roles.
- [x] **Extension config file:** `~/.config/hearth/office.toml` with Nextcloud URL, written by home-manager for Phase 2 extensions.

**Phase 2: Rust UNO Extensions ✓**

Native LibreOffice extensions built with the Rust UNO bindings (LibreOffice 26.2, `--enable-rust-uno`). Packaged as a single `.oxt` via Nix, installed by the libreoffice home-manager module.

- [x] **LibreOffice overlay:** `nix/libreoffice-hearth/` overrides the nixpkgs LibreOffice to 26.2.2.2 source with `--enable-rust-uno` configure flag, Rust toolchain in nativeBuildInputs, and `rust_uno` crate extraction as a separate output. Wrapped via the standard `wrapper.nix`. Exported as `pkgs.libreoffice-hearth` via overlay.
- [x] **hearth-office crate:** Standalone Cargo project at `crates/hearth-office/` (`crate-type = ["cdylib"]`, NOT a workspace member). Uses `hearth-common` as path dep, `reqwest` (blocking + rustls) for Nextcloud API, `arboard` for clipboard, `configparser` for NC Desktop config reading, `quick-xml` for WebDAV PROPFIND parsing. Reads config from `~/.config/hearth/office.toml`. Auth via Nextcloud Desktop client stored credentials + GNOME Keyring fallback.
- [x] **Share via Nextcloud extension:** `uno/share_handler.rs` — XDispatchProvider for `hearth:ShareViaNextcloud` protocol. Gets document URL, resolves to NC path (synced ~/Nextcloud/ or WebDAV), calls OCS Share API v2 (`POST /ocs/v2.php/apps/files_sharing/api/v1/shares`, shareType=3, permissions=1), copies public link to clipboard. Toolbar button + Tools menu entry via `Addons.xcu`.
- [x] **Nextcloud Comments sidebar:** `uno/comments_panel.rs` — sidebar panel (XToolPanel) with modeless dialog fallback. Resolves file ID via WebDAV PROPFIND `oc:fileid`, fetches comments via OCS API (`GET/POST /ocs/v2.php/apps/dav/api/v1/comments/files/{fileId}`). File ID cached per document URL. Shows commenter name + timestamp + message. Supports posting new comments.
- [x] **File Lock Status indicator:** `uno/lock_status.rs` — StatusbarController (with infobar/toolbar fallback). WebDAV PROPFIND with `{DAV:}lockdiscovery` property. Parses `<d:activelock>` for owner + timeout. Shows "Not locked" or "Locked by {username}" in status bar. 30-second refresh interval.
- [x] **.oxt packaging:** `nix/hearth-office-oxt.nix` — `runCommand` combining the `.so` from Crane build + XML descriptors (`nix/oxt/`: manifest.xml, description.xml, hearth-office.components, Addons.xcu, ProtocolHandler.xcu) into a ZIP archive. `home-modules/libreoffice.nix` auto-adds the .oxt to extensions when `enableExtensions = true`, switches to `pkgs.libreoffice-hearth`. `mk-fleet-host.nix` extended with `libreofficeExtensions` parameter.

**Stats:** 1 new crate (hearth-office, 11 source files ~600 lines), 6 Nix files (LO overlay + .oxt packaging), 6 OXT descriptor files, 1 VM test. Modified: flake.nix (+3 packages, overlay), overlays/default.nix (+2 entries), home-modules/libreoffice.nix (conditional LO package + auto .oxt), mk-fleet-host.nix (+libreofficeExtensions parameter).

#### 6D-4: Video Conferencing (Future)

Self-hosted video meetings integrated with chat, calendar, and Thunderbird.

- [ ] **Jitsi Meet deployment:** Jitsi Meet deployed as a Helm capability (`capabilities.meet`). Kanidm SSO via oauth2-proxy forward-auth. Prosody XMPP backend, JVB media routing.
- [ ] **Matrix integration:** Set `jitsi.preferred_domain` in Element Desktop's `config.json` (already managed in `home-modules/chat.nix`). Element has built-in first-party Jitsi widget support — video calls embed as iframes in any room. No custom extension needed.
- [ ] **Thunderbird calendar integration:** Install the existing Jitsi Meet event generator add-on (`addons.thunderbird.net`) via Thunderbird `ExtensionSettings` policy, pre-configured with the org's Jitsi server URL. One-click "Add video call" when creating calendar events. Meeting links visible in both Thunderbird and GNOME shell calendar. No custom extension needed.
- [ ] **Desktop integration:** `.desktop` launcher for Jitsi via service discovery. Service directory entry.

#### 6D-5: Document Classification & Labeling (Future)

Sensitivity labeling for documents using the open TSCP/BAILS standards, with enforcement through Nextcloud access control. Interoperable with Microsoft Information Protection (MIP) metadata for orgs that exchange documents with Microsoft shops.

- [ ] **LibreOffice TSCP toolbar:** Enable the built-in TSCP Classification toolbar via home-manager dconf/config across all role profiles. Deploy a custom `classification.xml` policy file defining the org's label taxonomy (e.g., Public, Internal, Confidential, Restricted). Labels stored as custom document properties in both ODF and OOXML formats.
- [ ] **Nextcloud enforcement pipeline:** Enable `files_confidential` (reads TSCP/BAILS metadata from uploads, auto-applies Nextcloud tags), `files_automatedtagging` (rule-based tagging by folder, user group, file type), and `files_accesscontrol` (denies access based on tag + user group combinations). Restricted tags so users cannot remove classification.
- [ ] **Visual markings:** LibreOffice macro/extension that applies header/footer text based on classification level on document open/save. Distributable via home-manager as a LibreOffice extension package.
- [ ] **MIP interoperability (optional):** Write `MSIP_Label_*` custom properties alongside BAILS properties for documents shared with Microsoft ecosystem partners. Read both formats when inspecting incoming documents.

#### 6D-6: Knowledge Base (Future)

Internal documentation and team wiki. BookStack deployed as a Helm capability — purpose-built wiki with draw.io diagrams, granular per-page permissions, WYSIWYG + Markdown editors, and full REST API. Chosen over Nextcloud Collectives for its materially better editor, diagram support, page-level permissions, and project maturity (18.5k GitHub stars, 10 years active).

- [ ] **BookStack deployment:** Helm capability (`capabilities.wiki`). BookStack Deployment (PHP/Laravel) + MariaDB (Bitnami subchart or shared instance). Kanidm SSO via `AUTH_METHOD=oidc` env vars. Auto-registration with configurable default role. Health probes, Ingress, PVC for uploads.
- [ ] **Kanidm integration:** `hearth-wiki` OAuth2 client in Kanidm bootstrap. BookStack roles mapped to Kanidm groups (hearth-users → Viewer, hearth-operators → Editor, hearth-admins → Admin).
- [ ] **Default content:** Bootstrap job creates initial shelves (Engineering, Operations, Onboarding) and a welcome page. Draw.io integration pointed at self-hosted instance or public diagrams.net.
- [ ] **Desktop integration:** Service discovery entry (`wiki` category), `.desktop` launcher via agent service bookmarks. Firefox bookmark in managed browser policy.

#### 6D-7: Password Manager (Future)

Shared team credential management via Vaultwarden (Bitwarden-compatible server).

- [ ] **Vaultwarden deployment:** Helm capability (`capabilities.passwords`). Vaultwarden Deployment (Rust, lightweight) with SQLite or PostgreSQL backend. Kanidm SSO via OpenID Connect. Ingress, PVC for data, health probes.
- [ ] **Kanidm integration:** `hearth-vault` OAuth2 client in Kanidm bootstrap. SSO login flow — users authenticate via Kanidm, Vaultwarden provisions accounts automatically.
- [ ] **Desktop integration:** Bitwarden desktop app + Firefox/Chrome extension pre-configured with self-hosted server URL via home-manager. GNOME Keyring integration for master password caching. Service discovery entry.
- [ ] **Organization vaults:** Bootstrap creates shared collections for team credentials (Infrastructure, Shared Services). Access controlled by Kanidm group membership.

#### 6D-8: Managed Browser & Printing (Future)

Fleet-managed Firefox policies and CUPS printing infrastructure.

- [x] **Firefox enterprise policy:** `home-modules/firefox.nix` + `modules/firefox.nix` — managed bookmarks auto-populated from enabled capabilities (chat, cloud, identity, monitoring), uBlock Origin force-installed, Floccus force-installed and pre-configured via `3rdparty.Extensions` managed_storage when Nextcloud is enabled (zero-touch personal bookmark sync via Nextcloud Bookmarks app), Bitwarden extension pre-wired for Vaultwarden (ready for 6D-7), homepage set to Hearth console, multi-cert trust (`internalCaCerts` list), per-role bookmarks (`roleBookmarks` by role name — admin gets Grafana automatically), DNS-over-HTTPS option, disabled telemetry/studies/Pocket. Grafana added to service directory via `HEARTH_GRAFANA_URL` (API + Helm configmap). Wired from `mk-fleet-host.nix` parameters (`grafanaUrl`, `vaultwardenUrl` added). Nextcloud Bookmarks app added to bootstrap.
- [ ] **Dynamic service bookmarks:** Currently managed bookmarks are static (set at closure build time from `mk-fleet-host.nix` parameters). The agent already writes `/var/lib/hearth/services/services.json` on each heartbeat. A future enhancement could read this at runtime and regenerate Firefox `ManagedBookmarks` policy without a closure rebuild — useful if services are added/removed frequently. Low priority since services are relatively stable (change when Helm chart is reconfigured).
- [x] **Nextcloud Bookmarks in Helm bootstrap:** The Bookmarks app is now installed in both the dev bootstrap script (`dev/nextcloud/bootstrap.sh`) and the production Helm Nextcloud bootstrap job (`templates/nextcloud/job-bootstrap.yaml`) so Floccus bookmark sync works in production deployments. Validated by full 217-test helm-unittest suite.
- [ ] **CUPS print server:** Helm capability (`capabilities.printing`) or standalone NixOS module. IPP Everywhere + driverless printing. Printer inventory managed via control plane API (name, location, model, driver). Per-location printer assignment via `extra_config` on machines.
- [ ] **Desktop print integration:** NixOS module configuring `cups-browsed` or static printer list from control plane. Users see location-appropriate printers in GNOME print dialog without manual setup. Print accounting via CUPS quotas (optional).

#### 6D-9: Fleet DNS Resolver (Future)

Encrypted, centrally-managed DNS for the fleet — internal service resolution, observability, and optional content filtering.

- [ ] **CoreDNS deployment:** Helm capability (`capabilities.dns`). CoreDNS Deployment with DoH endpoint (`/dns-query`) via `coredns-doh` plugin. Resolves internal service names (e.g., `chat.hearth.internal` → capability URLs from `values.yaml`). Forwards external queries to configurable upstream resolvers (Cloudflare, Quad9, or org-specified). Accessible only via Headscale mesh for fleet-only access. Health probes, Ingress (for DoH), PVC for optional blocklist/cache state.
- [ ] **Headscale integration:** Complements Headscale magic DNS — Headscale handles mesh device names (`machine.tailnet`), fleet resolver handles service names and policy. Configure CoreDNS as a split-horizon peer alongside Headscale's built-in DNS.
- [ ] **NixOS DNS module:** `modules/dns.nix` configures `systemd-resolved` to use the fleet resolver for all system DNS (not just Firefox). Fallback to public resolvers when off-mesh. `mk-fleet-host.nix` wires `dnsResolverUrl` parameter.
- [ ] **Firefox DoH integration:** Wire the existing `hearth.firefox.dnsOverHttps` option (`home-modules/firefox.nix`) to the fleet resolver URL via `mk-fleet-host.nix`. Locked policy ensures all Firefox DNS goes through the fleet resolver.
- [ ] **Observability:** CoreDNS Prometheus metrics exported to fleet Prometheus (ServiceMonitor). Grafana dashboard for DNS query volume, top domains, blocked queries, resolution latency. Loki integration for query logging (opt-in, privacy-sensitive).
- [ ] **Content filtering (optional):** DNS-level threat blocking via CoreDNS `hosts` or `forward` plugin with blocklist feeds (similar to Pi-hole). Blocklist management via control plane API or ConfigMap. Per-role filtering policies (e.g., stricter for default users, relaxed for developers).

---

## Operational Backlog {#operational-backlog}

Items surfaced during recent dev/debug sessions. Each carries enough context to be picked up in isolation. Use this section for follow-ups that don't belong to a specific phase — reliability fixes, test-coverage gaps, dev-loop ergonomics, and small RFCs.

### Pending

- [x] **seatd start-timeout / restart loop on cold boot.** Root cause confirmed via `tests/full-login-flow.nix`: nixpkgs' seatd module wraps the daemon with `sdnotify-wrapper` (from s6) and uses `Type = "notify"`. With systemd v259+, the wrapper's notify sequence ends with a message after `BARRIER=1`, which systemd rejects (`Extra notification messages sent with BARRIER=1, ignoring everything`) and then swallows `READY=1` — seatd's own logs (`Created VT-bound seat seat0`; `seatd started`) confirm the daemon itself is healthy, systemd just never sees it. Fail-fast (`TimeoutStartSec=30`) by itself was insufficient — the unit just looped failing every 30s. Fixed in `modules/greeter.nix` by overriding to `Type = "simple"` and dropping `sdnotify-wrapper` from `ExecStart`. seatd opens `/run/seatd.sock` synchronously in `main()` before its event loop, so the race window for downstream `After=seatd.service` consumers is sub-millisecond. Validated by VM test (`vm-full-login-flow`): seatd now reaches `active` immediately, test progresses 200+ seconds further before hitting an unrelated downstream issue.

- [x] **Cold-boot → greeter → desktop session test coverage.** `tests/full-login-flow.nix` now runs the full lifecycle end-to-end in ~78s: cold boot → `seatd.service` active (gates the cold-boot regression) → `greetd.service` active → kanidm-unixd resolves the test user → greeter does headless PAM/Kanidm auth via greetd → agent fetches a closure path from the mock API's `/env-closure` endpoint → activates `<closure>/activate` as the user → marker file appears in the user's home. Closure is a `pkgs.runCommand` stand-in registered via the existing `/api/v1/test/set-user-config` endpoint; `virtualisation.additionalPaths` stages it into the VM store so the agent's `nix-store --realise` is a no-op against the local store. Substituters disabled in the VM so the realise call doesn't burn time on cache.nixos.org. Side-quests fixed along the way: agent `preStart` needs `coreutils` in PATH; test user's kanidm `loginshell` must be `/run/current-system/sw/bin/bash` (not `/bin/bash`, which kanidm-unixd refuses as un-canonicalisable). **Still wanted (separate task):** drive an actual graphical session via `send_chars`/`get_screen_text` to exercise the cage compositor + GNOME boot path.

- [x] **kanidm-unixd `home_attr = "name"` regression on Kanidm 1.10.** Root-caused via the kanidm 1.10.3 source (`unix_integration/`): two interacting changes from 1.9 broke the `home_attr = "name"` setting. (1) `token_homedirectory()` checks `home_alias` first and only falls back to `home_attr` when alias is `None` (resolver.rs:782-785). (2) `DEFAULT_HOME_ALIAS = Some(HomeAttr::Spn)` — *omitting* `home_alias` falls back to SPN, not None (constants.rs:17). Together: any config without an explicit `home_alias` gets SPN-aliased homes regardless of `home_attr`. The previous `home_alias = "spn"` was redundant — the absence would have given the same result. Fixed in `modules/kanidm-client.nix` by setting `home_alias = "none"` (the v2 parser maps the string "none" → `Some(None)` to disable the alias). Validated by `vm-full-login-flow` strict `/home/testuser` assertion.

- [ ] **Push mechanism: control-plane → agent fast-path.** Today's loop (edit `home-modules/*.nix` → worker claim → nix evaluate flake tarball → build → sign → push to Attic → fan out to `user_environments` → 60s agent poll → fetch) is ~60-120s round-trip. Each hop hides bugs. **Dev push:** `just push-user-env <user>` or `hearth push --user <user>` builds the closure on the host and POSTs it directly to the agent's IPC socket via the existing 9p mount (or SSH:2222). Bypasses worker queue + Attic + DB. **Prod push:** SSE / WebSocket from agent → API so `complete_user_env_build` can notify affected agents within ~1s instead of waiting on a 60s poll. Auth via existing machine token; falls back to polling on disconnect. Out of scope: replacing Attic for content delivery, removing the build worker queue. **RFC drafted: [`docs/rfc-001-push-fast-path.md`](docs/rfc-001-push-fast-path.md) — awaiting review before implementation.**

- [x] **Stalwart webadmin bundle for production / air-gapped deployments.** Dev was fixed (`just setup` pre-fetches `webadmin.zip`, docker-compose bind-mounts it, `dev/stalwart/config.toml` points `[webadmin] resource = file:///...`). Helm chart now follows the same pattern: new `stalwart.webadmin.{enabled,url,image}` values, a `fetch-webadmin` init container (hardened: `runAsNonRoot`, `readOnlyRootFilesystem`, dropped caps, `RuntimeDefault` seccomp) that stages the zip into the existing `config-dir` emptyDir, and the configmap's `[webadmin] resource = file://…` block. Air-gapped clusters override `stalwart.webadmin.url` to an internal mirror. 7 new helm-unittest cases (chart suite now 217 tests).

- [x] **Renovate self-hosted workflow.** `renovate.json` is checked in (weekly schedule, semantic commits, per-manager grouping, auto-merge for cargo/npm patches, manual review for Kanidm/server image bumps, custom regex manager for `nix/kanidm-cli.nix` version pin). Self-hosted `renovatebot/github-action` workflow now lives at `.github/workflows/renovate.yml` — hourly cron probe (Renovate's own `schedule:` block constrains the actual PR-opening window to weekend mornings) plus `workflow_dispatch` for manual runs and a push trigger on `renovate.json` edits. Uses the default `GITHUB_TOKEN` with `contents`/`pull-requests`/`issues` write perms. Installing the [Renovate GitHub App](https://github.com/apps/renovate) on the repo is still the recommended path for orgs that allow it (better diagnostics, higher rate limits) — if the App is later installed, retire this workflow.

- [x] **Validate LibreOffice extension installs on stock nixpkgs LibreOffice.** `tests/libreoffice-extension.nix` is now wired into `flake.nix` `vmTests` as `vm-libreoffice-extension` and passes in ~42s. The test installs `hearth-office.oxt` via `unopkg add`, asserts `unopkg list` reports `com.hearth.office` as active, then boots `soffice --headless --calc` without crashing — proving the C++ UNO bridge's `component_getFactory` is reachable and the chained Rust `.so` loads. Two fixes were needed to make this work on stock LO: (a) drop the `<LibreOffice-minimal-version value="26.2">` dep from `nix/oxt/description.xml` (the bridge speaks the long-stable C++ UNO ABI and the Rust library is loaded over a C ABI — neither uses LO 26.2's Rust UNO bindings); (b) pre-create `/run/user/1000` via tmpfiles in the test because `su -` skips pam_systemd and LO needs `XDG_RUNTIME_DIR`. **Still wanted (separate task):** drive an interactive Nextcloud Share / Comments sidebar / Lock Status round-trip against the dev Nextcloud — needs a graphical session, not the headless soffice we can drive from a VM test.

- [x] **LibreOffice Addons.xcu "unknown node m1" warning.** Root-caused via the upstream Addons.xcs schema (`officecfg/registry/schema/org/openoffice/Office/Addons.xcs`) + `configmgr/source/xcuparser.cxx:786`. The warning was firing on the *menubar* entry, not the toolbar: `PopupMenu` is a `<group>` with named children `Title`, `Context`, and a `Submenu` set — menu items must go inside the `Submenu` wrapper, not directly under the popup id. The toolbar side was correct all along (`ToolBarItems` is a set-of-sets so toolbar items go directly under the toolbar id). Confirmed against the canonical upstream `odk/examples/python/minimal-extension/Addons.xcu` pattern. Fixed in `nix/oxt/Addons.xcu` by adding the `Submenu` wrapper plus the required `Title` prop on the popup node. `tests/libreoffice-extension.nix` now captures unopkg's stderr and asserts it contains no `unknown node` string so a regression in the schema structure fails the test (vm-libreoffice-extension still passes in ~42s).

### Recent fixes (record)

For archaeology — context for changes landed in the same session this backlog was filed:

- **nixpkgs 2026-04-14 → 2026-05-23** — fetchCargoVendor User-Agent fix (NixOS/nixpkgs#512735) bypasses crates.io's WAF blocking `python-requests/<version>`.
- **home-manager 2026-03-20 → 2026-05-30** — neovim `extraConfig` null-list regression in the in-flight upstream refactor (commits between `9670de29` and `7d8127d3`).
- **Kanidm 1.9 → 1.10.1** — 1.9 EOL 2026-05-31. Pin sites: `flake.nix` overlay (×2), `nix/kanidm-cli.nix` (version + cargoHash + src hash), `docker-compose.yml` image tag. See `CLAUDE.md` Key Conventions for the canonical pin list.
- **sqlx PgPool keepalive** — `test_before_acquire(true)` + `idle_timeout(10m)` + `max_lifetime(60m)` for both `hearth-api` and `hearth-build-worker`. Prevents silent stall when postgres / NAT drops idle TCP and sqlx's pool holds a dead socket.
- **pnpm 11 pre-script `depsStatusCheck` bypass** — `verifyDepsBeforeRun: false` in `web/pnpm-workspace.yaml`. Pre-script silently runs `pnpm install` before `pnpm run`/`exec`, which under `nix develop -c pnpm ...` has no TTY and aborts with `ERR_PNPM_ABORTED_REMOVE_MODULES_DIR_NO_TTY`.
- **attic CLI server qualifier in dev** — `ATTIC_CACHE_NAME=dev:hearth` in `justfile` dev-full + worker + push-cache. Without `dev:`, pushes leak to whatever the developer's attic `default-server` happens to be (e.g. a personal `attic.<domain>` cache). VM reads `dev:hearth` via Caddy → never meets the push.
- **kanidm-unixd `home_attr` `spn` → `name`** — keeps `@` out of `/etc/passwd`, `$HOME`, and `whoami`, which Nextcloud Desktop and several other clients mishandle. `home_alias = spn` retained so login by full SPN still works. Matching change in `flake.nix` `lib.buildUserEnv` (`home.username`/`home.homeDirectory` use the local part of the SPN).
- **Stalwart webadmin bundle pre-fetch (dev)** — `just setup` curls `webadmin.zip` into `dev/stalwart/`, docker-compose bind-mounts it at `/opt/stalwart/etc/webadmin.zip`, `config.toml` `[webadmin] resource =` points at the local file. Stalwart's GitHub download silently fails inside the container.

---

## Icebox {#icebox}

Items that are valuable but not currently prioritized. May be promoted to a phase based on user demand or strategic need.

### Closed Supply Chain (Attic-Only Substituters)
Optional mode where fleet devices use only the Hearth Attic cache as their Nix substituter — cache.nixos.org is removed. All store paths on every device were built by the org's build pipeline, signed with the org's key, and have corresponding SBOMs. Trades first-build speed for full supply chain provenance.

Benefits: supply chain control (NIST 800-53 SA-12, FedRAMP), accurate SBOM coverage for every path on every device, deterministic CVE impact queries ("which devices have this vulnerable package?" answered from Attic + deployment DB), air-gapped deployment support, single trust anchor (org signing key vs. Hydra's).

Implementation: build worker pushes full closures to Attic (already done). Fleet config toggle (`closedSupplyChain: true`) removes cache.nixos.org from `nix.settings.substituters`. Attic itself can optionally use cache.nixos.org as an upstream for warming the cache, but fleet devices never contact it directly. Helm values + `mk-fleet-host.nix` parameter.

### Conditional Access
Integrate compliance state with Kanidm's OAuth2 claims pipeline. Non-compliant devices (missed updates, failed attestation, config drift) get restricted OAuth2 tokens that block access to sensitive resources. Requires the compliance engine (Phase 5B) to exist first, and depends on Kanidm's claims-based access control maturing upstream.

### Canonical Identity Namespace
Decouple user identity from the auth backend by introducing a deployment-level identity domain (e.g., `testuser@hearth.local` instead of `testuser@kanidm.hearth.local`). The domain would come from deployment config rather than the IdP, keeping usernames stable across auth backend changes. Requires design work around capabilities disclosure (which IdP is active), multi-IdP federation, and how the namespace maps to home directories, Matrix IDs, and Nextcloud accounts. Currently single-tenant with short usernames (`testuser`); this would formalize the namespace for multi-tenant readiness.

### Multi-Tenancy
Multiple organizations sharing a single control plane deployment with isolated fleet views, RBAC boundaries, and separate Attic cache tenants. Relevant for SaaS deployment or MSP use cases. Not needed for self-hosted single-org deployments.

### Per-User Environment Customizations
Promoted to Phase 5D. Core infrastructure (DB schema, API, build pipeline, agent activation) is implemented. Remaining work: self-service UI, package allowlists, closure pre-warming.

### Fleet/osquery Integration
Deploy Fleet + osquery alongside the control plane for SQL-queryable endpoint telemetry. Custom osquery extension for Nix store package inventory. Integration layer syncing device state between Fleet and Hearth. Large integration surface — most of the value is already covered by heartbeat data and the Prometheus metrics pipeline.

### Application Updates Separate from System Updates
Flatpak for fast CVE patching of user-facing applications (browsers, office suites) on a faster cadence than full NixOS system updates. The agent already supports Flatpak installs via the software catalog — this extends it with automatic Flatpak update scheduling independent of system deployment cycles.

---

## Demo Environment {#demo-environment}

A reference deployment showcasing Hearth with the full enterprise integration stack. Not part of the Hearth product itself — these are components the org's NixOS module library would configure, packaged as a turnkey demo.

### Included in the demo stack (docker-compose + fleet VMs)
- **Control plane:** hearth-api + hearth-build-worker + PostgreSQL + Attic + Kanidm
- **Observability:** Prometheus + Grafana + Loki (with pre-built dashboards)
- **Fleet devices:** 2–3 NixOS VMs (microvm.nix) with hearth-agent, hearth-greeter, node_exporter, Promtail
- **Network storage:** NFS server with pam_mount-triggered home directory mounts
- **Printing:** CUPS server with per-location printer assignment via dconf
- **Proxy/network:** HTTP proxy + corporate CA certificate distribution
- **User data backup:** Restic backup to S3 (Garage) on a timer

### Purpose
Demonstrates the full end-to-end workflow: enrollment → first login → user environment activation → software request → deployment rollout → log search → monitoring dashboards. Provides a starting point for orgs evaluating Hearth and a reference for configuring enterprise integrations in their own module library.

---

## Repository Structure

```
hearth/
├── Cargo.toml                  # Workspace root
├── .cargo/config.toml          # Cargo settings
├── rust-toolchain.toml         # Rust toolchain pinning
├── flake.nix                   # Nix flake: crane builds, dev shell, modules, tests
├── flake.lock
├── .gitignore
├── .github/workflows/ci.yml   # CI pipeline
├── docker-compose.yml          # Local dev: PostgreSQL + Attic
├── ROADMAP.md                  # This file
├── docs/                       # Architecture documents
├── crates/
│   ├── hearth-common/          # Shared: IPC types, API client, config, nix utils
│   ├── hearth-agent/           # On-device agent (systemd service)
│   ├── hearth-greeter/         # GTK4 greetd greeter
│   ├── hearth-enrollment/      # Enrollment TUI (ratatui)
│   ├── hearth-api/             # Control plane REST API (Axum)
│   └── hearth-build-worker/    # Standalone build worker (job queue consumer)
├── migrations/                 # SQL migration files
│   ├── 001_machines.sql
│   ├── 002_user_environments.sql
│   ├── 003_deployments.sql
│   ├── 004_audit_events.sql
│   ├── 005_software_catalog.sql
│   ├── 006_deployment_machines.sql
│   ├── 008_identity.sql
│   ├── 009_build_jobs.sql
│   ├── 010_phase4_enterprise.sql
│   └── 012_hardware_and_instance_data.sql
├── modules/                    # NixOS modules
│   ├── agent.nix
│   ├── greeter.nix
│   ├── pam.nix                # PAM/NSS (authBackend: kanidm/sssd/none)
│   ├── kanidm-client.nix      # Kanidm unixd client for fleet devices
│   ├── desktop.nix
│   ├── hardening.nix
│   ├── enrollment.nix
│   ├── secure-boot.nix        # Lanzaboote Secure Boot
│   ├── tpm-fde.nix            # TPM2 full disk encryption
│   ├── logging.nix            # Promtail log forwarding to Loki
│   ├── metrics.nix            # vmagent + node_exporter metrics
│   └── roles/                  # Role-specific module compositions
├── home-modules/               # Home-manager profiles
│   ├── common.nix
│   ├── default.nix
│   ├── developer.nix
│   ├── designer.nix
│   └── admin.nix
├── overlays/                   # Nix overlays
├── hardware/                   # Hardware-specific NixOS profiles
│   ├── thinkpad-t14s.nix      # Lenovo ThinkPad T14s (AMD)
│   ├── framework-13.nix       # Framework Laptop 13 (Intel)
│   └── dell-latitude.nix      # Dell Latitude (Intel)
├── lib/
│   ├── mk-fleet-host.nix      # Parameterized host builder
│   ├── mk-enrollment-image.nix # Bootable enrollment ISO builder
│   └── disko-configs/          # Declarative disk partitioning
│       ├── standard.nix       # GPT + EFI + ext4
│       └── luks-lvm.nix       # GPT + EFI + LUKS + LVM
├── data/                       # Static assets (CSS, SVG)
├── tests/                      # NixOS VM tests (CI, hermetic)
│   ├── agent-polling.nix
│   ├── desktop-baseline.nix
│   ├── full-enrollment.nix
│   ├── user-login-flow.nix
│   └── offline-fallback.nix
├── web/                        # pnpm monorepo (frontend)
│   ├── packages/ui/            # @hearth/ui shared design system
│   ├── apps/catalog/           # @hearth/catalog Software Center SPA
│   └── apps/console/           # @hearth/console Admin Console SPA
├── chart/
│   └── hearth-home/            # Helm chart for Hearth Home Cluster
│       ├── Chart.yaml          # Metadata + subchart dependencies
│       ├── values.yaml         # Default values (capabilities model)
│       ├── values-production.yaml # Production overlay example
│       ├── templates/          # K8s manifests (api, attic, kanidm, headscale, build-worker, observability, tests)
│       ├── tests/              # helm-unittest test suites (105 tests)
│       └── ci/ct-values.yaml   # Minimal values for Kind smoke test
├── deploy/                     # Observability config
│   ├── grafana/
│   │   └── fleet-overview.json # Pre-built Grafana dashboard
│   └── promtail-config.yml    # Standard Promtail config for fleet
└── dev/                        # microvm.nix (interactive dev)
    ├── fleet-vm.nix
    ├── enrollment-vm.nix
    └── kanidm/                 # Kanidm dev stack
        ├── server.toml         # Kanidm server config
        └── bootstrap.sh        # Idempotent provisioning script
```

---

## CI Pipeline

Every PR: `nix flake check` (includes Rust builds, clippy, fmt, nextest, VM integration tests, and Helm chart lint/kubeconform on Linux) + `sqlx prepare --check`. Helm chart changes additionally trigger `.github/workflows/helm.yml` (helm-unittest, kubeconform, ct install on Kind).

Merges to main: additionally push to Attic.

---

## Dev Environment

### Local Stack (docker-compose)
- PostgreSQL 16 → port 5432
- Attic → port 8080 (binary cache, local FS storage)
- Kanidm → port 8443 (identity provider, self-signed TLS)
- Loki → port 3100 (log aggregation)
- Grafana → port 3001 (dashboards, pre-provisioned with Prometheus + Loki datasources)
- API server runs natively via `cargo run -p hearth-api`
- Build worker runs natively via `cargo run -p hearth-build-worker`

### nix develop Shell
Rust stable, cargo, clippy, rustfmt, rust-analyzer, sqlx-cli, GTK4 dev libs, pkg-config, nix-eval-jobs, attic-client, cargo-nextest, cargo-watch, docker-compose, kubernetes-helm, chart-testing, kubeconform, kind, jq, httpie

### NixOS VM Testing
- **nixos-test (CI):** QEMU VMs, multi-node, `nix flake check`
- **microvm.nix (dev):** Firecracker/Cloud Hypervisor, sub-second boot, bridged to host
