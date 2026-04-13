# Hearth development workflow recipes

# Default recipe: show available commands
default:
    @just --list

# One-time dev environment setup
setup:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "==> Pre-seeding runtime files..."
    # Synapse reads /data/oidc_client_secret at startup — the real value is
    # written later by dev/synapse/bootstrap.sh after Kanidm is ready.
    [ -f dev/synapse/oidc_client_secret ] || echo -n "placeholder-will-be-replaced" > dev/synapse/oidc_client_secret
    # Kanidm TLS cert must exist as a file before docker compose bind-mounts it,
    # otherwise Docker creates a directory placeholder that Kanidm can't read.
    if [ ! -f dev/kanidm/cert.pem ] || [ ! -f dev/kanidm/key.pem ]; then
        # Clean up directory placeholders left by Docker if cert was deleted while running
        [ -d dev/kanidm/cert.pem ] && rmdir dev/kanidm/cert.pem
        [ -d dev/kanidm/key.pem ] && rmdir dev/kanidm/key.pem
        echo "    Generating Kanidm self-signed TLS certificate..."
        openssl req -x509 -newkey rsa:4096 -sha256 -days 3650 -nodes \
            -keyout dev/kanidm/key.pem \
            -out dev/kanidm/cert.pem \
            -subj "/CN=kanidm.hearth.local" \
            -addext "basicConstraints=critical,CA:FALSE" \
            -addext "subjectAltName=DNS:kanidm.hearth.local,DNS:localhost,IP:127.0.0.1,IP:10.0.2.2,IP:::1" \
            2>/dev/null
    fi
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
    echo "==> Generating cache signing key..."
    if [ ! -f dev/attic/signing-key.sec ]; then
        nix key generate-secret --key-name hearth-cache > dev/attic/signing-key.sec
        nix key convert-secret-to-public < dev/attic/signing-key.sec > dev/attic/signing-key.pub
        echo "    Generated signing key-pair in dev/attic/"
    else
        echo "    Signing key already exists"
    fi
    CACHE_PUBLIC_KEY=$(cat dev/attic/signing-key.pub)
    echo "    Public key: $CACHE_PUBLIC_KEY"
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
    echo "==> Exporting Caddy Dev CA..."
    bash dev/caddy/bootstrap.sh
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

# Run the full demo environment (setup + seed data + start API)
demo:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "=== Hearth Demo Environment ==="
    echo ""
    just setup
    echo "==> Building and caching Hearth packages..."
    just push-cache
    echo "==> Seeding demo data..."
    bash dev/seed-demo-data.sh
    echo ""
    echo "=== Demo ready! ==="
    echo ""
    echo "  From host (direct ports):"
    echo "    Web UI (HMR):  http://localhost:5174  ← use this for dev"
    echo "    Web UI (API):  http://localhost:3000  (serves pre-built dist)"
    echo "    Element Chat:  http://localhost:8088"
    echo "    Nextcloud:     http://localhost:8089"
    echo "    Grafana:       http://localhost:3001"
    echo "    Kanidm:        https://localhost:8443"
    echo ""
    echo "  From enrolled VM (via Caddy + Dev CA):"
    echo "    Catalog:   https://api.hearth.local/"
    echo "    Kanidm:    https://kanidm.hearth.local/"
    echo "    Chat:      https://chat.hearth.local/"
    echo "    Cloud:     https://cloud.hearth.local/"
    echo "    Grafana:   https://grafana.hearth.local/"
    echo ""
    echo "  Login:  testadmin / test-demo-enrollment  (admin)"
    echo "          testuser  / test-demo-enrollment  (user)"
    echo ""
    echo "  IMPORTANT — to log in from your host browser, run ONCE:"
    echo "    just host-aliases   (adds *.hearth.local to /etc/hosts, sudo)"
    echo "  Then accept the Kanidm self-signed cert warning on first visit."
    echo ""
    echo "  Next:   just enroll <vm-name>    (e.g. just enroll demo)"
    echo "  See docs/DEMO.md for the full walkthrough."
    echo ""
    echo "  Starting API + Vite dev server (HMR)..."
    just dev-full

# Seed the database with demo data (requires infrastructure running)
seed:
    bash dev/seed-demo-data.sh

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
    # Service directory URLs for dev environment
    export HEARTH_SERVER_URL=http://localhost:3000
    export HEARTH_CHAT_URL=http://localhost:8088
    export HEARTH_CLOUD_URL=http://localhost:8089
    export HEARTH_IDENTITY_URL=https://localhost:8443
    export HEARTH_MATRIX_SERVER_NAME=hearth.local
    export HEARTH_FLAKE_REF="${HEARTH_FLAKE_REF:-tarball+http://localhost:3000/api/v1/fleet-config/flake.tar.gz}"
    # Cache URL as seen by enrolled VMs (10.0.2.2 = QEMU host gateway).
    export HEARTH_ATTIC_SERVER="http://10.0.2.2:8080"
    # Cache signing public key for signature verification.
    if [ -f dev/attic/signing-key.pub ]; then
        export HEARTH_CACHE_PUBLIC_KEY="$(cat dev/attic/signing-key.pub)"
    fi
    echo ""
    echo "NOTE: 'just dev' only runs the API server (serves pre-built web/dist)."
    echo "  For live frontend HMR, run 'just dev-full' instead, or 'just web-dev' in a second terminal."
    echo "  HEARTH_FLAKE_REF=$HEARTH_FLAKE_REF"
    echo ""
    exec cargo run -p hearth-api

# Start API + Vite dev server (frontend HMR) together.
# API on :3000, web on :5174 (proxies /api to :3000). Open http://localhost:5174.
dev-full:
    #!/usr/bin/env bash
    set -euo pipefail
    if [ -f dev/kanidm/.env ]; then
        set -a; source dev/kanidm/.env; set +a
        echo "Loaded auth config from dev/kanidm/.env (auth enabled)"
    else
        echo "WARNING: dev/kanidm/.env not found — auth disabled. Run 'just setup' first."
    fi
    export HEARTH_SERVER_URL=http://localhost:3000
    export HEARTH_CHAT_URL=http://localhost:8088
    export HEARTH_CLOUD_URL=http://localhost:8089
    export HEARTH_IDENTITY_URL=https://localhost:8443
    export HEARTH_MATRIX_SERVER_NAME=hearth.local
    export HEARTH_FLAKE_REF="${HEARTH_FLAKE_REF:-tarball+http://localhost:3000/api/v1/fleet-config/flake.tar.gz}"
    export ATTIC_CACHE_URL="${ATTIC_CACHE_URL:-http://localhost:8080}"
    # Cache URL as seen by enrolled VMs (10.0.2.2 = QEMU host gateway).
    export HEARTH_ATTIC_SERVER="http://10.0.2.2:8080"
    # Cache signing key for the build worker.
    export HEARTH_CACHE_SIGNING_KEY="${HEARTH_CACHE_SIGNING_KEY:-dev/attic/signing-key.sec}"
    # Cache public key for enrolled machines.
    if [ -f dev/attic/signing-key.pub ]; then
        export HEARTH_CACHE_PUBLIC_KEY="$(cat dev/attic/signing-key.pub)"
    fi
    (cd web && pnpm dev) &
    WEB_PID=$!
    # Start build worker in background so build jobs are automatically claimed.
    export ATTIC_CACHE_NAME="${ATTIC_CACHE_NAME:-hearth}"
    cargo run -p hearth-build-worker &
    WORKER_PID=$!
    trap "kill $WEB_PID $WORKER_PID 2>/dev/null || true" EXIT INT TERM
    echo ""
    echo "=== Hearth dev ==="
    echo "  API:       http://localhost:3000"
    echo "  Web (HMR): http://localhost:5174  ← open this one"
    echo "  Worker:    running (PID $WORKER_PID)"
    echo ""
    cargo run -p hearth-api

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
    # Service directory URLs for dev environment
    export HEARTH_SERVER_URL=http://localhost:3000
    export HEARTH_CHAT_URL=http://localhost:8088
    export HEARTH_CLOUD_URL=http://localhost:8089
    export HEARTH_IDENTITY_URL=https://localhost:8443
    export HEARTH_MATRIX_SERVER_NAME=hearth.local
    export HEARTH_FLAKE_REF="${HEARTH_FLAKE_REF:-tarball+http://localhost:3000/api/v1/fleet-config/flake.tar.gz}"
    # Cache URL as seen by enrolled VMs (10.0.2.2 = QEMU host gateway).
    # Uses Attic's port directly — no Caddy proxy needed for HTTP binary cache.
    export HEARTH_ATTIC_SERVER="http://10.0.2.2:8080"
    exec cargo watch -x 'run -p hearth-api'

# Start a build worker (HEARTH_FLAKE_REF defaults to API tarball endpoint)
worker:
    HEARTH_FLAKE_REF="${HEARTH_FLAKE_REF:-tarball+http://localhost:3000/api/v1/fleet-config/flake.tar.gz}" \
    ATTIC_CACHE_URL="${ATTIC_CACHE_URL:-http://localhost:8080}" \
    ATTIC_CACHE_NAME="${ATTIC_CACHE_NAME:-hearth}" \
    HEARTH_CACHE_SIGNING_KEY="${HEARTH_CACHE_SIGNING_KEY:-dev/attic/signing-key.sec}" \
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

# Add *.hearth.local -> 127.0.0.1 entries to the host /etc/hosts (sudo).
# Idempotent: skips the line if it's already present.
host-aliases:
    #!/usr/bin/env bash
    set -euo pipefail
    MARKER="# hearth demo aliases"
    if grep -qF "$MARKER" /etc/hosts 2>/dev/null; then
        echo "host aliases already present in /etc/hosts"
        exit 0
    fi
    echo "==> Adding *.hearth.local aliases to /etc/hosts (sudo required)..."
    sudo sh -c "printf '\n%s\n127.0.0.1 api.hearth.local kanidm.hearth.local chat.hearth.local cloud.hearth.local grafana.hearth.local cache.hearth.local\n' '$MARKER' >> /etc/hosts"
    echo "    Added. Remove later with: sudo sed -i '/$MARKER/,+1d' /etc/hosts"

# Boot enrollment ISO in QEMU for testing (persists to dev/vms/<name>.qcow2)
enroll name:
    bash dev/run-enrollment.sh {{name}}

# Boot a previously-enrolled VM from its installed disk
start-vm name:
    bash dev/run-vm.sh {{name}}

# List enrolled VMs (disks under dev/vms/)
list-vms:
    @ls -1 dev/vms/ 2>/dev/null | sed 's/\.qcow2$//' || echo "(no VMs yet)"

# Delete an enrolled VM's disk
destroy-vm name:
    rm -f dev/vms/{{name}}.qcow2
    @echo "Removed dev/vms/{{name}}.qcow2"

# Run a pre-enrolled fleet VM (requires `just demo` to be running)
fleet-vm:
    #!/usr/bin/env bash
    set -euo pipefail
    DB_URL="${DATABASE_URL:-postgres://hearth:hearth@localhost:5432/hearth}"
    MACHINE_ID=$(uuidgen)

    # Load dev env for machine token secret
    if [ -f dev/kanidm/.env ]; then
        set -a; source dev/kanidm/.env; set +a
    fi

    if [ -z "${HEARTH_MACHINE_TOKEN_SECRET:-}" ]; then
        echo "ERROR: HEARTH_MACHINE_TOKEN_SECRET not set. Run 'just setup' first."
        exit 1
    fi

    # Mint a machine token (HS256 JWT) and compute its SHA-256 hash for the DB.
    JWT_OUTPUT=$(MACHINE_ID="$MACHINE_ID" SECRET="$HEARTH_MACHINE_TOKEN_SECRET" node -e '
        const crypto = require("crypto");
        const secret = Buffer.from(process.env.SECRET, "base64");
        const machineId = process.env.MACHINE_ID;
        const now = Math.floor(Date.now() / 1000);
        const header = Buffer.from(JSON.stringify({alg:"HS256",typ:"JWT"})).toString("base64url");
        const payload = Buffer.from(JSON.stringify({
            sub: "machine:" + machineId,
            machine_id: machineId,
            iat: now,
            exp: now + 90*24*3600
        })).toString("base64url");
        const sig = crypto.createHmac("sha256", secret)
            .update(header+"."+payload).digest("base64url");
        const token = header+"."+payload+"."+sig;
        const hash = crypto.createHash("sha256").update(token).digest("hex");
        process.stdout.write(token + " " + hash);
    ')
    MACHINE_TOKEN="${JWT_OUTPUT% *}"
    TOKEN_HASH="${JWT_OUTPUT#* }"

    echo "==> Registering fleet-vm as machine $MACHINE_ID..."
    psql "$DB_URL" --quiet -c "
        INSERT INTO machines (id, hostname, enrollment_status, role, tags, hardware_fingerprint, machine_token_hash)
        VALUES ('$MACHINE_ID', 'hearth-fleet-vm', 'active', 'developer', '{}', 'fleet-vm-$(date +%s)', '$TOKEN_HASH')
    "

    # Create log directory for 9p shared mount
    export FLEET_VM_LOGS="$(pwd)/dev/fleet-vm-logs"
    mkdir -p "$FLEET_VM_LOGS"

    echo "==> Building and booting fleet VM..."
    echo "    Machine ID: $MACHINE_ID"
    echo "    Login with: testadmin / test-demo-enrollment"
    echo "    Logs will appear in: dev/fleet-vm-logs/"
    echo ""
    HEARTH_FLEET_VM_MACHINE_ID="$MACHINE_ID" \
    HEARTH_FLEET_VM_MACHINE_TOKEN="$MACHINE_TOKEN" \
        nix run --impure .#fleet-vm

# Clean up fleet VM state (disk images, DB rows, logs)
fleet-vm-clean:
    #!/usr/bin/env bash
    set -euo pipefail
    DB_URL="${DATABASE_URL:-postgres://hearth:hearth@localhost:5432/hearth}"
    echo "==> Removing fleet-vm machine rows from DB..."
    psql "$DB_URL" --quiet -c "DELETE FROM machines WHERE hostname = 'hearth-fleet-vm'" 2>/dev/null || true
    echo "==> Removing fleet VM disk image..."
    rm -f hearth-fleet-vm.qcow2
    echo "==> Clearing fleet VM logs..."
    rm -rf dev/fleet-vm-logs/*
    echo "==> Done. Run 'just fleet-vm' to start fresh."

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

# Build all Hearth packages and push them to the local Attic cache.
# This ensures the build worker can substitute packages instead of building from source.
push-cache:
    #!/usr/bin/env bash
    set -euo pipefail
    SIGNING_KEY="dev/attic/signing-key.sec"
    echo "Building hearth packages..."
    nix build .#hearth-agent .#hearth-greeter .#hearth-enrollment --no-link --print-out-paths | while read -r path; do
        # Sign the store path before pushing if signing key exists
        if [ -f "$SIGNING_KEY" ]; then
            echo "  Signing $path"
            nix store sign --key-file "$SIGNING_KEY" "$path" 2>/dev/null || true
        fi
        echo "  Pushing $path"
        attic push hearth "$path" 2>/dev/null || true
    done
    echo "Packages pushed to Attic cache"

# Validate home-manager role profiles evaluate successfully
check-home:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "Evaluating home-manager role profiles..."
    failed=0
    for role in default developer designer admin; do
        printf "  %-12s" "$role"
        if nix eval --no-eval-cache --impure --expr \
          "let flake = builtins.getFlake \"path:$(pwd)\"; in flake.homeConfigurations.$role.activationPackage" \
          > /dev/null 2>&1; then
            echo "✓"
        else
            echo "✗ FAILED"
            # Re-run to show the error
            nix eval --no-eval-cache --impure --expr \
              "let flake = builtins.getFlake \"path:$(pwd)\"; in flake.homeConfigurations.$role.activationPackage" \
              2>&1 | tail -20
            failed=1
        fi
    done
    echo ""
    echo "Evaluating buildUserEnv with empty overrides..."
    for role in default developer designer admin; do
        printf "  %-12s" "$role"
        cfg=$(mktemp)
        echo "{\"username\":\"check\",\"base_role\":\"$role\",\"overrides\":{}}" > "$cfg"
        if nix build --no-link --no-eval-cache --impure --expr \
          "let flake = builtins.getFlake \"path:$(pwd)\"; in flake.lib.buildUserEnv { userConfigPath = \"$cfg\"; }" \
          > /dev/null 2>&1; then
            echo "✓"
        else
            echo "✗ FAILED"
            nix build --no-link --no-eval-cache --impure --expr \
              "let flake = builtins.getFlake \"path:$(pwd)\"; in flake.lib.buildUserEnv { userConfigPath = \"$cfg\"; }" \
              2>&1 | tail -20
            failed=1
        fi
        rm -f "$cfg"
    done
    if [ "$failed" -eq 1 ]; then
        echo ""
        echo "Some profiles failed evaluation!"
        exit 1
    fi
    echo ""
    echo "All home-manager profiles OK"

# Validate that the fleet config evaluates successfully (catches module errors early)
check-fleet:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "Evaluating fleet config with dummy instance data..."
    cat > /tmp/hearth-check-fleet.json << 'INST'
    {"hostname":"check-fleet","machine_id":"00000000-0000-0000-0000-000000000000","role":"default","server_url":"http://localhost:3000","hardware_config":null,"extra_config":{}}
    INST
    nix eval --no-eval-cache --impure --expr \
      'let flake = builtins.getFlake "path:'"$(pwd)"'"; in (flake.lib.buildMachineConfig { instanceDataPath = "/tmp/hearth-check-fleet.json"; }).config.system.build.toplevel' \
      > /dev/null
    echo "Fleet config evaluation succeeded"

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

# ============================================================================
# Security & compliance workflows
# ============================================================================

# Show implemented compliance controls
compliance-status:
    @echo "==> Implemented compliance controls:"
    @find modules/compliance -name "*.nix" ! -name "default.nix" \
        -exec basename {} .nix \; | sort | sed 's/^/    /'
    @echo ""
    @echo "==> Hardening module:"
    @echo "    modules/hardening.nix (standard + strict levels)"
    @echo ""
    @echo "==> Registry: docs/compliance-controls.yaml"
    @echo "==> Run /compliance-audit cis-level1 in Claude Code for full gap analysis"

# Check Helm chart security hardening posture
helm-security:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "==> Rendering chart manifests..."
    rendered=$(helm template hearth chart/hearth-home \
        --set capabilities.observability=false 2>/dev/null)
    echo ""
    echo "==> Checking for container securityContext..."
    sc=$(echo "$rendered" | grep -c "securityContext:" || true)
    echo "    Found $sc securityContext block(s) in rendered manifests"
    echo ""
    echo "==> Checking for NetworkPolicy resources..."
    np=$(echo "$rendered" | grep -c "^kind: NetworkPolicy" || true)
    echo "    Found $np NetworkPolicy resource(s)"
    echo ""
    echo "==> Checking for PodDisruptionBudget resources..."
    pdb=$(echo "$rendered" | grep -c "^kind: PodDisruptionBudget" || true)
    echo "    Found $pdb PodDisruptionBudget resource(s)"
    echo ""
    echo "==> Run /hardening-check helm in Claude Code for detailed analysis"

# Run cargo-audit for known vulnerabilities in Rust dependencies
cargo-audit:
    @cargo audit || echo "Install cargo-audit: cargo install cargo-audit"

