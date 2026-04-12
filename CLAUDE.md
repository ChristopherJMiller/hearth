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

- **Kanidm version:** Pinned to **1.9** globally. Docker uses `kanidm/server:1.9.0`, NixOS modules use `pkgs.kanidm` which resolves to `kanidm_1_9` via the overlay in `flake.nix`. The pin lives in two places: `flake.nix` (inline overlay in `pkgs` definition + `overlays.default`) and `docker-compose.yml` (image tag). Update both together.
- **Linker:** mold via clang on Linux (configured in `.cargo/config.toml`)
- **sqlx offline mode:** `SQLX_OFFLINE=true` is set by default so builds work without a live database. The `.sqlx/` directory stores query metadata for compile-time checking.
- **Rust edition:** 2024
- **Logging:** `tracing` crate with `RUST_LOG` env var (default: `info`)
- **CI checks:** clippy with `--deny warnings`, cargo fmt, sqlx prepare --check, pnpm typecheck + build

## Security Development Guidelines

Hearth ships security/compliance tooling — see `SECURITY.md`,
`docs/threat-model.md`, and `docs/compliance-registry.md`. When making
changes that affect security, follow these rules.

### Auth requirements

- Every new API route MUST use one of the 5 auth extractors from
  `crates/hearth-api/src/auth.rs`: `UserIdentity`, `MachineIdentity`,
  `OptionalIdentity`, `OperatorIdentity`, or `AdminIdentity`.
- Write operations on fleet state (deployments, approvals, policies)
  use `AdminIdentity` or `OperatorIdentity`, not `UserIdentity`.
- Intentionally unauthenticated routes must have a comment explaining
  why (e.g., `/health`, `/metrics`, the enrollment polling endpoint).
- Dev mode grants a dev-admin identity when `KANIDM_OIDC_ISSUER` is
  unset. Never deploy to production without this env var set.

### Input validation and safety

- Use `sqlx` compile-time checked queries (`query!`, `query_as!`).
  Never construct SQL strings manually.
- Validate file paths to prevent directory traversal — follow the
  pattern in `crates/hearth-api/src/routes/compliance.rs` (`serve_sbom_file`).
- Avoid `.unwrap()` / `.expect()` in request handlers — panics are DoS
  vectors. `.expect()` at startup on required config is fine.
- New endpoints that accept JSON should set a body size limit via
  `axum::extract::DefaultBodyLimit`.

### Secret management

- Secrets come from environment variables in production, never
  hardcoded.
- In Helm: use `existingSecret` references for production; the chart
  supports auto-generated secrets for dev.
- Machine tokens are HS256 JWTs whose SHA-256 hash is stored in the
  `machines.machine_token_hash` column for revocation.
- The `.danger_accept_invalid_certs(true)` pattern is for dev
  self-signed Kanidm certs only — do not extend it to new call sites.

### Compliance controls

- New NixOS compliance controls go in `modules/compliance/` following
  the `cis-1-1-1.nix` template (`enable` option + read-only `meta`
  attribute with `{id, title, severity, description, family,
  benchmark}`).
- Wire new controls into `modules/compliance/default.nix` under both
  `imports` and the appropriate profile's `mkIf` block.
- Update `docs/compliance-controls.yaml` when adding, changing, or
  promoting controls.
- Run `/compliance-audit cis-level1` in Claude Code to check framework
  coverage.

### Helm chart security

- All new Deployment / StatefulSet / Job templates MUST include a
  container-level `securityContext` with: `allowPrivilegeEscalation:
  false`, `readOnlyRootFilesystem: true`, `runAsNonRoot: true`,
  `capabilities: { drop: [ALL] }`, and `seccompProfile: { type:
  RuntimeDefault }`.
- Secrets must never appear in ConfigMaps — use `Secret` +
  `secretKeyRef` or `envFrom.secretRef`.
- New services should ship with a corresponding `NetworkPolicy`
  template.

### Using the security agents and skills

Claude Code agents live in `.claude/agents/` and slash commands in
`.claude/commands/`. Use them as part of the dev loop:

- `/security-review` — before submitting PRs that touch auth,
  enrollment, routes, or infrastructure
- `/compliance-audit <framework>` — when adding controls or checking
  posture (`cis-level1` is the primary target)
- `/threat-model <component>` — when adding new data flows, trust
  boundary crossings, or handlers with significant attack surface
- `/hardening-check <scope>` — when modifying Helm templates or NixOS
  modules

Known accepted risks are tracked in `docs/threat-model.md` — consult
it before re-flagging them.
