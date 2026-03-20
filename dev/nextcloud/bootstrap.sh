#!/usr/bin/env bash
# dev/nextcloud/bootstrap.sh — Configure Nextcloud with OIDC and defaults
#
# Idempotent: safe to run multiple times. Installs user_oidc app,
# configures Kanidm as the OIDC provider, sets up Redis caching,
# and creates default shared folders.
#
# Prerequisites:
#   - Nextcloud running and healthy (docker-compose up nextcloud)
#   - Kanidm bootstrap already run (OIDC client secret written to .env)
#
# Usage:
#   bash dev/nextcloud/bootstrap.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
NEXTCLOUD_URL="${NEXTCLOUD_URL:-http://localhost:8089}"
KANIDM_URL="${KANIDM_URL:-https://localhost:8443}"

echo "==> Nextcloud bootstrap for Hearth Cloud Storage"
echo "    URL: $NEXTCLOUD_URL"

# Helper: run occ command inside the Nextcloud container
occ() {
    docker compose exec -T -u www-data nextcloud php occ "$@"
}

# ---------------------------------------------------------------------------
# Step 1: Wait for Nextcloud to be ready
# ---------------------------------------------------------------------------
echo ""
echo "==> Waiting for Nextcloud to be ready..."

MAX_WAIT=120
WAITED=0
until curl -sf "$NEXTCLOUD_URL/status.php" | grep -q '"installed":true' 2>/dev/null; do
    if [ "$WAITED" -ge "$MAX_WAIT" ]; then
        echo "    ERROR: Nextcloud not ready after ${MAX_WAIT}s"
        exit 1
    fi
    sleep 2
    WAITED=$((WAITED + 2))
done
echo "    Nextcloud is ready"

# ---------------------------------------------------------------------------
# Step 2: Configure Redis caching
# ---------------------------------------------------------------------------
echo ""
echo "==> Configuring Redis caching..."

occ config:system:set redis host --value="nextcloud-redis"
occ config:system:set redis port --value="6379" --type=integer
occ config:system:set memcache.local --value='\OC\Memcache\APCu'
occ config:system:set memcache.locking --value='\OC\Memcache\Redis'
occ config:system:set memcache.distributed --value='\OC\Memcache\Redis'
echo "    Redis caching configured"

# ---------------------------------------------------------------------------
# Step 3: Set trusted domains and other config
# ---------------------------------------------------------------------------
echo ""
echo "==> Configuring trusted domains..."

occ config:system:set trusted_domains 0 --value="localhost"
occ config:system:set trusted_domains 1 --value="localhost:8089"
occ config:system:set trusted_domains 2 --value="nextcloud"
occ config:system:set overwrite.cli.url --value="$NEXTCLOUD_URL"
occ config:system:set default_phone_region --value="US"
echo "    Trusted domains configured"

# ---------------------------------------------------------------------------
# Step 4: Install and configure user_oidc app
# ---------------------------------------------------------------------------
echo ""
echo "==> Setting up OIDC authentication..."

# Install user_oidc if not already installed
if occ app:list --shipped=false 2>/dev/null | grep -q "user_oidc"; then
    echo "    user_oidc app already installed"
else
    occ app:install user_oidc 2>/dev/null || echo "    user_oidc install returned non-zero (may already be installed)"
fi

# Enable the app
occ app:enable user_oidc 2>/dev/null || true

# Read OIDC client secret from Kanidm bootstrap output
NEXTCLOUD_SECRET=""
if [ -f "$SCRIPT_DIR/../kanidm/.env" ]; then
    NEXTCLOUD_SECRET=$(grep '^NEXTCLOUD_OIDC_CLIENT_SECRET=' "$SCRIPT_DIR/../kanidm/.env" 2>/dev/null | cut -d= -f2 || true)
fi

if [ -n "$NEXTCLOUD_SECRET" ]; then
    # Configure OIDC provider (update if exists, create if not)
    # Check if provider already exists
    EXISTING=$(occ user_oidc:provider 2>/dev/null | grep -c "Hearth" || true)
    if [ "$EXISTING" -gt 0 ]; then
        echo "    OIDC provider 'Hearth' already configured — updating"
    fi

    occ user_oidc:provider Hearth \
        --clientid="hearth-nextcloud" \
        --clientsecret="$NEXTCLOUD_SECRET" \
        --discoveryuri="${KANIDM_URL}/oauth2/openid/hearth-nextcloud/.well-known/openid-configuration" \
        --unique-uid=0 \
        --mapping-uid="preferred_username" \
        --mapping-display-name="name" \
        --mapping-email="email" \
        --check-bearer="0" 2>/dev/null || true

    # Allow unencrypted OIDC in dev (Kanidm uses self-signed cert)
    occ config:app:set user_oidc allow_multiple_user_backends --value="1" 2>/dev/null || true

    echo "    OIDC provider 'Hearth' configured (Kanidm)"
else
    echo "    WARNING: No NEXTCLOUD_OIDC_CLIENT_SECRET found in dev/kanidm/.env"
    echo "    Run 'just setup' or 'bash dev/kanidm/bootstrap.sh' first"
fi

# ---------------------------------------------------------------------------
# Step 5: Create default shared folders for admin user
# ---------------------------------------------------------------------------
echo ""
echo "==> Creating default folder structure..."

# Create standard folders for the admin user
for folder in Documents Projects Shared; do
    occ files:scan admin --path="/admin/files/$folder" 2>/dev/null || \
        docker compose exec -T -u www-data nextcloud mkdir -p "/var/www/html/data/admin/files/$folder" 2>/dev/null || true
done

# Trigger a file scan to pick up the new folders
occ files:scan admin 2>/dev/null || true
echo "    Default folders created"

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
echo ""
echo "=== Nextcloud bootstrap complete ==="
echo ""
echo "  Nextcloud URL:  $NEXTCLOUD_URL"
echo "  Admin user:     admin / admin"
echo "  OIDC provider:  Hearth (Kanidm)"
echo ""
echo "  WebDAV endpoint:"
echo "    ${NEXTCLOUD_URL}/remote.php/dav/files/USERNAME/"
echo ""
echo "  Default folders: Documents, Projects, Shared"
echo ""
