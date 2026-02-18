# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What is Hearth?

Hearth is an enterprise NixOS desktop fleet management platform. It manages device enrollment, configuration deployment, software catalog, and user environments across a fleet of NixOS workstations.

## Build & Development Commands

Enter the dev shell first: `nix develop` (provides Rust toolchain, sqlx-cli, GTK4 libs, pnpm, etc.)

### Rust (workspace root)

- **Build all:** `cargo build --workspace`
- **Build one crate:** `cargo build -p hearth-agent`
- **Test all:** `cargo test --workspace` (or `cargo nextest run --workspace`)
- **Test one crate:** `cargo test -p hearth-common`
- **Clippy:** `cargo clippy --workspace -- --deny warnings`
- **Format check:** `cargo fmt --check`
- **Watch:** `cargo watch -x 'test --workspace'`
- **Run API server:** `cargo run -p hearth-api` (needs PostgreSQL, see below)

### Frontend (web/)

- **Install deps:** `cd web && pnpm install`
- **Dev server:** `cd web && pnpm dev` (runs @hearth/catalog Vite dev server)
- **Build:** `cd web && pnpm build` (builds @hearth/ui first, then @hearth/catalog)
- **Typecheck:** `cd web && pnpm typecheck`

### Database

- **Start PostgreSQL:** `docker-compose up -d postgres`
- **Run migrations:** `sqlx migrate run` (from workspace root, reads `migrations/`)
- **Connection:** `postgres://hearth:hearth@localhost:5432/hearth`

### Nix

- **Full CI check:** `nix flake check`
- **Build a package:** `nix build .#hearth-agent`

## Architecture

### Rust Crates (crates/)

- **hearth-common** — Shared library: IPC message types, API client trait + reqwest impl, config structs, API request/response types, Nix store path utilities
- **hearth-agent** — On-device systemd service: polls control plane for target state, sends heartbeats, runs IPC server (Unix socket) for greeter, handles software installs (Flatpak), compares system closures. Uses `CancellationToken` for coordinated shutdown.
- **hearth-api** — Control plane: Axum REST API on port 3000, PostgreSQL via sqlx with compile-time checked queries (offline mode via `.sqlx/`), auto-runs migrations on startup. Routes at `/api/v1/{machines,heartbeat,catalog,requests}`. Serves the catalog SPA at `/catalog`.
- **hearth-greeter** — GTK4 greetd greeter (stub)
- **hearth-enrollment** — ratatui TUI for device enrollment (stub)

### Frontend (web/)

pnpm monorepo with two packages:
- **@hearth/ui** (`web/packages/ui/`) — shared design system (tokens, components)
- **@hearth/catalog** (`web/apps/catalog/`) — Software Center SPA (React 19, Vite 6, TanStack Query v5)

### NixOS Integration

- **modules/** — NixOS modules (agent, greeter, pam/greetd, desktop/GNOME, hardening, enrollment, roles)
- **home-modules/** — Home-manager role profiles (common, default, developer, designer, admin)
- **lib/mk-fleet-host.nix** — Helper to build a fleet host NixOS configuration
- **overlays/** — Adds hearth packages to nixpkgs
- **tests/** — NixOS VM integration tests (QEMU-based, run via `nix flake check`)

### Database

PostgreSQL with 5 migration files in `migrations/`. Key tables: `machines`, `user_environments`, `deployments`, `audit_events`, `software_catalog` + `software_requests`. Uses custom enums (enrollment_status, deploy_status, etc.).

## Key Conventions

- **Linker:** mold via clang on Linux (configured in `.cargo/config.toml`)
- **sqlx offline mode:** `SQLX_OFFLINE=true` is set by default so builds work without a live database. The `.sqlx/` directory stores query metadata for compile-time checking.
- **Rust edition:** 2024
- **Logging:** `tracing` crate with `RUST_LOG` env var (default: `info`)
- **CI checks:** clippy with `--deny warnings`, cargo fmt, sqlx prepare --check, pnpm typecheck + build
