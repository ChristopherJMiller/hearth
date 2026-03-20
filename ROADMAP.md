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
- [x] **Agent per-user closure activation:** Agent queries control plane for pre-built closure at login, falls back to role template via home-manager. Heartbeat pre-stages closures from cache.
- [x] **Systemd socket activation:** Agent IPC socket managed by systemd with correct greeter permissions.
- [x] **Greeter fixes:** File-based password delivery for headless tests, HEARTH_GREETER_LOG_FILE support, proper Kanidm TLS cert chain.
- [x] **Agent home directory creation:** Agent creates user home directories (via getent + mkdir + chown) before environment activation, fixing the ordering issue where pam_mkhomedir runs after the agent prepares the environment.
- [x] **Refactored activation paths:** Extracted shared `run_as_user()` and `ensure_home_dir()` helpers in agent ipc.rs, eliminating duplicated runuser/shutdown logic across pre-built closure and role template branches.
- [x] **Full login flow VM test:** Fixed dynamic home directory resolution from getent passwd (handles kanidm-unixd UUID-based home_attr), mock home-manager activation marker verification works end-to-end.
- [x] **homeConfigurations flake output:** CI-verifiable home-manager configurations for all four roles (default, developer, designer, admin). Fixed home-manager deprecation warnings (git.extraConfig → git.settings, git.delta → programs.delta, ssh.extraConfig → ssh.matchBlocks).
- [x] **Package allowlist/denylist:** `HEARTH_PACKAGE_ALLOWLIST` env var restricts `extra_packages` in user overrides (API returns 400 listing disallowed packages). Nix-side defense-in-depth: `buildUserEnv` filters `package_denylist` field.
- [x] **Self-service config UI:** `/api/v1/me/config` GET/PUT endpoints with `UserIdentity` auth (restricted fields: git config, editor, shell aliases, session variables). Admin-only fields preserved on merge. Settings page at `/settings` with key-value editors, visible to all users. `AuthClaims::username()` method consolidates duplicated username extraction.

### 5E: User Environment Polish (Future)

- [ ] **Closure pre-warming:** When a machine enrolls or changes role, the control plane enumerates likely users (from Kanidm group membership for the assigned role) and queues pre-builds of their per-user closures. Reduces first-login latency from "1–3 minute build" to "15–60 second cache pull."
- [ ] **WiFi/802.1X certificate distribution:** The control plane provisions 802.1X machine certificates as part of enrollment secrets. The NixOS module configures `wpa_supplicant` or `iwd` with the certificate and network profile. Certificates rotate via the control plane's secret management.

### 5F: Scale (Future)

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

#### 6D-2: Calendar & Contacts (Future)

Desktop integration for Nextcloud's built-in CalDAV/CardDAV services. GNOME's native apps (Calendar, Contacts) become the interface — no extra web UIs needed.

- [ ] **GNOME Online Accounts:** Home-manager module configuring a Nextcloud GNOME Online Account entry. Single sign-on populates Calendar, Contacts, and Files automatically. Requires `gnome-online-accounts` with Nextcloud provider.
- [ ] **GNOME Calendar + Contacts:** Add `gnome-calendar` and `gnome-contacts` to role profiles. Pre-configured via GNOME Online Accounts — no manual CalDAV/CardDAV URL entry needed.
- [ ] **Shared organizational calendar:** Nextcloud bootstrap creates default shared calendars (company holidays, all-hands). Auto-subscribed for all users via Nextcloud group calendar sharing.

#### 6D-3: Collaborative Document Editing (Future)

Real-time co-editing of documents, spreadsheets, and slides inside Nextcloud — the Google Docs/Sheets/Slides equivalent.

- [ ] **Collabora Online integration:** Collabora Online (LibreOffice-based) deployed as a Helm capability (`capabilities.office`). Nextcloud `richdocuments` app connects to Collabora for browser-based collaborative editing. Supports ODF, OOXML, and PDF export. Kanidm SSO via Nextcloud session.
- [ ] **Desktop integration:** LibreOffice on fleet devices already works with synced ~/Nextcloud files. Collabora handles browser-based multi-user editing; LibreOffice handles offline/local editing. No conflict — files sync via Nextcloud Desktop client.

#### 6D-4: Video Conferencing (Future)

Self-hosted video meetings integrated with chat and calendar.

- [ ] **Jitsi Meet deployment:** Jitsi Meet deployed as a Helm capability (`capabilities.meet`). Kanidm SSO via oauth2-proxy forward-auth. Prosody XMPP backend, JVB media routing.
- [ ] **Matrix integration:** Element Desktop configured with Jitsi widget for in-chat video calls. Click-to-call from any Matrix room.
- [ ] **Calendar integration:** Nextcloud calendar events can embed Jitsi Meet links. Meeting URLs auto-generated on event creation.
- [ ] **Desktop integration:** `.desktop` launcher for Jitsi via service discovery. GNOME Calendar shows meeting links inline.

---

## Icebox {#icebox}

Items that are valuable but not currently prioritized. May be promoted to a phase based on user demand or strategic need.

### Conditional Access
Integrate compliance state with Kanidm's OAuth2 claims pipeline. Non-compliant devices (missed updates, failed attestation, config drift) get restricted OAuth2 tokens that block access to sensitive resources. Requires the compliance engine (Phase 5B) to exist first, and depends on Kanidm's claims-based access control maturing upstream.

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
