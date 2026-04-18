#!/bin/bash
# dev/stalwart/bootstrap.sh — Idempotent Stalwart Mail Server provisioning
#
# Creates the mail domain and test user accounts via the Stalwart admin API.
# Safe to run multiple times — operations are idempotent.

set -euo pipefail

STALWART_URL="${STALWART_URL:-http://localhost:8025}"
DOMAIN="hearth.local"
ADMIN_PASSWORD="${STALWART_ADMIN_PASSWORD:-admin}"

echo "==> Waiting for Stalwart to be ready..."
for i in $(seq 1 30); do
    if curl -sf "$STALWART_URL/" > /dev/null 2>&1; then
        echo "    Stalwart is ready"
        break
    fi
    if [ "$i" = "30" ]; then
        echo "ERROR: Stalwart did not become healthy"
        exit 1
    fi
    sleep 2
done

# Get admin auth token
AUTH=$(echo -n "admin:${ADMIN_PASSWORD}" | base64)

api() {
    local method="$1"
    local path="$2"
    local data="${3:-}"
    if [ -n "$data" ]; then
        curl -sf -X "$method" "$STALWART_URL$path" \
            -H "Authorization: Basic $AUTH" \
            -H "Content-Type: application/json" \
            -d "$data" 2>/dev/null || true
    else
        curl -sf -X "$method" "$STALWART_URL$path" \
            -H "Authorization: Basic $AUTH" 2>/dev/null || true
    fi
}

# Create domain
echo "==> Creating domain '$DOMAIN'..."
api POST "/api/domain/$DOMAIN" '{}' || true
echo "    Domain configured"

# Create test user accounts with mailboxes
echo "==> Creating test user accounts..."
for user in testuser testadmin testdev testdesigner; do
    echo "    Creating $user@$DOMAIN..."
    api POST "/api/principal" "{
        \"type\": \"individual\",
        \"name\": \"$user\",
        \"secrets\": [\"password\"],
        \"emails\": [\"$user@$DOMAIN\"],
        \"description\": \"Hearth test user ($user)\"
    }" || true
done

echo ""
echo "==> Stalwart bootstrap complete!"
echo "    Admin UI: $STALWART_URL"
echo "    Domain:   $DOMAIN"
echo "    IMAP:     localhost:1993 (TLS)"
echo "    SMTP:     localhost:1587 (STARTTLS)"
echo "    Users:    testuser@$DOMAIN, testadmin@$DOMAIN, testdev@$DOMAIN, testdesigner@$DOMAIN"
echo "    Password: password (for all test accounts)"
