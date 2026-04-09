# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What is Hearth?

Hearth is an enterprise NixOS desktop fleet management platform. It manages device enrollment, configuration deployment, software catalog, and user environments across a fleet of NixOS workstations.

## Build & Development Commands

Enter the dev shell first: `nix develop` (provides Rust toolchain, sqlx-cli, GTK4 libs, pnpm, etc.)
Or run a single command without entering the shell: `nix develop -c <command>`

A `justfile` provides common workflows — run `just` to list all recipes.

### Quick Start

- **First-time setup:** `just setup` (starts infra, runs migrations, bootstraps Kanidm, builds web)
- **Run API server:** `just dev` (loads Kanidm auth config automatically)
- **Run build worker:** `just worker`
- **Run all checks:** `just check` (clippy + fmt + tests)

### Rust (workspace root)

- **Build all:** `cargo build --workspace`
- **Build one crate:** `cargo build -p hearth-agent`
- **Test all:** `cargo test --workspace` (or `cargo nextest run --workspace`)
- **Test one crate:** `cargo test -p hearth-common`
- **Run a single test:** `cargo test -p hearth-api test_name`
- **Integration tests:** `cargo test --workspace -- --ignored` (needs live PostgreSQL via `DATABASE_URL`)
- **Clippy:** `cargo clippy --workspace -- --deny warnings`
- **Format check:** `cargo fmt --check`
- **Watch:** `cargo watch -x 'test --workspace'`

### Frontend (web/)

- **Install deps:** `cd web && pnpm install`
- **Dev server:** `cd web && pnpm dev` (runs @hearth/web Vite dev server on port 5174)
- **Build:** `cd web && pnpm build` (builds @hearth/ui first, then @hearth/web)
- **Typecheck:** `cd web && pnpm typecheck`

### Database

- **Start infrastructure:** `just infra` (PostgreSQL, Attic cache, Kanidm)
- **Run migrations:** `just migrate` (or `sqlx migrate run`)
- **Connection:** `postgres://hearth:hearth@localhost:5432/hearth`

### Nix

- **Full CI check:** `nix flake check` (runs all checks including VM tests — slow)
- **Build a package:** `nix build .#hearth-agent`
- **Build enrollment ISO:** `just build-iso`

### NixOS VM Tests (tests/)

QEMU-based integration tests that spin up full NixOS VMs. These are part of `nix flake check` and can be built individually:

- **Run all VM tests:** `nix flake check` (includes all checks)
- **Run a single VM test:** `nix build .#checks.x86_64-linux.vm-agent-heartbeat` (replace with test name)

Available tests: `vm-agent-polling`, `vm-desktop-baseline`, `vm-full-enrollment`, `vm-agent-heartbeat`, `vm-offline-fallback`, `vm-agent-system-switch`, `vm-kanidm-auth`, `vm-full-login-flow`. Test sources are in `tests/`, each is a NixOS test module imported in `flake.nix` under `vmTests`.

### Helm Chart (chart/hearth-home/)

The "Hearth Home Cluster" Helm chart deploys the control plane and all supporting services with a capabilities toggle model.

- **Lint:** `just helm-lint`
- **Unit tests:** `just helm-test` (105 tests via helm-unittest)
- **Schema validation:** `just helm-validate` (kubeconform)
- **All chart checks:** `just helm-check`
- **Update dependencies:** `just helm-deps`

Capabilities are toggled in `values.yaml`: `identity` (Kanidm), `mesh` (Headscale), `builds` (build-worker), `chat` (Synapse/Matrix), `cloud` (Nextcloud), `observability` (Grafana/Loki/Prometheus). Chart lint + kubeconform also runs as part of `nix flake check` via the `helmChartLint` check.

## Architecture

### Rust Crates (crates/)

- **hearth-common** — Shared library: IPC message types, API client trait + reqwest impl, config structs, API request/response types, Nix store path utilities
- **hearth-agent** — On-device systemd service: polls control plane for target state, sends heartbeats, runs IPC server (Unix socket) for greeter, handles software installs (Flatpak), compares system closures. Uses `CancellationToken` for coordinated shutdown.
- **hearth-api** — Control plane: Axum REST API on port 3000, PostgreSQL via sqlx with compile-time checked queries (offline mode via `.sqlx/`), auto-runs migrations on startup. Routes at `/api/v1/{machines,heartbeat,catalog,requests}`. Serves the unified web SPA as a fallback for all non-API routes.
- **hearth-build-worker** — Polls PostgreSQL job queue, evaluates flakes, builds derivations, pushes to Attic cache
- **hearth-greeter** — GTK4 greetd greeter (stub)
- **hearth-enrollment** — ratatui TUI for device enrollment (stub)

### Frontend (web/)

pnpm monorepo with two packages:
- **@hearth/ui** (`web/packages/ui/`) — shared design system (tokens, components)
- **@hearth/web** (`web/apps/hearth/`) — unified web app: software catalog (user-facing) + admin console (React 19, Vite 6, TanStack Router, TanStack Query v5, OIDC auth via oidc-client-ts)

### NixOS Integration

- **modules/** — NixOS modules (agent, greeter, pam/greetd, desktop/GNOME, hardening, enrollment, roles)
- **home-modules/** — Home-manager role profiles (common, default, developer, designer, admin)
- **lib/mk-fleet-host.nix** — Helper to build a fleet host NixOS configuration
- **overlays/** — Adds hearth packages to nixpkgs
- **tests/** — NixOS VM integration tests (QEMU-based, run via `nix flake check`)

### Helm Chart (chart/hearth-home/)

Deploys the "Hearth Home Cluster" — all control plane services. Uses a capabilities model:
- **Core (always):** hearth-api, Attic binary cache, PostgreSQL (Bitnami subchart)
- **identity:** Kanidm (StatefulSet, bootstrap Job, TLS)
- **mesh:** Headscale (Deployment, PVC)
- **builds:** hearth-build-worker (Deployment, Nix store PVC)
- **chat:** Synapse/Matrix (Element Web frontend)
- **cloud:** Nextcloud (file storage/collaboration)
- **observability:** Grafana, Loki, Prometheus (subcharts)

Tests: 12 test suites, 105 unit tests (helm-unittest), kubeconform schema validation, ct smoke test in CI.

### Database

PostgreSQL with migration files in `migrations/`. Key tables: `machines`, `user_environments`, `deployments`, `audit_events`, `software_catalog` + `software_requests`. Uses custom enums (enrollment_status, deploy_status, etc.).

## Key Conventions

- **Linker:** mold via clang on Linux (configured in `.cargo/config.toml`)
- **sqlx offline mode:** `SQLX_OFFLINE=true` is set by default so builds work without a live database. The `.sqlx/` directory stores query metadata for compile-time checking.
- **Rust edition:** 2024
- **Logging:** `tracing` crate with `RUST_LOG` env var (default: `info`)
- **CI checks:** clippy with `--deny warnings`, cargo fmt, sqlx prepare --check, pnpm typecheck + build
