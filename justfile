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
    echo "==> Running database migrations..."
    sqlx migrate run
    echo "==> Building web frontends..."
    cd web && pnpm install && pnpm build
    cd ..
    echo "==> Starting API server for role registration..."
    cargo build -p hearth-api
    cargo run -p hearth-api &
    API_PID=$!
    # Wait for API to be ready
    until curl -sf http://localhost:3000/healthz >/dev/null 2>&1; do sleep 1; done
    echo "    API server ready"
    echo "==> Building role templates..."
    just build-roles
    echo "==> Building enrollment ISO..."
    nix build .#enrollment-iso
    # Stop the temporary API server
    kill $API_PID 2>/dev/null || true
    wait $API_PID 2>/dev/null || true
    echo ""
    echo "=== Setup complete! ==="
    echo ""
    echo "  just dev       — Start API server"
    echo "  just enroll    — Boot enrollment ISO in QEMU"
    echo "  just fleet-vm  — Run pre-built fleet VM"
    echo "  just check     — Run all checks"

# Start the API server (with auto-reload)
dev:
    cargo run -p hearth-api

# Start API server with file watching
dev-watch:
    cargo watch -x 'run -p hearth-api'

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

# Build all role template closures, push to Attic, and register in the API
build-roles:
    #!/usr/bin/env bash
    set -euo pipefail
    ROLES="default developer designer admin"
    for role in $ROLES; do
        echo "==> Building role-template-${role}..."
        CLOSURE=$(nix build ".#role-template-${role}" --print-out-paths --no-link)
        echo "    Built: ${CLOSURE}"
        echo "    Pushing to Attic cache..."
        attic push hearth "$CLOSURE"
        echo "    Registering in API..."
        curl -sf -X PUT http://localhost:3000/api/v1/role-closures \
            -H 'Content-Type: application/json' \
            -d "{\"role\": \"${role}\", \"closure\": \"${CLOSURE}\"}"
        echo ""
        echo "    Done: ${role} -> ${CLOSURE}"
    done
    echo ""
    echo "=== All role templates built and registered ==="
