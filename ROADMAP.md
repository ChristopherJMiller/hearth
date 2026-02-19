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

## Phase 4: Enterprise Hardening {#phase-4}

Items from `docs/pieces-to-fill-in.txt`:

- [ ] TPM device identity + attestation, disk encryption key escrow
- [ ] Printing/CUPS per-location, WiFi/802.1X profile distribution
- [ ] SIEM/log forwarding (journald → Loki), compliance modules (STIG/CIS mapping)
- [ ] Device decommissioning (remote wipe, Headscale revocation)
- [ ] Proxy/corporate network config, user data backup, multi-monitor handling
- [ ] Headscale mesh integration, performance at 500+ scale
- [ ] Application updates separate from system updates (Flatpak for fast CVE patching)

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
│   └── hearth-api/             # Control plane REST API (Axum)
├── migrations/                 # SQL migration files
│   ├── 001_machines.sql
│   ├── 002_user_environments.sql
│   ├── 003_deployments.sql
│   ├── 004_audit_events.sql
│   ├── 005_software_catalog.sql
│   └── 006_deployment_machines.sql
├── modules/                    # NixOS modules
│   ├── agent.nix
│   ├── greeter.nix
│   ├── pam.nix
│   ├── desktop.nix
│   ├── hardening.nix
│   ├── enrollment.nix
│   └── roles/                  # Role-specific module compositions
├── home-modules/               # Home-manager profiles
│   ├── common.nix
│   ├── default.nix
│   ├── developer.nix
│   ├── designer.nix
│   └── admin.nix
├── overlays/                   # Nix overlays
├── lib/
│   ├── mk-fleet-host.nix      # Parameterized host builder
│   └── mk-enrollment-image.nix # Bootable enrollment ISO builder
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
└── dev/                        # microvm.nix (interactive dev)
    ├── fleet-vm.nix
    └── enrollment-vm.nix
```

---

## CI Pipeline

Every PR: `nix flake check` (includes Rust builds, clippy, fmt, nextest, and VM integration tests on Linux) + `sqlx prepare --check`

Merges to main: additionally push to Attic.

---

## Dev Environment

### Local Stack (docker-compose)
- PostgreSQL 16 → port 5432
- Attic → port 8080 (binary cache, local FS storage)
- API server runs natively via `cargo run -p hearth-api`

### nix develop Shell
Rust stable, cargo, clippy, rustfmt, rust-analyzer, sqlx-cli, GTK4 dev libs, pkg-config, nix-eval-jobs, attic-client, cargo-nextest, cargo-watch, docker-compose, jq, httpie

### NixOS VM Testing
- **nixos-test (CI):** QEMU VMs, multi-node, `nix flake check`
- **microvm.nix (dev):** Firecracker/Cloud Hypervisor, sub-second boot, bridged to host
