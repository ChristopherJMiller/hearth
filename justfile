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
    echo "==> Waiting for Synapse..."
    until curl -sf http://localhost:8008/health >/dev/null 2>&1; do sleep 2; done
    echo "    Synapse ready"
    echo "==> Bootstrapping Matrix chat..."
    bash dev/synapse/bootstrap.sh
    echo "==> Waiting for Nextcloud..."
    until curl -sf http://localhost:8089/status.php >/dev/null 2>&1; do sleep 2; done
    echo "    Nextcloud ready"
    echo "==> Bootstrapping Nextcloud cloud storage..."
    bash dev/nextcloud/bootstrap.sh
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

# Run web dev server (:5174)
web-dev:
    cd web && pnpm dev

# Typecheck web frontends
web-check:
    cd web && pnpm typecheck

# Build enrollment ISO
build-iso:
    nix build .#enrollment-iso

# Set up Headscale mesh VPN for development
headscale-setup:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "==> Creating Headscale user..."
    docker compose exec -T headscale headscale users create hearth 2>/dev/null || echo "    User 'hearth' already exists"
    echo "==> Generating API key..."
    API_KEY=$(docker compose exec -T headscale headscale apikeys create --expiration 8760h 2>/dev/null)
    echo ""
    echo "=== Headscale ready ==="
    echo ""
    echo "Add to your environment or dev/kanidm/.env:"
    echo "  export HEADSCALE_URL=http://localhost:8085"
    echo "  export HEADSCALE_API_KEY=$API_KEY"

# Set up Matrix/Synapse chat for development
matrix-setup:
    bash dev/synapse/bootstrap.sh

# Set up Nextcloud cloud storage for development
nextcloud-setup:
    bash dev/nextcloud/bootstrap.sh

# Push a Nix closure to the local Attic cache
cache-push PATH:
    attic push hearth {{PATH}}

# ===== Helm chart recipes =====

# Lint the Helm chart
helm-lint:
    helm lint chart/hearth-home --strict

# Run Helm chart unit tests (helm-unittest)
helm-test:
    helm unittest chart/hearth-home

# Validate rendered manifests against K8s schemas
helm-validate:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "==> Validating default values..."
    helm template hearth chart/hearth-home \
        --set capabilities.observability=false \
        | kubeconform -strict -ignore-missing-schemas -kubernetes-version 1.29.0
    echo "    Default values OK"
    echo "==> Validating with all capabilities..."
    helm template hearth chart/hearth-home \
        --set capabilities.observability=false \
        --set capabilities.identity=true \
        --set capabilities.mesh=true \
        --set capabilities.builds=true \
        --set capabilities.chat=true \
        --set capabilities.cloud=true \
        | kubeconform -strict -ignore-missing-schemas -kubernetes-version 1.29.0
    echo "    All capabilities OK"
    echo "==> Validating minimal (core only)..."
    helm template hearth chart/hearth-home \
        --set capabilities.identity=false \
        --set capabilities.mesh=false \
        --set capabilities.builds=false \
        --set capabilities.observability=false \
        | kubeconform -strict -ignore-missing-schemas -kubernetes-version 1.29.0
    echo "    Core only OK"

# Run all Helm chart checks (lint + unittest + kubeconform)
helm-check:
    just helm-lint
    just helm-test
    just helm-validate

# Update Helm chart dependencies
helm-deps:
    cd chart/hearth-home && helm dependency update

# ===== Hearth Home Cluster (Kind) =====

# Create a Kind cluster and deploy the Hearth Home Cluster
helm-up:
    #!/usr/bin/env bash
    set -euo pipefail
    CLUSTER=hearth-home
    if kind get clusters 2>/dev/null | grep -q "^${CLUSTER}$"; then
        echo "Kind cluster '${CLUSTER}' already exists"
    else
        echo "==> Creating Kind cluster '${CLUSTER}'..."
        kind create cluster --name "${CLUSTER}" --wait 60s
    fi
    echo "==> Updating Helm chart dependencies..."
    helm dependency update chart/hearth-home
    echo "==> Installing Hearth Home Cluster..."
    helm upgrade --install hearth chart/hearth-home \
        --create-namespace --namespace hearth \
        --set capabilities.observability=false \
        --wait --timeout 300s
    echo ""
    echo "=== Hearth Home Cluster is running ==="
    echo ""
    echo "  kubectl --context kind-${CLUSTER} -n hearth get pods"
    echo "  just helm-status"
    echo "  just helm-test-cluster"
    echo "  just helm-down          # tear down"

# Deploy with all capabilities enabled (identity, mesh, builds)
helm-up-full:
    #!/usr/bin/env bash
    set -euo pipefail
    CLUSTER=hearth-home
    if ! kind get clusters 2>/dev/null | grep -q "^${CLUSTER}$"; then
        echo "==> Creating Kind cluster '${CLUSTER}'..."
        kind create cluster --name "${CLUSTER}" --wait 60s
    fi
    helm dependency update chart/hearth-home
    echo "==> Installing Hearth Home Cluster (all capabilities)..."
    helm upgrade --install hearth chart/hearth-home \
        --create-namespace --namespace hearth \
        --set capabilities.identity=true \
        --set capabilities.mesh=true \
        --set capabilities.builds=true \
        --set capabilities.observability=false \
        --wait --timeout 300s
    echo ""
    echo "=== Hearth Home Cluster is running (all capabilities) ==="

# Show status of the Hearth Home Cluster
helm-status:
    kubectl --context kind-hearth-home -n hearth get pods,svc,pvc

# Run helm tests against the running cluster
helm-test-cluster:
    helm test hearth --namespace hearth --timeout 120s

# Port-forward hearth-api to localhost:3000
helm-forward:
    kubectl --context kind-hearth-home -n hearth port-forward svc/hearth-hearth-home-api 3000:3000

# Tear down the Kind cluster
helm-down:
    kind delete cluster --name hearth-home

