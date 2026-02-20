#!/usr/bin/env bash
# dev/setup.sh — One-time dev environment bootstrap
#
# Use this if you don't have `just` installed. Otherwise, prefer `just setup`.
#
# Usage:
#   bash dev/setup.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT"

echo "==> Starting infrastructure..."
docker-compose up -d

echo "==> Waiting for services..."
until docker-compose exec -T postgres pg_isready -U hearth >/dev/null 2>&1; do sleep 1; done
echo "    PostgreSQL ready"
until docker-compose exec -T attic curl -sf http://localhost:8080/ >/dev/null 2>&1; do sleep 2; done
echo "    Attic ready"
until curl -sf --insecure https://localhost:8443/status >/dev/null 2>&1; do sleep 2; done
echo "    Kanidm ready"

echo "==> Configuring Attic cache..."
TOKEN=$(docker-compose exec -T attic atticadm make-token \
    --config /etc/attic/server.toml \
    --sub "dev" \
    --validity "10y" \
    --pull '*' --push '*' --create-cache '*' --delete '*' \
    2>/dev/null || echo "")
if [ -n "$TOKEN" ]; then
    attic login dev http://localhost:8080 "$TOKEN"
    attic cache create hearth || true
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
cd "$REPO_ROOT"

echo ""
echo "=== Setup complete! ==="
echo ""
echo "  just dev                   — Start API server (with Kanidm auth)"
echo "  bash dev/run-enrollment.sh — Boot enrollment ISO in QEMU"
echo "  nix run .#fleet-vm         — Run pre-built fleet VM"
echo ""
