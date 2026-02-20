# Hearth development workflow recipes

# Default recipe: show available commands
default:
    @just --list

# One-time dev environment setup
setup:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "==> Starting infrastructure..."
    docker compose up -d
    echo "==> Waiting for services..."
    until docker compose exec -T postgres pg_isready -U hearth >/dev/null 2>&1; do sleep 1; done
    echo "    PostgreSQL ready"
    until docker compose exec -T attic wget -q --spider http://localhost:8080/ >/dev/null 2>&1; do sleep 2; done
    echo "    Attic ready"
    until curl -sf --insecure https://localhost:8443/status >/dev/null 2>&1; do sleep 2; done
    echo "    Kanidm ready"
    echo "==> Configuring Attic cache..."
    TOKEN=$(docker compose exec -T attic atticadm make-token \
        --config /etc/attic/server.toml \
        --sub "dev" \
        --validity "10y" \
        --pull '*' --push '*' --create-cache '*' --delete '*' \
        2>/dev/null || echo "")
    if [ -n "$TOKEN" ]; then
        attic login dev http://localhost:8080 "$TOKEN" 2>/dev/null
        attic cache create hearth 2>/dev/null || true
        echo "    Attic cache 'hearth' ready"
    else
        echo "    WARNING: Could not create Attic token (is atticd running?)"
    fi
    echo "==> Bootstrapping Kanidm identity provider..."
    bash dev/kanidm/bootstrap.sh
    echo "==> Running database migrations..."
    sqlx migrate run
    echo "==> Building web frontends..."
    cd web && pnpm install && pnpm build
    cd ..
    echo ""
    echo "=== Setup complete! ==="
    echo ""
    echo "  just dev       — Start API server (with Kanidm auth)"
    echo "  just worker    — Start build worker"
    echo "  just build-iso — Build enrollment ISO"
    echo "  just enroll    — Boot enrollment ISO in QEMU"
    echo "  just check     — Run all checks"

# Start the API server (sources Kanidm auth config from dev/kanidm/.env)
dev:
    #!/usr/bin/env bash
    set -euo pipefail
    if [ -f dev/kanidm/.env ]; then
        set -a; source dev/kanidm/.env; set +a
        echo "Loaded auth config from dev/kanidm/.env (auth enabled)"
    else
        echo "WARNING: dev/kanidm/.env not found — auth disabled. Run 'just setup' first."
    fi
    exec cargo run -p hearth-api

# Start API server with file watching
dev-watch:
    #!/usr/bin/env bash
    set -euo pipefail
    if [ -f dev/kanidm/.env ]; then
        set -a; source dev/kanidm/.env; set +a
        echo "Loaded auth config from dev/kanidm/.env (auth enabled)"
    else
        echo "WARNING: dev/kanidm/.env not found — auth disabled. Run 'just setup' first."
    fi
    exec cargo watch -x 'run -p hearth-api'

# Start a build worker
worker:
    cargo run -p hearth-build-worker

# Start build worker with file watching
worker-watch:
    cargo watch -x 'run -p hearth-build-worker'

# Start infrastructure services
infra:
    docker compose up -d

# Stop infrastructure services
infra-down:
    docker compose down

# Run database migrations
migrate:
    sqlx migrate run

# Boot enrollment ISO in QEMU for testing
enroll:
    bash dev/run-enrollment.sh

# Run the pre-built fleet VM
fleet-vm:
    nix run .#fleet-vm

# Run all checks (clippy, fmt, tests)
check:
    cargo clippy --workspace -- --deny warnings
    cargo fmt --check
    cargo test --workspace

# Run tests only
test *ARGS:
    cargo test --workspace {{ARGS}}

# Build web frontends
web-build:
    cd web && pnpm install && pnpm build

# Run both web dev servers (catalog :5173, console :5174)
web-dev:
    cd web && pnpm dev

# Run only catalog dev server
web-dev-catalog:
    cd web && pnpm dev:catalog

# Run only console dev server
web-dev-console:
    cd web && pnpm dev:console

# Typecheck web frontends
web-check:
    cd web && pnpm typecheck

# Build enrollment ISO
build-iso:
    nix build .#enrollment-iso

# Push a Nix closure to the local Attic cache
cache-push PATH:
    attic push hearth {{PATH}}

